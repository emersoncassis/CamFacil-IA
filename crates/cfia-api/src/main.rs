//! API da nuvem do CamFácil IA (https://cfia.ysafe.com.br) — SDD-08.
//!
//! Serve o web app, o login com Google, as contas de técnico, o pareamento dos
//! Agentes-Cam-Elo-IA e a sinalização WebRTC. Nunca recebe vídeo.

mod api;
mod auth;
mod db;
mod estudio;
mod hub;
mod util;
mod web;

use axum::{
    extract::Request,
    http::{header, HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState(Arc<Inner>);

pub struct Inner {
    pub db: deadpool_postgres::Pool,
    pub http: reqwest::Client,
    pub google: Option<auth::Google>,
    pub jwks: auth::Jwks,
    pub hub: hub::Hub,
    pub limite: api::Limite,
    /// Ex.: https://cfia.ysafe.com.br (sem barra no fim).
    pub base_url: String,
}

impl std::ops::Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

/// Erro de API: código HTTP + mensagem em pt-BR.
pub type Falha = (StatusCode, Json<serde_json::Value>);

pub fn erro(status: StatusCode, msg: &str) -> Falha {
    (status, Json(json!({"erro": msg})))
}

pub fn interno<E: std::fmt::Display>(e: E) -> Falha {
    eprintln!("erro interno: {e}");
    erro(StatusCode::INTERNAL_SERVER_ERROR, "erro interno")
}

/// IP do cliente (atrás do proxy do Dokploy/Traefik).
pub fn ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().to_string())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        })
        .unwrap_or_default()
}

pub fn origem_ok(state: &AppState, headers: &HeaderMap) -> bool {
    headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|o| o == state.base_url)
}

/// Pedidos que mudam algo precisam vir do próprio site (proteção CSRF).
async fn exigir_origem(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let muda = !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if muda && req.uri().path().starts_with("/api/") && !origem_ok(&state, req.headers()) {
        return erro(StatusCode::FORBIDDEN, "origem não permitida").into_response();
    }
    next.run(req).await
}

async fn cabecalhos(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let mut res = next.run(req).await;
    // WebSocket segue o protocolo do domínio (wss em produção, ws no teste local).
    let ws = match state.base_url.strip_prefix("https://") {
        Some(h) => format!("wss://{h}"),
        None => format!("ws://{}", state.base_url.trim_start_matches("http://")),
    };
    let csp = format!(
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:; \
         connect-src 'self' {ws}; frame-ancestors 'none'; base-uri 'none'; \
         form-action 'self' https://accounts.google.com"
    );
    let h = res.headers_mut();
    let mut set = |k: &'static str, v: &str| {
        if let Ok(v) = HeaderValue::from_str(v) {
            h.insert(k, v);
        }
    };
    set("content-security-policy", &csp);
    set(
        "strict-transport-security",
        "max-age=31536000; includeSubDomains",
    );
    set("referrer-policy", "no-referrer");
    set("x-content-type-options", "nosniff");
    set("x-frame-options", "DENY");
    set(
        "permissions-policy",
        "camera=(self), microphone=(), geolocation=()",
    );
    res
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/saude", get(api::saude))
        .route("/baixar/{arquivo}", get(web::baixar))
        .route("/api/auth/google", get(auth::iniciar))
        .route("/api/auth/google/callback", get(auth::retorno))
        .route("/api/auth/sair", post(auth::sair))
        .route("/api/eu", get(api::eu))
        .route("/api/escola/selo", get(api::meu_selo).post(api::gerar_selo))
        .route("/api/escola/selo/{codigo}", get(api::selo_publico))
        .route(
            "/api/estudio/receitas",
            get(estudio::listar).post(estudio::publicar),
        )
        .route("/api/estudio/receitas/minhas", get(estudio::minhas))
        .route("/api/estudio/receitas/{id}", delete(estudio::apagar))
        .route(
            "/api/estudio/receitas/{id}/receita.json",
            get(estudio::baixar),
        )
        .route("/api/estudio/notebook", post(estudio::notebook))
        .route("/api/estudio/validar", post(estudio::validar))
        .route("/esquemas/receita-v1.schema.json", get(estudio::esquema))
        .route(
            "/api/dispositivos",
            get(api::listar_dispositivos).post(api::criar_dispositivo),
        )
        .route("/api/dispositivos/{id}", delete(api::revogar_dispositivo))
        .route("/api/agentes", get(api::listar_agentes))
        .route("/api/agentes/{id}", patch(api::editar_agente))
        .route("/api/parear", post(api::parear))
        .route("/ws/agente", get(hub::ws_agente))
        .route("/ws/painel", get(hub::ws_painel))
        .fallback(web::estatico)
        .layer(middleware::from_fn_with_state(state.clone(), exigir_origem))
        .layer(middleware::from_fn_with_state(state.clone(), cabecalhos))
        .with_state(state)
}

fn env(nome: &str) -> Option<String> {
    std::env::var(nome).ok().filter(|v| !v.trim().is_empty())
}

#[tokio::main]
async fn main() {
    let url = env("DATABASE_URL").expect("defina DATABASE_URL");
    let base_url = env("BASE_URL")
        .unwrap_or_else(|| "https://cfia.ysafe.com.br".into())
        .trim_end_matches('/')
        .to_string();
    let porta: u16 = env("PORT").and_then(|p| p.parse().ok()).unwrap_or(8080);
    let google = match (env("GOOGLE_CLIENT_ID"), env("GOOGLE_CLIENT_SECRET")) {
        (Some(client_id), Some(client_secret)) => Some(auth::Google {
            client_id,
            client_secret,
        }),
        _ => {
            eprintln!("aviso: GOOGLE_CLIENT_ID/GOOGLE_CLIENT_SECRET ausentes; login desativado");
            None
        }
    };

    let pool = db::pool(&url).expect("DATABASE_URL inválida");
    // O banco pode demorar a subir no primeiro deploy: tenta por até 60 s.
    let mut tentativas = 0;
    loop {
        match db::migrate(&pool).await {
            Ok(()) => break,
            Err(e) if tentativas < 30 => {
                tentativas += 1;
                eprintln!("aguardando o banco ({e})");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            Err(e) => panic!("não foi possível preparar o banco: {e}"),
        }
    }

    let state = AppState(Arc::new(Inner {
        db: pool,
        http: reqwest::Client::builder()
            .user_agent(concat!("cfia-api/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("cliente HTTP"),
        google,
        jwks: auth::Jwks::default(),
        hub: hub::Hub::default(),
        limite: api::Limite::default(),
        base_url,
    }));

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", porta))
        .await
        .expect("não foi possível abrir a porta");
    println!(
        "cfia-api {} ouvindo na porta {porta}",
        env!("CARGO_PKG_VERSION")
    );
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .expect("servidor");
}

#[cfg(test)]
mod testes_integracao;
