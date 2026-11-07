//! Rotas HTTP da fase 1.

use crate::{assets, Shared};
use axum::{
    extract::Path,
    extract::State,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
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
        .route("/api/video/ffmpeg", get(get_ffmpeg))
        .route("/api/video/webcams", get(get_webcams))
        .route("/api/cameras", get(get_cameras).post(post_camera))
        .route("/api/cameras/{id}", delete(delete_camera))
        .route("/api/cameras/{id}/mjpeg", get(get_mjpeg))
        .route("/api/cameras/{id}/foto.jpg", get(get_foto))
        .route("/api/cameras/{id}/regras", get(get_regras).post(post_regra))
        .route("/api/regras/{id}", patch(patch_regra).delete(delete_regra))
        .route("/api/estado", get(get_estado))
        .route("/api/eventos", get(get_eventos))
        .route("/api/modulos", get(get_modulos).post(post_modulos))
        .route("/api/selos", get(get_selos))
        .route("/api/elo", get(get_elo))
        .route("/api/elo/codigo", post(post_elo_codigo))
        .route("/api/elo/qr.svg", get(get_elo_qr))
        .route("/api/elo/confirmar", post(post_elo_confirmar))
        .route("/api/elo/desparear", post(post_elo_desparear))
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
    if let Some(nome) = uri
        .path()
        .strip_prefix("/aulas/")
        .filter(|n| n.ends_with(".ogg"))
    {
        return audio_aula(nome).await;
    }
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

// ---------- Agente-Cam-Elo-IA (pareamento com o site) ----------

async fn get_elo(State(app): State<Shared>) -> Json<Value> {
    Json(json!(app.elo.status()))
}

async fn post_elo_codigo(State(app): State<Shared>) -> ApiResult {
    app.elo
        .novo_codigo()
        .map(|s| Json(json!(s)))
        .map_err(|m| bad(StatusCode::CONFLICT, m))
}

async fn get_elo_qr(State(app): State<Shared>) -> Response {
    let svg = app.elo.status().link.and_then(|l| crate::elo::qr_svg(&l));
    match svg {
        Some(s) => (
            [
                (header::CONTENT_TYPE, "image/svg+xml"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            s,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "sem código ativo").into_response(),
    }
}

#[derive(Deserialize)]
struct ConfirmarReq {
    aceito: bool,
}

async fn post_elo_confirmar(State(app): State<Shared>, Json(r): Json<ConfirmarReq>) -> ApiResult {
    app.elo
        .confirmar(r.aceito)
        .map(|_| Json(json!({"ok": true})))
        .map_err(|m| bad(StatusCode::CONFLICT, m))
}

async fn post_elo_desparear(State(app): State<Shared>) -> ApiResult {
    app.elo
        .desparear()
        .map(|_| Json(json!({"ok": true})))
        .map_err(|m| bad(StatusCode::CONFLICT, m))
}

// ---------- Vídeo, câmeras e regras (fase 2) ----------

async fn get_ffmpeg() -> Json<Value> {
    let p = crate::video::achar_ffmpeg();
    let versao = p.as_ref().and_then(crate::video::versao_ffmpeg);
    Json(json!({
        "encontrado": p.is_some(),
        "caminho": p.map(|p| p.display().to_string()),
        "versao": versao,
        "comando_instalar": crate::video::comando_instalar(),
    }))
}

async fn get_webcams() -> Json<Value> {
    let v = tokio::task::spawn_blocking(crate::video::webcams)
        .await
        .unwrap_or_default();
    Json(json!({ "webcams": v }))
}

async fn get_cameras(State(app): State<Shared>) -> ApiResult {
    let cams = app.store.cameras().map_err(internal)?;
    let lista: Vec<Value> = cams
        .into_iter()
        .map(|c| {
            let estado = app.video.canal(c.id).map(|k| k.estado());
            json!({
                "id": c.id,
                "nome": c.nome,
                "fonte": c.fonte,
                "url_sem_senha": rtsp::redact(&c.url),
                "estado": estado,
            })
        })
        .collect();
    Ok(Json(json!({ "cameras": lista })))
}

#[derive(Deserialize)]
struct NovaCamera {
    nome: String,
    fonte: String,
    #[serde(default)]
    url: String,
}

async fn post_camera(State(app): State<Shared>, Json(n): Json<NovaCamera>) -> ApiResult {
    let nome: String = n.nome.trim().chars().take(60).collect();
    if nome.is_empty() {
        return Err(bad(StatusCode::BAD_REQUEST, "dê um nome para a câmera"));
    }
    match n.fonte.as_str() {
        "rtsp" if !n.url.starts_with("rtsp://") => {
            return Err(bad(
                StatusCode::BAD_REQUEST,
                "endereço RTSP inválido; use o gerador",
            ))
        }
        "arquivo" if n.url.trim().is_empty() => {
            return Err(bad(
                StatusCode::BAD_REQUEST,
                "informe o caminho do arquivo de vídeo",
            ))
        }
        "rtsp" | "arquivo" | "demo" | "webcam" => {}
        _ => return Err(bad(StatusCode::BAD_REQUEST, "fonte desconhecida")),
    }
    let id = app
        .store
        .criar_camera(&nome, &n.fonte, n.url.trim())
        .map_err(internal)?;
    app.video.iniciar(id, &n.fonte, n.url.trim());
    Ok(Json(json!({ "id": id })))
}

async fn delete_camera(State(app): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    app.video.parar(id);
    if !app.store.remover_camera(id).map_err(internal)? {
        return Err(bad(StatusCode::NOT_FOUND, "câmera não encontrada"));
    }
    Ok(Json(json!({ "removida": true })))
}

async fn get_foto(State(app): State<Shared>, Path(id): Path<i64>) -> Response {
    match app
        .video
        .canal(id)
        .and_then(|c| c.ultimo.lock().unwrap().clone())
    {
        Some(j) => (
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            j.as_ref().clone(),
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "sem imagem ainda").into_response(),
    }
}

/// Transmissão ao vivo em MJPEG (funciona direto num <img>).
async fn get_mjpeg(State(app): State<Shared>, Path(id): Path<i64>) -> Response {
    let Some(canal) = app.video.canal(id) else {
        return (StatusCode::NOT_FOUND, "câmera não encontrada").into_response();
    };
    let rx = canal.tx.subscribe();
    let primeiro = canal.ultimo.lock().unwrap().clone();
    let corpo = futures_util::stream::unfold((rx, primeiro), |(mut rx, primeiro)| async move {
        let jpeg = match primeiro {
            Some(j) => j,
            None => loop {
                match rx.recv().await {
                    Ok(j) => break j,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return None,
                }
            },
        };
        let mut parte = format!(
            "--quadro\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
            jpeg.len()
        )
        .into_bytes();
        parte.extend_from_slice(&jpeg);
        parte.extend_from_slice(b"\r\n");
        Some((Ok::<_, std::io::Error>(parte), (rx, None)))
    });
    (
        [
            (
                header::CONTENT_TYPE,
                "multipart/x-mixed-replace; boundary=quadro",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        axum::body::Body::from_stream(corpo),
    )
        .into_response()
}

async fn get_regras(State(app): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    Ok(Json(
        json!({ "regras": app.store.regras(id).map_err(internal)? }),
    ))
}

#[derive(Deserialize)]
struct NovaRegra {
    nome: String,
    forma: String,
    pontos: Vec<cf_core::visao::Ponto>,
    #[serde(default)]
    rotulo_on: String,
    #[serde(default)]
    rotulo_off: String,
}

async fn post_regra(
    State(app): State<Shared>,
    Path(cam): Path<i64>,
    Json(r): Json<NovaRegra>,
) -> ApiResult {
    if app.store.camera(cam).map_err(internal)?.is_none() {
        return Err(bad(StatusCode::NOT_FOUND, "câmera não encontrada"));
    }
    let nome: String = r.nome.trim().chars().take(40).collect();
    let min = match r.forma.as_str() {
        "zona" => 3,
        "linha" => 2,
        _ => return Err(bad(StatusCode::BAD_REQUEST, "forma deve ser zona ou linha")),
    };
    if nome.is_empty() {
        return Err(bad(StatusCode::BAD_REQUEST, "dê um nome para a regra"));
    }
    if r.pontos.len() < min || r.pontos.len() > 24 {
        return Err(bad(StatusCode::BAD_REQUEST, "desenho incompleto"));
    }
    if r.pontos
        .iter()
        .any(|p| !(0.0..=1.0).contains(&p.x) || !(0.0..=1.0).contains(&p.y))
    {
        return Err(bad(StatusCode::BAD_REQUEST, "pontos fora da imagem"));
    }
    let pontos = if r.forma == "linha" {
        &r.pontos[..2]
    } else {
        &r.pontos[..]
    };
    let rot = |s: &str| s.trim().chars().take(24).collect::<String>();
    let id = app
        .store
        .criar_regra(
            cam,
            &nome,
            &r.forma,
            pontos,
            &rot(&r.rotulo_on),
            &rot(&r.rotulo_off),
        )
        .map_err(internal)?;
    app.video.recarregar_regras(cam);
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
struct EditarRegra {
    camera_id: i64,
    nome: String,
    #[serde(default)]
    rotulo_on: String,
    #[serde(default)]
    rotulo_off: String,
}

async fn patch_regra(
    State(app): State<Shared>,
    Path(id): Path<i64>,
    Json(r): Json<EditarRegra>,
) -> ApiResult {
    let nome: String = r.nome.trim().chars().take(40).collect();
    if nome.is_empty() {
        return Err(bad(StatusCode::BAD_REQUEST, "dê um nome para a regra"));
    }
    if !app
        .store
        .editar_regra(id, &nome, r.rotulo_on.trim(), r.rotulo_off.trim())
        .map_err(internal)?
    {
        return Err(bad(StatusCode::NOT_FOUND, "regra não encontrada"));
    }
    app.video.recarregar_regras(r.camera_id);
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
struct CamQuery {
    camera_id: i64,
}

async fn delete_regra(
    State(app): State<Shared>,
    Path(id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<CamQuery>,
) -> ApiResult {
    if !app.store.remover_regra(id).map_err(internal)? {
        return Err(bad(StatusCode::NOT_FOUND, "regra não encontrada"));
    }
    app.video.recarregar_regras(q.camera_id);
    Ok(Json(json!({ "removida": true })))
}

async fn get_estado(State(app): State<Shared>) -> ApiResult {
    let cams = app.store.cameras().map_err(internal)?;
    let mapa: serde_json::Map<String, Value> = cams
        .iter()
        .filter_map(|c| {
            app.video
                .canal(c.id)
                .map(|k| (c.id.to_string(), json!(k.estado())))
        })
        .collect();
    Ok(Json(json!({ "cameras": mapa })))
}

#[derive(Deserialize)]
struct LimiteQuery {
    #[serde(default = "cinquenta")]
    limite: i64,
}
fn cinquenta() -> i64 {
    50
}

async fn get_eventos(
    State(app): State<Shared>,
    axum::extract::Query(q): axum::extract::Query<LimiteQuery>,
) -> ApiResult {
    Ok(Json(
        json!({ "eventos": app.store.eventos(q.limite.clamp(1, 500)).map_err(internal)? }),
    ))
}

async fn get_modulos(State(app): State<Shared>) -> Json<Value> {
    let m = crate::video::modulos(&app.store);
    Json(json!({ "movimento": m.movimento, "cerca": m.cerca, "saude": m.saude }))
}

#[derive(Deserialize)]
struct ModuloReq {
    modulo: String,
    ligado: bool,
}

async fn post_modulos(State(app): State<Shared>, Json(r): Json<ModuloReq>) -> ApiResult {
    let chave = match r.modulo.as_str() {
        "movimento" => "modulo_movimento",
        "cerca" => "modulo_cerca",
        "saude" => "modulo_saude",
        _ => return Err(bad(StatusCode::BAD_REQUEST, "módulo desconhecido")),
    };
    app.store
        .set(chave, if r.ligado { "1" } else { "0" })
        .map_err(internal)?;
    Ok(Json(json!({ "modulo": r.modulo, "ligado": r.ligado })))
}

async fn get_selos(State(app): State<Shared>) -> ApiResult {
    let v: Vec<Value> = app
        .store
        .selos()
        .map_err(internal)?
        .into_iter()
        .map(|(m, t)| json!({ "modulo": m, "ganho_em": t }))
        .collect();
    Ok(Json(json!({ "selos": v })))
}

/// Áudios das aulas: pasta `aulas` ao lado do executável ou na pasta de dados
/// (permite regravar sem recompilar); senão, a gravação embutida. Nome validado para não sair da pasta.
async fn audio_aula(nome: &str) -> Response {
    let valido = nome.len() < 64
        && nome
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && !nome.contains("..");
    if !valido {
        return (StatusCode::NOT_FOUND, "não encontrado").into_response();
    }
    let mut pastas = vec![cf_core::paths::data_dir().join("aulas")];
    if let Some(d) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("aulas")))
    {
        pastas.push(d);
    }
    for p in pastas {
        if let Ok(b) = std::fs::read(p.join(nome)) {
            return ([(header::CONTENT_TYPE, "audio/ogg")], b).into_response();
        }
    }
    // Sem arquivo local: usa a gravação embutida no binário.
    match assets::find(&format!("/aulas/{nome}")) {
        Some(a) => ([(header::CONTENT_TYPE, a.mime)], a.body).into_response(),
        None => (StatusCode::NOT_FOUND, "não encontrado").into_response(),
    }
}
