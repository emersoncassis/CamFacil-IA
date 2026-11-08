//! Tela Rede (SDD-05, fase 4): foto da rede, inventário e diferença entre fotos.
//!
//! A varredura roda numa tarefa própria; a UI acompanha por polling em
//! `GET /api/rede/foto`. Nada aqui roda sozinho: só quando o técnico pede.

use crate::{App, Shared};
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use cf_core::rede::{self, CameraRede, Foto, Mudanca};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

struct EstadoFoto {
    rodando: bool,
    progresso: Arc<AtomicUsize>,
    erro: Option<String>,
}

pub struct Rede {
    foto: Mutex<EstadoFoto>,
}

static REDE: OnceLock<Arc<Rede>> = OnceLock::new();

/// Chamado uma vez no início do programa.
pub fn iniciar(app: Shared) {
    let _ = &app;
    let r = Arc::new(Rede {
        foto: Mutex::new(EstadoFoto {
            rodando: false,
            progresso: Arc::new(AtomicUsize::new(0)),
            erro: None,
        }),
    });
    let _ = REDE.set(r);
}

fn rede() -> Option<Arc<Rede>> {
    REDE.get().cloned()
}

/// Câmeras RTSP cadastradas, na forma que a rede precisa.
pub fn cameras_rede(app: &App) -> Vec<CameraRede> {
    app.store
        .cameras()
        .unwrap_or_default()
        .iter()
        .filter_map(CameraRede::de)
        .collect()
}

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn erro(status: StatusCode, msg: impl ToString) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "erro": msg.to_string() })))
}

/// `POST /api/rede/foto`: começa a varredura (409 se já estiver rodando).
pub async fn post_foto(State(app): State<Shared>) -> ApiResult {
    let r = rede().ok_or_else(|| erro(StatusCode::SERVICE_UNAVAILABLE, "rede não iniciada"))?;
    let progresso = {
        let mut e = r.foto.lock().unwrap_or_else(|p| p.into_inner());
        if e.rodando {
            return Err(erro(
                StatusCode::CONFLICT,
                "já estou tirando uma foto da rede",
            ));
        }
        e.rodando = true;
        e.erro = None;
        e.progresso = Arc::new(AtomicUsize::new(0));
        e.progresso.clone()
    };
    let cams = cameras_rede(&app);
    tokio::spawn(async move {
        let foto = rede::tirar_foto(&cams, progresso).await;
        let resultado = app.store.foto_rede_salvar(&foto).map_err(|e| e.to_string());
        if let Some(r) = rede() {
            let mut e = r.foto.lock().unwrap_or_else(|p| p.into_inner());
            e.rodando = false;
            e.erro = resultado.err();
        }
    });
    Ok(Json(
        json!({ "iniciado": true, "total": rede::total_sondas() }),
    ))
}

/// `GET /api/rede/foto`: progresso, última foto, diferença para a anterior e histórico.
pub async fn get_foto(State(app): State<Shared>) -> ApiResult {
    let (rodando, progresso, erro_txt) = match rede() {
        Some(r) => {
            let e = r.foto.lock().unwrap_or_else(|p| p.into_inner());
            (
                e.rodando,
                e.progresso.load(Ordering::Relaxed),
                e.erro.clone(),
            )
        }
        None => (false, 0, None),
    };
    let fotos = app
        .store
        .fotos_rede(2)
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let ultima: Option<&Foto> = fotos.first().map(|(_, f)| f);
    let anterior: Option<&Foto> = fotos.get(1).map(|(_, f)| f);
    let mudancas: Vec<Mudanca> = match (anterior, ultima) {
        (Some(a), Some(u)) => rede::comparar(a, u),
        _ => Vec::new(),
    };
    let historico = app
        .store
        .fotos_rede_resumo(30)
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({
        "rodando": rodando,
        "progresso": progresso,
        "total": rede::total_sondas(),
        "erro": erro_txt,
        "ultima": ultima,
        "anterior_quando": anterior.map(|a| a.quando),
        "mudancas": mudancas,
        "historico": historico,
    })))
}

/// `GET /api/rede/fotos/{id}`: uma foto antiga inteira.
pub async fn get_foto_id(
    State(app): State<Shared>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> axum::response::Response {
    let fotos = app.store.fotos_rede(30).unwrap_or_default();
    match fotos.into_iter().find(|(i, _)| *i == id) {
        Some((_, f)) => Json(json!({ "foto": f })).into_response(),
        None => (StatusCode::NOT_FOUND, "foto não encontrada").into_response(),
    }
}
