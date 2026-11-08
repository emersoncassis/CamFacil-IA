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
use cf_core::{bench, conectar, onvif, rtsp, system};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

pub fn router(app: Shared) -> Router {
    Router::new()
        .route("/api/system", get(get_system))
        .route("/api/bench", get(get_bench).post(post_bench))
        .route("/api/rtsp/vendors", get(get_vendors))
        .route("/api/rtsp/build", post(post_build))
        .route("/api/rtsp/test", post(post_test))
        .route("/api/discover", get(get_discover))
        .route("/api/procurar", get(get_procurar))
        .route("/api/autoinicio", get(get_autoinicio).post(post_autoinicio))
        .route("/api/conectar", post(post_conectar))
        .route("/api/disclaimer", get(get_disclaimer).post(post_disclaimer))
        .route("/api/video/ffmpeg", get(get_ffmpeg).post(post_ffmpeg))
        .route("/api/video/webcams", get(get_webcams))
        .route("/api/cameras", get(get_cameras).post(post_camera))
        .route("/api/cameras/{id}", delete(delete_camera))
        .route("/api/cameras/{id}/mjpeg", get(get_mjpeg))
        .route("/api/cameras/{id}/foto.jpg", get(get_foto))
        .route(
            "/api/cameras/{id}/sugestoes",
            get(get_sugestoes).post(post_sugestoes),
        )
        .route("/api/cameras/{id}/regras", get(get_regras).post(post_regra))
        .route("/api/regras/{id}", patch(patch_regra).delete(delete_regra))
        .route("/api/estado", get(get_estado))
        .route("/api/eventos", get(get_eventos))
        .route("/api/saude", get(get_saude))
        .route("/api/modulos", get(get_modulos).post(post_modulos))
        .route("/api/ia", get(get_ia).delete(delete_ia))
        .route("/api/ia/baixar", post(post_ia_baixar))
        .route("/api/ia/cancelar", post(post_ia_cancelar))
        .route("/api/ia/permanencia", post(post_ia_permanencia))
        .route(
            "/api/cameras/{id}/calor",
            get(get_calor).delete(delete_calor),
        )
        .route("/api/selos", get(get_selos))
        .route("/api/elo", get(get_elo))
        .route("/api/elo/codigo", post(post_elo_codigo))
        .route("/api/elo/qr.svg", get(get_elo_qr))
        .route("/api/elo/confirmar", post(post_elo_confirmar))
        .route("/api/elo/desparear", post(post_elo_desparear))
        .route("/api/fuso", post(post_fuso))
        .route(
            "/api/telegram",
            get(get_telegram)
                .post(post_telegram)
                .delete(delete_telegram),
        )
        .route("/api/telegram/testar", post(post_telegram_testar))
        .route("/api/telegram/fotos", post(post_telegram_fotos))
        .route(
            "/api/telegram/chats/{id}",
            post(post_telegram_chat).delete(delete_telegram_chat),
        )
        .route("/api/tailscale", get(get_tailscale))
        .route("/api/tailscale/qr.svg", get(get_tailscale_qr))
        .route(
            "/api/rede/foto",
            get(crate::rede::get_foto).post(crate::rede::post_foto),
        )
        .route("/api/rede/fotos/{id}", get(crate::rede::get_foto_id))
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

/// Procura câmeras de dois jeitos ao mesmo tempo: ONVIF e varredura da porta
/// RTSP na rede local. Junta por IP e marca as que já foram adicionadas.
async fn get_procurar(State(app): State<Shared>) -> Json<Value> {
    let (onvif, rtsp) = tokio::join!(
        onvif::discover(Duration::from_secs(3)),
        conectar::varrer_rtsp(554)
    );
    let ja: Vec<String> = app
        .store
        .cameras()
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.url)
        .collect();
    let mut lista: Vec<Value> = Vec::new();
    let mut ips: Vec<String> = onvif.iter().map(|d| d.ip.clone()).collect();
    for ip in rtsp {
        if !ips.contains(&ip) {
            ips.push(ip);
        }
    }
    ips.sort_by_key(|ip| ip.parse::<std::net::Ipv4Addr>().map(u32::from).unwrap_or(0));
    for ip in ips {
        let d = onvif.iter().find(|d| d.ip == ip);
        let marca = format!("@{ip}:");
        lista.push(json!({
            "ip": ip,
            "fabricante": d.and_then(|d| d.fabricante.clone()),
            "modelo": d.and_then(|d| d.modelo.clone().or(d.nome.clone())),
            "mac": d.and_then(|d| d.mac.clone()),
            "ja_adicionada": ja.iter().any(|u| u.contains(&marca)),
        }));
    }
    Json(json!({
        "cameras": lista,
        "rede": conectar::ip_local().map(|i| i.to_string()),
    }))
}

#[derive(Deserialize)]
struct PedidoConectar {
    ip: String,
    #[serde(default = "porta_rtsp")]
    porta: u16,
    #[serde(default)]
    usuario: String,
    #[serde(default)]
    senha: String,
    #[serde(default)]
    fabricante: Option<String>,
    #[serde(default)]
    nome: Option<String>,
}

fn porta_rtsp() -> u16 {
    554
}

/// Um clique: confere a senha, acha o caminho do vídeo e já adiciona a câmera.
async fn post_conectar(State(app): State<Shared>, Json(p): Json<PedidoConectar>) -> ApiResult {
    let c = conectar::conectar(
        &p.ip,
        p.porta,
        p.usuario.trim(),
        &p.senha,
        p.fabricante.as_deref(),
    )
    .await
    .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    let nome: String = p
        .nome
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("Câmera {}", p.ip.trim()))
        .trim()
        .chars()
        .take(60)
        .collect();
    let id = app
        .store
        .criar_camera(&nome, "rtsp", &c.url)
        .map_err(internal)?;
    app.video.iniciar(id, "rtsp", &c.url);
    Ok(Json(json!({
        "id": id,
        "nome": nome,
        "fabricante": c.fabricante,
        "endereco": c.url_sem_senha,
    })))
}

async fn get_autoinicio() -> Json<Value> {
    let ligado = tokio::task::spawn_blocking(crate::autoinicio::ligado)
        .await
        .unwrap_or(false);
    Json(json!({ "ligado": ligado }))
}

#[derive(Deserialize)]
struct PedidoAutoinicio {
    ligado: bool,
}

async fn post_autoinicio(Json(p): Json<PedidoAutoinicio>) -> ApiResult {
    tokio::task::spawn_blocking(move || crate::autoinicio::definir(p.ligado))
        .await
        .map_err(internal)?
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "ligado": p.ligado })))
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

// ---------- Acesso de fora pelo Tailscale (ADR-0005) ----------

pub(crate) fn url_tailscale(app: &Shared, ip: std::net::Ipv4Addr) -> String {
    let porta = app
        .store
        .get("port")
        .ok()
        .flatten()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(7070);
    format!("http://{ip}:{porta}")
}

async fn get_tailscale(State(app): State<Shared>) -> Json<Value> {
    let s = crate::tailscale::situacao().await;
    let escutando = app.tailscale.escutando();
    let lan = std::env::args().any(|a| a == "--lan");
    let url = s.ip.map(|ip| url_tailscale(&app, ip));
    let url_nome = s
        .nome
        .as_ref()
        .zip(s.ip)
        .map(|(n, ip)| url_tailscale(&app, ip).replace(&ip.to_string(), n));
    // Logado mas ainda sem escutar nesse IP: a vigília liga em até 1 minuto.
    let aguardando = s.ip.is_some() && !lan && escutando != s.ip;
    Json(json!({
        "instalado": s.instalado,
        "logado": s.logado,
        "ip": s.ip,
        "nome": s.nome,
        "url": url,
        "url_nome": url_nome,
        "escutando": escutando.is_some() || (lan && s.ip.is_some()),
        "aguardando": aguardando,
        "lan": lan,
        "erro": s.erro,
        "comando_instalar": crate::tailscale::comando_instalar(),
        "sistema": std::env::consts::OS,
    }))
}

async fn get_tailscale_qr(State(app): State<Shared>) -> Response {
    let svg = crate::tailscale::ip()
        .await
        .map(|ip| url_tailscale(&app, ip))
        .and_then(|u| crate::elo::qr_svg(&u));
    match svg {
        Some(s) => (
            [
                (header::CONTENT_TYPE, "image/svg+xml"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            s,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "Tailscale não conectado").into_response(),
    }
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

async fn post_ffmpeg() -> ApiResult {
    tokio::task::spawn_blocking(crate::video::instalar_ffmpeg)
        .await
        .map_err(internal)?
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "instalado": true })))
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

#[derive(Deserialize, Default)]
struct PedidoSugestao {
    #[serde(default)]
    segundos: Option<u64>,
}

/// Começa a observar a câmera para sugerir regras.
async fn post_sugestoes(
    State(app): State<Shared>,
    Path(id): Path<i64>,
    body: Option<Json<PedidoSugestao>>,
) -> ApiResult {
    let canal = app
        .video
        .canal(id)
        .ok_or_else(|| bad(StatusCode::NOT_FOUND, "câmera não encontrada"))?;
    let seg = body.and_then(|b| b.0.segundos).unwrap_or(60).clamp(15, 600);
    *canal.observacao.lock().unwrap() = Some(crate::video::Observacao {
        obs: cf_core::sugestao::Observador::new(),
        inicio: std::time::Instant::now(),
        duracao: Duration::from_secs(seg),
    });
    Ok(Json(json!({ "observando": true, "segundos": seg })))
}

/// Andamento da observação e as sugestões (parciais ou finais).
async fn get_sugestoes(State(app): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    let canal = app
        .video
        .canal(id)
        .ok_or_else(|| bad(StatusCode::NOT_FOUND, "câmera não encontrada"))?;
    let g = canal.observacao.lock().unwrap();
    let Some(o) = g.as_ref() else {
        return Ok(Json(json!({ "iniciado": false })));
    };
    let passou = o.inicio.elapsed();
    let terminou = passou >= o.duracao;
    Ok(Json(json!({
        "iniciado": true,
        "terminou": terminou,
        "restante_s": o.duracao.saturating_sub(passou).as_secs(),
        "progresso": (passou.as_secs_f32() / o.duracao.as_secs_f32()).min(1.0),
        "pessoas": o.obs.pessoas(),
        "sugestoes": o.obs.sugerir(),
    })))
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

/// Saúde das câmeras para a tela Início: situação atual de cada uma e os
/// últimos eventos de saúde (mudanças confirmadas, sem spam).
async fn get_saude(State(app): State<Shared>) -> ApiResult {
    let cams = app.store.cameras().map_err(internal)?;
    let nome_de = |id: i64| {
        cams.iter()
            .find(|c| c.id == id)
            .map(|c| c.nome.clone())
            .unwrap_or_else(|| format!("Câmera {id}"))
    };
    let lista: Vec<Value> = cams
        .iter()
        .filter(|c| c.ativa)
        .map(|c| {
            let e = app
                .video
                .canal(c.id)
                .map(|k| k.estado())
                .unwrap_or_default();
            json!({
                "id": c.id,
                "nome": c.nome,
                "online": e.online,
                "saude": e.saude,
                "saude_desde_s": e.saude_desde_s,
                "ultimo_problema_s": e.ultimo_problema_s,
            })
        })
        .collect();
    let eventos: Vec<Value> = app
        .store
        .eventos_tipo("saude", 20)
        .map_err(internal)?
        .into_iter()
        .map(|e| {
            json!({
                "camera": nome_de(e.camera_id),
                "situacao": e.detalhe,
                "quando": e.quando,
            })
        })
        .collect();
    Ok(Json(json!({
        "ligado": crate::video::modulos(&app.store).saude,
        "cameras": lista,
        "eventos": eventos,
    })))
}

async fn get_modulos(State(app): State<Shared>) -> Json<Value> {
    let m = crate::video::modulos(&app.store);
    let perm = app
        .store
        .get("permanencia_s")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60);
    Json(json!({
        "movimento": m.movimento, "cerca": m.cerca, "saude": m.saude,
        "pessoas": m.pessoas, "contagem": m.contagem, "permanencia": m.permanencia, "calor": m.calor,
        "permanencia_s": perm,
        "ia_pronta": app.ia.pronto(),
    }))
}

// ---------- Nível médio: runtime, modelo e detector ----------

fn estado_ia(app: &Shared) -> Value {
    let a = app.ia.andamento();
    json!({
        "pronto": app.ia.pronto(),
        "suportado": crate::ia::pacote_ort().is_some(),
        "plataforma": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        "onnxruntime": { "instalado": app.ia.lib_ort().is_some(), "versao": crate::ia::ORT_VERSAO },
        "modelo": { "instalado": app.ia.modelo().is_some(), "nome": crate::ia::MODELO_NOME },
        "baixando": app.ia.baixando(),
        "andamento": a,
        "detector": app.ia.detector.lock().unwrap().clone(),
        "pasta": app.ia.dir().display().to_string(),
    })
}

async fn get_ia(State(app): State<Shared>) -> Json<Value> {
    Json(estado_ia(&app))
}

/// Começa o download (runtime + modelo) em segundo plano.
async fn post_ia_baixar(State(app): State<Shared>) -> ApiResult {
    if crate::ia::pacote_ort().is_none() {
        return Err(bad(
            StatusCode::UNPROCESSABLE_ENTITY,
            "não há ONNX Runtime oficial para este sistema",
        ));
    }
    if !app.ia.pronto() && !app.ia.baixando() {
        let ia = app.ia.clone();
        tokio::spawn(async move {
            ia.baixar().await;
        });
    }
    Ok(Json(estado_ia(&app)))
}

async fn post_ia_cancelar(State(app): State<Shared>) -> Json<Value> {
    app.ia.cancelar();
    Json(estado_ia(&app))
}

async fn delete_ia(State(app): State<Shared>) -> ApiResult {
    app.ia.apagar().map_err(|e| bad(StatusCode::CONFLICT, e))?;
    Ok(Json(estado_ia(&app)))
}

#[derive(Deserialize)]
struct PermanenciaReq {
    segundos: u64,
}

async fn post_ia_permanencia(
    State(app): State<Shared>,
    Json(r): Json<PermanenciaReq>,
) -> ApiResult {
    let s = r.segundos.clamp(5, 3600);
    app.store
        .set("permanencia_s", &s.to_string())
        .map_err(internal)?;
    Ok(Json(json!({ "permanencia_s": s })))
}

/// Mapa de calor acumulado (grade), só para o técnico.
async fn get_calor(State(app): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    let canal = app
        .video
        .canal(id)
        .ok_or_else(|| bad(StatusCode::NOT_FOUND, "câmera não encontrada"))?;
    let m = canal.medio.lock().unwrap();
    Ok(Json(
        json!({ "calor": m.rastreador.calor, "ligado": crate::video::modulos(&app.store).calor }),
    ))
}

async fn delete_calor(State(app): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    let canal = app
        .video
        .canal(id)
        .ok_or_else(|| bad(StatusCode::NOT_FOUND, "câmera não encontrada"))?;
    canal.medio.lock().unwrap().rastreador.calor.limpar();
    Ok(Json(json!({ "limpo": true })))
}

#[derive(Deserialize)]
struct ModuloReq {
    modulo: String,
    ligado: bool,
}

async fn post_modulos(State(app): State<Shared>, Json(r): Json<ModuloReq>) -> ApiResult {
    crate::video::definir_modulo(&app.store, &r.modulo, r.ligado)
        .map_err(|e| bad(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(json!({ "modulo": r.modulo, "ligado": r.ligado })))
}

// ---------- Telegram (SDD-04) ----------

#[derive(Deserialize)]
struct FusoReq {
    /// Minutos a somar ao UTC (ex.: -180 para Brasília).
    minutos: i64,
}

/// A dashboard informa o fuso do navegador; o bot usa para escrever a hora.
async fn post_fuso(State(app): State<Shared>, Json(f): Json<FusoReq>) -> ApiResult {
    app.store
        .set("fuso_min", &f.minutos.clamp(-840, 840).to_string())
        .map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

fn telegram(app: &Shared) -> Result<&Arc<cf_notify::Telegram>, (StatusCode, Json<Value>)> {
    app.telegram.get().ok_or_else(|| {
        bad(
            StatusCode::SERVICE_UNAVAILABLE,
            "Telegram indisponível (não foi possível abrir o cofre de segredos)",
        )
    })
}

async fn get_telegram(State(app): State<Shared>) -> ApiResult {
    let tg = telegram(&app)?;
    let chats = app.store.telegram_chats().map_err(internal)?;
    Ok(Json(json!({
        "situacao": tg.situacao(),
        "chats": chats,
    })))
}

#[derive(Deserialize)]
struct TokenReq {
    token: String,
}

/// Salva o token (cifrado) depois de confirmar com o Telegram que ele vale.
async fn post_telegram(State(app): State<Shared>, Json(r): Json<TokenReq>) -> ApiResult {
    let tg = telegram(&app)?;
    let eu = tg
        .definir_token(&r.token)
        .await
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "bot": eu.username, "nome": eu.first_name })))
}

async fn delete_telegram(State(app): State<Shared>) -> ApiResult {
    telegram(&app)?.remover_token().map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
}

/// Testa o token salvo e manda uma mensagem aos contatos autorizados.
async fn post_telegram_testar(State(app): State<Shared>) -> ApiResult {
    let tg = telegram(&app)?;
    let eu = tg
        .testar()
        .await
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    let enviados = tg.enviar_teste().await.unwrap_or(0);
    Ok(Json(json!({ "bot": eu.username, "enviados": enviados })))
}

#[derive(Deserialize)]
struct FotosReq {
    ligado: bool,
}

async fn post_telegram_fotos(State(app): State<Shared>, Json(r): Json<FotosReq>) -> ApiResult {
    telegram(&app)?.definir_fotos(r.ligado).map_err(internal)?;
    Ok(Json(json!({ "fotos": r.ligado })))
}

#[derive(Deserialize)]
struct ChatReq {
    autorizado: bool,
}

async fn post_telegram_chat(
    State(app): State<Shared>,
    Path(id): Path<i64>,
    Json(r): Json<ChatReq>,
) -> ApiResult {
    if !app
        .store
        .telegram_autorizar(id, r.autorizado)
        .map_err(internal)?
    {
        return Err(bad(StatusCode::NOT_FOUND, "contato não encontrado"));
    }
    Ok(Json(json!({ "chat_id": id, "autorizado": r.autorizado })))
}

async fn delete_telegram_chat(State(app): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    if !app.store.telegram_remover(id).map_err(internal)? {
        return Err(bad(StatusCode::NOT_FOUND, "contato não encontrado"));
    }
    Ok(Json(json!({ "removido": true })))
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
