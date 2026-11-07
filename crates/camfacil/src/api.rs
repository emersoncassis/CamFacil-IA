//! Rotas HTTP da fase 1.

use crate::{assets, Shared};
use axum::{
    extract::State,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cf_core::{bench, onvif, rtsp, system};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

pub fn router(app: Shared) -> Router {
    Router::new()
        .route("/api/system", get(get_system))
        .route("/api/bench", get(get_bench).post(post_bench))
        .route("/api/rtsp/vendors", get(get_vendors))
        .route("/api/rtsp/build", post(post_build))
        .route("/api/rtsp/test", post(post_test))
        .route("/api/discover", get(get_discover))
        .route("/api/disclaimer", get(get_disclaimer).post(post_disclaimer))
        .fallback(static_file)
        .with_state(app)
}

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn bad(status: StatusCode, msg: impl ToString) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "erro": msg.to_string() })))
}

fn internal<E: std::fmt::Display>(e: E) -> (StatusCode, Json<Value>) {
    bad(StatusCode::INTERNAL_SERVER_ERROR, e)
}

async fn static_file(uri: Uri) -> Response {
    match assets::find(uri.path()) {
        Some(a) => (
            [
                (header::CONTENT_TYPE, a.mime),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            a.body,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "não encontrado").into_response(),
    }
}

async fn get_system(State(app): State<Shared>) -> ApiResult {
    let info = system::read().await;
    let nivel = app.store.get("bench_level").map_err(internal)?;
    Ok(Json(json!({ "sistema": info, "nivel": nivel })))
}

async fn get_bench(State(app): State<Shared>) -> ApiResult {
    let last = app.store.get("bench_result").map_err(internal)?;
    let parsed: Value = last
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null);
    Ok(Json(json!({ "resultado": parsed })))
}

async fn post_bench(State(app): State<Shared>) -> ApiResult {
    let result = tokio::task::spawn_blocking(|| bench::run(Duration::from_secs(4)))
        .await
        .map_err(internal)?;
    let as_json = serde_json::to_string(&result).map_err(internal)?;
    app.store.set("bench_result", &as_json).map_err(internal)?;
    app.store
        .set("bench_level", result.nivel.as_str())
        .map_err(internal)?;
    Ok(Json(json!({ "resultado": result })))
}

async fn get_vendors() -> Json<Value> {
    Json(json!({ "fabricantes": rtsp::VENDORS }))
}

async fn post_build(Json(req): Json<rtsp::BuildRequest>) -> ApiResult {
    let url = rtsp::build(&req).map_err(|e| bad(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(
        json!({ "url": url, "url_sem_senha": rtsp::redact(&url) }),
    ))
}

#[derive(Deserialize)]
struct TestReq {
    ip: String,
    #[serde(default = "rtsp_port")]
    porta: u16,
}
fn rtsp_port() -> u16 {
    554
}

async fn post_test(Json(req): Json<TestReq>) -> Json<Value> {
    Json(json!(rtsp::test(&req.ip, req.porta).await))
}

async fn get_discover() -> Json<Value> {
    let found = onvif::discover(Duration::from_secs(3)).await;
    Json(json!({ "cameras": found }))
}

const DISCLAIMER_DOC: &str = "isencao";

async fn get_disclaimer(State(app): State<Shared>) -> ApiResult {
    let ok = app
        .store
        .accepted(DISCLAIMER_DOC, cf_core::DISCLAIMER_VERSION)
        .map_err(internal)?;
    Ok(Json(
        json!({ "aceito": ok, "versao": cf_core::DISCLAIMER_VERSION }),
    ))
}

async fn post_disclaimer(State(app): State<Shared>) -> ApiResult {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    app.store
        .accept(DISCLAIMER_DOC, cf_core::DISCLAIMER_VERSION, now)
        .map_err(internal)?;
    Ok(Json(json!({ "aceito": true })))
}
