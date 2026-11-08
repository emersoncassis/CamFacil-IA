//! Rotas HTTP das integrações da fase 3b: WhatsApp (Evolution API) e webhook
//! genérico. As rotas em si ficam em `api.rs`; aqui, só os handlers.

use crate::api::{bad, internal, ApiResult};
use crate::Shared;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

type Falha = (StatusCode, Json<Value>);

fn whatsapp(app: &Shared) -> Result<&Arc<cf_notify::WhatsApp>, Falha> {
    app.whatsapp.get().ok_or_else(|| {
        bad(
            StatusCode::SERVICE_UNAVAILABLE,
            "WhatsApp indisponível (não foi possível abrir o cofre de segredos)",
        )
    })
}

/// Endereço local do programa (IP da rede + porta) para montar URLs de webhook
/// e de fotos. Sem IP de rede, cai em `localhost`.
pub fn url_base_local(app: &Shared) -> String {
    let porta = app
        .store
        .get("port")
        .ok()
        .flatten()
        .unwrap_or_else(|| "7070".into());
    let ip = cf_core::conectar::ip_local()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "localhost".into());
    format!("http://{ip}:{porta}")
}

// ---------- WhatsApp ----------

pub async fn get_whatsapp(State(app): State<Shared>) -> ApiResult {
    let wa = whatsapp(&app)?;
    let contatos = app.store.whatsapp_contatos().map_err(internal)?;
    let token = wa.token_webhook().ok();
    Ok(Json(json!({
        "situacao": wa.situacao(),
        "contatos": contatos,
        "webhook_url": token.map(|t| format!("{}/api/whatsapp/webhook/{t}", url_base_local(&app))),
    })))
}

#[derive(Deserialize)]
pub struct ConfigWa {
    url: String,
    apikey: String,
    instancia: String,
}

pub async fn post_whatsapp(State(app): State<Shared>, Json(c): Json<ConfigWa>) -> ApiResult {
    crate::termos::exigir_tecnico(&app)?;
    let wa = whatsapp(&app)?;
    let conexao = wa
        .configurar(&c.url, &c.apikey, &c.instancia)
        .await
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "conexao": conexao })))
}

pub async fn delete_whatsapp(State(app): State<Shared>) -> ApiResult {
    whatsapp(&app)?.remover().map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn post_whatsapp_testar(State(app): State<Shared>) -> ApiResult {
    let (conexao, enviados) = whatsapp(&app)?
        .testar()
        .await
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "conexao": conexao, "enviados": enviados })))
}

/// QR da Evolution em base64 (`data:image/png;base64,…`) para o número ler.
pub async fn post_whatsapp_qr(State(app): State<Shared>) -> ApiResult {
    let qr = whatsapp(&app)?
        .qr()
        .await
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "qr": qr, "conectado": qr.is_none() })))
}

#[derive(Deserialize)]
pub struct FotosReq {
    ligado: bool,
}

pub async fn post_whatsapp_fotos(State(app): State<Shared>, Json(r): Json<FotosReq>) -> ApiResult {
    whatsapp(&app)?.definir_fotos(r.ligado).map_err(internal)?;
    Ok(Json(json!({ "fotos": r.ligado })))
}

#[derive(Deserialize)]
pub struct RegistrarWebhookReq {
    /// Endereço do CamFácil IA que a Evolution alcança (ex.: `http://10.0.0.5:7070`).
    url_base: Option<String>,
}

pub async fn post_whatsapp_webhook_registrar(
    State(app): State<Shared>,
    Json(r): Json<RegistrarWebhookReq>,
) -> ApiResult {
    let wa = whatsapp(&app)?;
    let base = r
        .url_base
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| url_base_local(&app));
    let url = format!(
        "{}/api/whatsapp/webhook/{}",
        base.trim().trim_end_matches('/'),
        wa.token_webhook().map_err(internal)?
    );
    wa.registrar_webhook(&url)
        .await
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "url": url })))
}

/// Entrada dos eventos da Evolution (mensagens recebidas). O token na URL é
/// a única autenticação: quem não o tem recebe 404, sem pista.
pub async fn post_whatsapp_webhook(
    State(app): State<Shared>,
    Path(token): Path<String>,
    Json(corpo): Json<Value>,
) -> ApiResult {
    let wa = whatsapp(&app)?;
    let n = wa
        .receber_webhook(&token, &corpo)
        .await
        .map_err(|_| bad(StatusCode::NOT_FOUND, "não encontrado"))?;
    Ok(Json(json!({ "tratadas": n })))
}

#[derive(Deserialize)]
pub struct ContatoReq {
    autorizado: bool,
}

pub async fn post_whatsapp_contato(
    State(app): State<Shared>,
    Path(jid): Path<String>,
    Json(r): Json<ContatoReq>,
) -> ApiResult {
    if !app
        .store
        .whatsapp_autorizar(&jid, r.autorizado)
        .map_err(internal)?
    {
        return Err(bad(StatusCode::NOT_FOUND, "contato não encontrado"));
    }
    Ok(Json(json!({ "jid": jid, "autorizado": r.autorizado })))
}

pub async fn delete_whatsapp_contato(
    State(app): State<Shared>,
    Path(jid): Path<String>,
) -> ApiResult {
    if !app.store.whatsapp_remover(&jid).map_err(internal)? {
        return Err(bad(StatusCode::NOT_FOUND, "contato não encontrado"));
    }
    Ok(Json(json!({ "removido": true })))
}

// ---------- Webhook genérico ----------

fn webhook(app: &Shared) -> Result<&Arc<cf_notify::Webhook>, Falha> {
    app.webhook.get().ok_or_else(|| {
        bad(
            StatusCode::SERVICE_UNAVAILABLE,
            "Webhook indisponível (não foi possível abrir o cofre de segredos)",
        )
    })
}

pub async fn get_webhook(State(app): State<Shared>) -> ApiResult {
    let wh = webhook(&app)?;
    // Exemplo do JSON que sai, para o técnico montar a automação do outro lado.
    let exemplo = cf_notify::webhook::Evento {
        programa: "CamFácil IA",
        versao: cf_core::VERSION,
        tipo: "regra".into(),
        camera_id: 1,
        camera: "Portão".into(),
        regra_id: Some(3),
        regra: Some("Cerca da frente".into()),
        quando: 1_762_500_000,
        hora: "22:14".into(),
        texto: "22:14 — Cerca da frente: passagem (entrada) · Portão".into(),
        foto_url: Some(format!(
            "{}/api/arquivo/foto/1/1762500000-regra-3.jpg",
            url_base_local(&app)
        )),
    };
    Ok(Json(
        json!({ "situacao": wh.situacao(), "exemplo": exemplo }),
    ))
}

#[derive(Deserialize)]
pub struct ConfigWh {
    url: String,
    #[serde(default)]
    cabecalho: String,
}

pub async fn post_webhook(State(app): State<Shared>, Json(c): Json<ConfigWh>) -> ApiResult {
    crate::termos::exigir_tecnico(&app)?;
    webhook(&app)?
        .configurar(&c.url, &c.cabecalho)
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn delete_webhook(State(app): State<Shared>) -> ApiResult {
    webhook(&app)?.remover().map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn post_webhook_testar(State(app): State<Shared>) -> ApiResult {
    let status = webhook(&app)?
        .enviar_teste()
        .await
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "status": status })))
}
