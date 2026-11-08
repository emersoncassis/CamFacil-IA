//! Rotas HTTP da fase 1.

use crate::{arquivo, assets, integracoes, termos, Shared};
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
        .route("/api/gravador/adicionar", post(post_gravador_adicionar))
        .route(
            "/api/login-cameras",
            get(get_login).post(post_login).delete(delete_login),
        )
        .route("/api/disclaimer", get(get_disclaimer).post(post_disclaimer))
        .route("/api/video/ffmpeg", get(get_ffmpeg).post(post_ffmpeg))
        .route("/api/video/webcams", get(get_webcams))
        .route("/api/cameras", get(get_cameras).post(post_camera))
        .route(
            "/api/cameras/{id}",
            patch(patch_camera).delete(delete_camera),
        )
        .route("/api/cameras/{id}/mjpeg", get(get_mjpeg))
        .route("/api/cameras/{id}/foto.jpg", get(get_foto))
        .route("/api/cameras/{id}/nomes-canais", get(get_nomes_canais))
        .route(
            "/api/cameras/{id}/sugestoes",
            get(get_sugestoes)
                .post(post_sugestoes)
                .delete(delete_sugestoes),
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
        .route(
            "/api/whatsapp",
            get(integracoes::get_whatsapp)
                .post(integracoes::post_whatsapp)
                .delete(integracoes::delete_whatsapp),
        )
        .route(
            "/api/whatsapp/testar",
            post(integracoes::post_whatsapp_testar),
        )
        .route("/api/whatsapp/qr", post(integracoes::post_whatsapp_qr))
        .route(
            "/api/whatsapp/fotos",
            post(integracoes::post_whatsapp_fotos),
        )
        .route(
            "/api/whatsapp/webhook/registrar",
            post(integracoes::post_whatsapp_webhook_registrar),
        )
        .route(
            "/api/whatsapp/webhook/{token}",
            post(integracoes::post_whatsapp_webhook),
        )
        .route(
            "/api/whatsapp/contatos/{jid}",
            post(integracoes::post_whatsapp_contato).delete(integracoes::delete_whatsapp_contato),
        )
        .route(
            "/api/webhook",
            get(integracoes::get_webhook)
                .post(integracoes::post_webhook)
                .delete(integracoes::delete_webhook),
        )
        .route(
            "/api/webhook/testar",
            post(integracoes::post_webhook_testar),
        )
        .route(
            "/api/arquivo",
            get(arquivo::get_arquivo).post(arquivo::post_arquivo),
        )
        .route("/api/arquivo/{nome}", get(arquivo::get_foto_arquivo))
        .route("/api/termos", get(termos::get_termos))
        .route("/api/termos/tecnico", post(termos::post_termos_tecnico))
        .route("/api/termos/cliente", post(termos::post_termos_cliente))
        .route("/api/termos/cliente.md", get(termos::get_termos_cliente_md))
        .route("/api/termos/placa.md", get(termos::get_termos_placa_md))
        .route("/api/termos/apagar", post(termos::post_termos_apagar))
        .route("/api/tailscale", get(get_tailscale))
        .route("/api/tailscale/qr.svg", get(get_tailscale_qr))
        // Fase 5 (Manutenção): módulos próprios.
        .route("/tv", get(crate::tv::pagina))
        .route(
            "/api/tv",
            get(crate::tv::get_config).post(crate::tv::post_config),
        )
        .route("/api/tv/estado", get(crate::tv::get_estado))
        .route(
            "/api/tv/logo",
            post(crate::tv::post_logo).delete(crate::tv::delete_logo),
        )
        .route("/api/tv/logo.png", get(crate::tv::get_logo))
        .route("/api/tv/whatsapp.svg", get(crate::tv::get_whatsapp_qr))
        .route("/api/hardware", get(crate::hardware::get_atual))
        .route("/api/hardware/log", get(crate::hardware::get_log))
        .route(
            "/api/relatorio",
            get(crate::relatorio::get_lista).post(crate::relatorio::post_gerar),
        )
        .route(
            "/api/relatorio/{arquivo}",
            get(crate::relatorio::get_baixar).delete(crate::relatorio::delete_relatorio),
        )
        .route(
            "/api/relatorio/{arquivo}/telegram",
            post(crate::relatorio::post_telegram),
        )
        .route(
            "/api/estresse",
            get(crate::estresse::get_estado)
                .post(crate::estresse::post_iniciar)
                .delete(crate::estresse::delete_cancelar),
        )
        .route(
            "/api/estudio/modelos",
            get(crate::estudio::get_modelos).post(crate::estudio::post_modelo),
        )
        .route(
            "/api/estudio/modelos/{id}",
            delete(crate::estudio::delete_modelo),
        )
        .route(
            "/api/estudio/modelos/{id}/ligar",
            post(crate::estudio::post_ligar),
        )
        .route(
            "/api/estudio/coleta",
            get(crate::estudio::get_coleta).post(crate::estudio::post_coleta),
        )
        .route(
            "/api/estudio/coleta/parar",
            post(crate::estudio::post_coleta_parar),
        )
        .route(
            "/api/estudio/coleta/{receita}",
            delete(crate::estudio::delete_coleta),
        )
        .route(
            "/api/estudio/modelos/{id}/onnx",
            axum::routing::put(crate::estudio::put_onnx).layer(
                axum::extract::DefaultBodyLimit::max(crate::estudio::ONNX_MAX + 1024),
            ),
        )
        .route("/api/dados", get(get_dados))
        .route("/api/dados/apagar-tudo", post(post_apagar_tudo))
        .route("/api/backup", post(crate::backup::post_exportar))
        .route("/api/backup/telegram", post(crate::backup::post_telegram))
        .route(
            "/api/backup/importar",
            post(crate::backup::post_importar).layer(axum::extract::DefaultBodyLimit::max(
                crate::backup::TAMANHO_MAX + 1024,
            )),
        )
        .route(
            "/api/rede/foto",
            get(crate::rede::get_foto).post(crate::rede::post_foto),
        )
        .route("/api/rede/fotos/{id}", get(crate::rede::get_foto_id))
        .route(
            "/api/rede/diagnostico",
            get(crate::rede::get_diagnostico).post(crate::rede::post_diagnostico),
        )
        .route("/api/rede/resumo", get(crate::rede::get_resumo))
        .route("/api/rede/diagrama.svg", get(crate::rede::get_diagrama))
        .route(
            "/api/rede/cameras/{id}/atualizar",
            post(crate::rede::post_atualizar),
        )
        // Fase 6: MCP (desligado por padrão) e botões de portão/relé.
        .route("/mcp", post(crate::mcp::post_mcp).get(crate::mcp::get_mcp))
        .route(
            "/api/mcp",
            get(crate::mcp::get_config).post(crate::mcp::post_config),
        )
        .route("/api/mcp/token", post(crate::mcp::post_token))
        .route("/botoes", get(crate::botoes::pagina))
        .route(
            "/api/botoes",
            get(crate::botoes::get_botoes).post(crate::botoes::post_botao),
        )
        .route("/api/botoes/pin", post(crate::botoes::post_pin))
        .route("/api/botoes/log", get(crate::botoes::get_log))
        .route("/api/botoes/{id}", delete(crate::botoes::delete_botao))
        .route(
            "/api/botoes/{id}/preparar",
            post(crate::botoes::post_preparar),
        )
        .route(
            "/api/botoes/{id}/acionar",
            post(crate::botoes::post_acionar),
        )
        .fallback(static_file)
        .with_state(app)
}

pub(crate) type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

pub(crate) fn bad(status: StatusCode, msg: impl ToString) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "erro": msg.to_string() })))
}

pub(crate) fn internal<E: std::fmt::Display>(e: E) -> (StatusCode, Json<Value>) {
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
    /// Usar o login salvo (o mesmo das outras câmeras) em vez de usuário/senha.
    #[serde(default)]
    usar_salvo: bool,
    /// Corrigir o IP sozinho se a câmera mudar de endereço (padrão desligado).
    #[serde(default)]
    auto_ip: bool,
    /// MAC visto na busca (ONVIF), quando houver.
    #[serde(default)]
    mac: Option<String>,
    /// Modelo visto na busca (ONVIF): ajuda a reconhecer gravador.
    #[serde(default)]
    modelo: Option<String>,
}

fn porta_rtsp() -> u16 {
    554
}

/// Um clique: confere a senha, acha o caminho do vídeo e já adiciona a câmera.
async fn post_conectar(State(app): State<Shared>, Json(p): Json<PedidoConectar>) -> ApiResult {
    let (usuario, senha) = if p.usar_salvo {
        login_salvo(&app).ok_or_else(|| {
            bad(
                StatusCode::CONFLICT,
                "não há login salvo; digite usuário e senha",
            )
        })?
    } else {
        (p.usuario.trim().to_string(), p.senha.clone())
    };
    let c = conectar::conectar(&p.ip, p.porta, &usuario, &senha, p.fabricante.as_deref())
        .await
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    // Gravador (DVR/NVR)? Então o técnico escolhe os canais antes de criar.
    let det = cf_core::gravador::detectar(
        p.ip.trim(),
        p.porta,
        &usuario,
        &senha,
        &c.url,
        p.modelo.as_deref(),
    )
    .await;
    if det.gravador {
        let token = guardar_gravador(GravadorPendente {
            url: c.url.clone(),
            mac: p.mac.clone(),
            criado: std::time::Instant::now(),
        });
        let nome = p
            .nome
            .clone()
            .filter(|n| !n.trim().is_empty())
            // Sem IP no nome: os canais viram "Gravador N · Canal N".
            .unwrap_or_else(|| nome_livre(&app, "Gravador"));
        return Ok(Json(json!({
            "tipo": "gravador",
            "token": token,
            "nome": nome,
            "fabricante": c.fabricante,
            "endereco": c.url_sem_senha,
            "origem": det.origem,
            "canais": det.canais,
        })));
    }
    let nome: String = p
        .nome
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| nome_livre(&app, "Câmera"))
        .trim()
        .chars()
        .take(60)
        .collect();
    let id = app
        .store
        .criar_camera(&nome, "rtsp", &c.url)
        .map_err(internal)?;
    if p.auto_ip {
        app.store.definir_auto_ip(id, true).map_err(internal)?;
    }
    app.video.iniciar(id, "rtsp", &c.url);
    // Aprende o MAC já: a conexão que acabou de dar certo deixa o IP na ARP.
    let host = cf_core::rede::host_da_url(&c.url).map(|(h, _)| h);
    let mut mac = match &host {
        Some(h) => crate::rede::aprender_mac_agora(&app, id, h).await,
        None => None,
    };
    if mac.is_none() {
        if let Some(m) = p.mac.as_deref().and_then(cf_core::rede::normalizar_mac) {
            let _ = app.store.definir_mac_camera(id, &m);
            mac = Some(m);
        }
    }
    Ok(Json(json!({
        "tipo": "camera",
        "id": id,
        "nome": nome,
        "fabricante": c.fabricante,
        "endereco": c.url_sem_senha,
        "auto_ip": p.auto_ip,
        "mac_conhecido": mac.is_some(),
    })))
}

/// `GET /api/cameras/{id}/nomes-canais`: nome real de cada canal do gravador
/// de onde a câmera veio (para sugerir o nome ao trocar de canal). Teto de
/// ≈3 s; se o gravador não responder, a lista vem vazia.
async fn get_nomes_canais(State(app): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    let c = app
        .store
        .camera(id)
        .map_err(internal)?
        .ok_or_else(|| bad(StatusCode::NOT_FOUND, "câmera não encontrada"))?;
    let fab = rtsp::fabricante_do_caminho(&c.url);
    let host = cf_core::rede::host_da_url(&c.url).map(|(h, _)| h);
    let nomes = match (c.fonte.as_str(), fab, host) {
        ("rtsp", Some(fab), Some(host)) if c.canal > 0 => {
            let (u, s) = rtsp::credenciais(&c.url);
            let (http, onvif) = tokio::join!(
                tokio::time::timeout(
                    cf_core::nomes_canais::LIMITE,
                    cf_core::nomes_canais::buscar_http(&host, 80, &u, &s, fab),
                ),
                tokio::time::timeout(
                    cf_core::nomes_canais::LIMITE,
                    cf_core::gravador::canais_onvif(&host, 80, &u, &s),
                ),
            );
            let onvif = cf_core::nomes_canais::de_onvif(&onvif.ok().flatten().unwrap_or_default());
            cf_core::nomes_canais::juntar(&[&http.unwrap_or_default(), &onvif])
        }
        _ => Default::default(),
    };
    Ok(Json(json!({ "nomes": nomes })))
}

// ---------- Gravador (DVR/NVR): escolher os canais ----------

/// Gravador já conferido esperando o técnico escolher os canais. A senha
/// fica só aqui, na memória, por no máximo 15 min.
struct GravadorPendente {
    url: String,
    mac: Option<String>,
    criado: std::time::Instant,
}

const GRAVADOR_VALIDADE: Duration = Duration::from_secs(15 * 60);

fn gravadores_pendentes(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, GravadorPendente>> {
    static P: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, GravadorPendente>>,
    > = std::sync::OnceLock::new();
    P.get_or_init(Default::default)
}

fn guardar_gravador(g: GravadorPendente) -> String {
    let token: String = cf_notify::cofre::aleatorio(16)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let mut m = gravadores_pendentes()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    m.retain(|_, v| v.criado.elapsed() < GRAVADOR_VALIDADE);
    m.insert(token.clone(), g);
    token
}

#[derive(Deserialize)]
struct CanalEscolhido {
    canal: u32,
    #[serde(default)]
    nome: Option<String>,
}

#[derive(Deserialize)]
struct PedidoGravador {
    token: String,
    #[serde(default)]
    nome: String,
    canais: Vec<CanalEscolhido>,
    #[serde(default)]
    stream: rtsp::Stream,
    #[serde(default)]
    auto_ip: bool,
}

/// Câmeras antigas com nome padrão "Câmera 192.168.0.201": troca por
/// "Câmera N" (o IP não vai no nome). Roda uma vez no início.
pub(crate) fn tirar_ip_dos_nomes(store: &cf_core::store::Store) {
    let Ok(cams) = store.cameras() else { return };
    let mut usados: std::collections::HashSet<String> =
        cams.iter().map(|c| c.nome.clone()).collect();
    for mut c in cams {
        let Some((host, _)) = cf_core::rede::host_da_url(&c.url) else {
            continue;
        };
        if c.nome != format!("Câmera {host}") {
            continue;
        }
        let Some(novo) = (1..)
            .map(|n| format!("Câmera {n}"))
            .find(|n| !usados.contains(n))
        else {
            continue;
        };
        usados.insert(novo.clone());
        c.nome = novo;
        let _ = store.atualizar_camera(&c);
    }
}

/// Nome padrão sem IP: "Câmera 3" (o primeiro número que ainda não existe).
fn nome_livre(app: &Shared, base: &str) -> String {
    let nomes: std::collections::HashSet<String> = app
        .store
        .cameras()
        .map(|v| v.into_iter().map(|c| c.nome).collect())
        .unwrap_or_default();
    (1..)
        .map(|n| format!("{base} {n}"))
        .find(|n| {
            !nomes
                .iter()
                .any(|x| x == n || x.starts_with(&format!("{n} · ")))
        })
        .unwrap_or_else(|| base.to_string())
}

/// Nome de cada câmera vinda de gravador: "<gravador> · Canal N".
fn nome_canal(gravador: &str, canal: u32, proprio: Option<&str>) -> String {
    let n = proprio
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(String::from)
        .unwrap_or_else(|| format!("{} · Canal {canal}", gravador.trim()));
    n.chars().take(60).collect()
}

/// `POST /api/gravador/adicionar`: cada canal escolhido vira uma câmera.
async fn post_gravador_adicionar(
    State(app): State<Shared>,
    Json(p): Json<PedidoGravador>,
) -> ApiResult {
    if p.canais.is_empty() {
        return Err(bad(StatusCode::BAD_REQUEST, "escolha pelo menos um canal"));
    }
    if p.canais.len() > cf_core::gravador::MAX_CANAIS as usize
        || p.canais
            .iter()
            .any(|c| c.canal == 0 || c.canal > cf_core::gravador::MAX_CANAIS)
    {
        return Err(bad(StatusCode::BAD_REQUEST, "canal inválido"));
    }
    let (url, mac_busca) = {
        let m = gravadores_pendentes()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let g = m
            .get(&p.token)
            .filter(|g| g.criado.elapsed() < GRAVADOR_VALIDADE)
            .ok_or_else(|| {
                bad(
                    StatusCode::GONE,
                    "a conexão com o gravador expirou; conecte de novo",
                )
            })?;
        (g.url.clone(), g.mac.clone())
    };
    let host = cf_core::rede::host_da_url(&url)
        .map(|(h, _)| h)
        .unwrap_or_default();
    let base = if p.nome.trim().is_empty() {
        nome_livre(&app, "Gravador")
    } else {
        p.nome.trim().to_string()
    };
    let mut criadas = Vec::new();
    let mut vistos = std::collections::BTreeSet::new();
    for c in p.canais.iter().filter(|c| vistos.insert(c.canal)) {
        let Some(u) = rtsp::url_do_canal(&url, c.canal, p.stream) else {
            return Err(bad(
                StatusCode::BAD_REQUEST,
                "endereço do gravador sem canal",
            ));
        };
        let nome = nome_canal(&base, c.canal, c.nome.as_deref());
        let id = app
            .store
            .criar_camera(&nome, "rtsp", &u)
            .map_err(internal)?;
        app.store
            .definir_gravador_canal(id, &host, c.canal)
            .map_err(internal)?;
        if p.auto_ip {
            app.store.definir_auto_ip(id, true).map_err(internal)?;
        }
        app.video.iniciar(id, "rtsp", &u);
        criadas.push((id, nome));
    }
    // O MAC é do gravador: vale para todos os canais.
    let mut mac = match criadas.first() {
        Some((id, _)) => crate::rede::aprender_mac_agora(&app, *id, &host).await,
        None => None,
    };
    if mac.is_none() {
        mac = mac_busca.as_deref().and_then(cf_core::rede::normalizar_mac);
    }
    if let Some(m) = &mac {
        for (id, _) in &criadas {
            let _ = app.store.definir_mac_camera(*id, m);
        }
    }
    gravadores_pendentes()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&p.token);
    Ok(Json(json!({
        "cameras": criadas.iter().map(|(id, nome)| json!({ "id": id, "nome": nome })).collect::<Vec<_>>(),
        "auto_ip": p.auto_ip,
        "mac_conhecido": mac.is_some(),
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

// ---------- Dados do programa (pasta ao lado do executável) ----------

async fn get_dados() -> Json<Value> {
    let dir = cf_core::paths::data_dir();
    let antiga = cf_core::paths::pasta_antiga_existente();
    Json(json!({
        "pasta": dir.display().to_string(),
        "mb": cf_core::paths::tamanho(&dir) as f64 / 1_048_576.0,
        "antiga": antiga.as_ref().map(|p| p.display().to_string()),
        "antiga_mb": antiga.as_ref().map(|p| cf_core::paths::tamanho(p) as f64 / 1_048_576.0),
    }))
}

#[derive(Deserialize)]
struct PedidoApagarTudo {
    confirmar: String,
}

/// Apaga TUDO: câmeras, regras, histórico, fotos, logins, chaves e a pasta
/// antiga. O banco está aberto, então marca e fecha o programa; ao abrir de
/// novo, ele apaga e começa do zero. Também desliga o "abrir com o computador".
async fn post_apagar_tudo(Json(p): Json<PedidoApagarTudo>) -> ApiResult {
    if p.confirmar.trim() != "APAGAR" {
        return Err(bad(StatusCode::BAD_REQUEST, "digite APAGAR para confirmar"));
    }
    let _ = crate::autoinicio::definir(false);
    let marca = cf_core::paths::data_dir().join(cf_core::paths::MARCA_APAGAR);
    std::fs::write(&marca, b"1").map_err(internal)?;
    tokio::spawn(async {
        tokio::time::sleep(Duration::from_millis(800)).await;
        std::process::exit(0);
    });
    Ok(Json(json!({ "ok": true })))
}

// ---------- Login padrão das câmeras (o mesmo para todas) ----------
// Guardado cifrado (cofre do cf-notify), nunca devolvido à tela: a tela só
// sabe se existe e qual é o usuário.

const LOGIN_CHAVE: &str = "login_cameras";

fn login_salvo(app: &Shared) -> Option<(String, String)> {
    let guardado = app.store.get(LOGIN_CHAVE).ok().flatten()?;
    if guardado.is_empty() {
        return None;
    }
    let claro = cf_notify::cofre::Cofre::abrir().ok()?.decifrar(&guardado)?;
    let v: Value = serde_json::from_str(&claro).ok()?;
    Some((
        v.get("u")?.as_str()?.to_string(),
        v.get("s")?.as_str()?.to_string(),
    ))
}

async fn get_login(State(app): State<Shared>) -> Json<Value> {
    let l = login_salvo(&app);
    Json(json!({ "salvo": l.is_some(), "usuario": l.map(|x| x.0) }))
}

#[derive(Deserialize)]
struct PedidoLogin {
    usuario: String,
    senha: String,
}

async fn post_login(State(app): State<Shared>, Json(p): Json<PedidoLogin>) -> ApiResult {
    let usuario = p.usuario.trim();
    if usuario.is_empty() {
        return Err(bad(StatusCode::BAD_REQUEST, "informe o usuário"));
    }
    let cofre = cf_notify::cofre::Cofre::abrir().map_err(internal)?;
    let claro = json!({ "u": usuario, "s": p.senha }).to_string();
    app.store
        .set(LOGIN_CHAVE, &cofre.cifrar(&claro))
        .map_err(internal)?;
    Ok(Json(json!({ "ok": true, "usuario": usuario })))
}

async fn delete_login(State(app): State<Shared>) -> ApiResult {
    app.store.set(LOGIN_CHAVE, "").map_err(internal)?;
    Ok(Json(json!({ "ok": true })))
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
                "ativa": c.ativa,
                "estado": estado,
                "auto_ip": c.auto_ip,
                "mac_conhecido": c.mac_conhecido(),
                "gravador": (!c.gravador.is_empty()).then_some(&c.gravador),
                "canal": (c.canal > 0).then_some(c.canal),
                "stream_principal": principal_disponivel(&c.fonte, &c.url),
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
    /// Autocorreção de IP (só vale para RTSP).
    #[serde(default)]
    auto_ip: bool,
}

async fn post_camera(State(app): State<Shared>, Json(n): Json<NovaCamera>) -> ApiResult {
    let nome: String = n.nome.trim().chars().take(60).collect();
    if nome.is_empty() {
        return Err(bad(StatusCode::BAD_REQUEST, "dê um nome para a câmera"));
    }
    validar_fonte(&n.fonte, &n.url).map_err(|m| bad(StatusCode::BAD_REQUEST, m))?;
    let id = app
        .store
        .criar_camera(&nome, &n.fonte, n.url.trim())
        .map_err(internal)?;
    if n.auto_ip && n.fonte == "rtsp" {
        app.store.definir_auto_ip(id, true).map_err(internal)?;
    }
    app.video.iniciar(id, &n.fonte, n.url.trim());
    Ok(Json(json!({ "id": id })))
}

/// Confere se a fonte existe e se o endereço combina com ela.
fn validar_fonte(fonte: &str, url: &str) -> Result<(), &'static str> {
    match fonte {
        "rtsp" if !url.trim().starts_with("rtsp://") => {
            Err("endereço RTSP inválido; use o gerador")
        }
        "arquivo" if url.trim().is_empty() => Err("informe o caminho do arquivo de vídeo"),
        "rtsp" | "arquivo" | "demo" | "webcam" => Ok(()),
        _ => Err("fonte desconhecida"),
    }
}

/// Edição de câmera. Só os campos enviados mudam. A senha nunca volta para a
/// tela: `senha` troca a senha do endereço RTSP só quando vem preenchida.
#[derive(Deserialize, Default)]
struct EdicaoCamera {
    #[serde(default)]
    nome: Option<String>,
    #[serde(default)]
    fonte: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    senha: Option<String>,
    #[serde(default)]
    ativa: Option<bool>,
    /// Autocorreção de IP pelo MAC (só RTSP).
    #[serde(default)]
    auto_ip: Option<bool>,
    /// Troca o canal de uma câmera que veio de gravador.
    #[serde(default)]
    canal: Option<u32>,
}

fn aplicar_edicao(
    atual: &cf_core::store::Camera,
    e: &EdicaoCamera,
) -> Result<cf_core::store::Camera, &'static str> {
    let mut c = atual.clone();
    if let Some(n) = &e.nome {
        c.nome = n.trim().chars().take(60).collect();
        if c.nome.is_empty() {
            return Err("dê um nome para a câmera");
        }
    }
    if let Some(f) = &e.fonte {
        c.fonte = f.trim().to_string();
    }
    if let Some(u) = &e.url {
        let u = u.trim();
        c.url = if c.fonte == "rtsp" && atual.fonte == "rtsp" {
            rtsp::manter_senha(u, &atual.url)
        } else {
            u.to_string()
        };
    }
    if c.fonte == "demo" {
        c.url.clear();
    }
    validar_fonte(&c.fonte, &c.url)?;
    if let Some(s) = e.senha.as_deref().filter(|s| !s.is_empty()) {
        if c.fonte != "rtsp" {
            return Err("senha só vale para câmera RTSP");
        }
        c.url = rtsp::trocar_senha(&c.url, s);
    }
    if let Some(a) = e.ativa {
        c.ativa = a;
    }
    if let Some(a) = e.auto_ip {
        if a && c.fonte != "rtsp" {
            return Err("correção de IP só vale para câmera RTSP");
        }
        c.auto_ip = a;
    }
    if c.fonte != "rtsp" {
        c.auto_ip = false;
    }
    if let Some(n) = e.canal.filter(|n| *n != atual.canal) {
        if atual.canal == 0 || c.fonte != "rtsp" {
            return Err("só câmera que veio de gravador tem canal");
        }
        if n == 0 || n > cf_core::gravador::MAX_CANAIS {
            return Err("canal inválido");
        }
        let stream = rtsp::canal_da_url(&c.url)
            .map(|(_, s)| s)
            .unwrap_or_default();
        c.url = rtsp::url_do_canal(&c.url, n, stream).ok_or("endereço do gravador sem canal")?;
        c.canal = n;
    }
    Ok(c)
}

async fn patch_camera(
    State(app): State<Shared>,
    Path(id): Path<i64>,
    Json(e): Json<EdicaoCamera>,
) -> ApiResult {
    let atual = app
        .store
        .camera(id)
        .map_err(internal)?
        .ok_or_else(|| bad(StatusCode::NOT_FOUND, "câmera não encontrada"))?;
    let nova = aplicar_edicao(&atual, &e).map_err(|m| bad(StatusCode::BAD_REQUEST, m))?;
    if !app.store.atualizar_camera(&nova).map_err(internal)? {
        return Err(bad(StatusCode::NOT_FOUND, "câmera não encontrada"));
    }
    // Endereço apontado para outro aparelho à mão: o MAC aprendido não vale
    // mais (senão a vigília "corrigiria" de volta para o aparelho antigo).
    let host = |u: &str| cf_core::rede::host_da_url(u).map(|(h, _)| h);
    if nova.fonte != "rtsp" || host(&nova.url) != host(&atual.url) {
        let _ = app.store.definir_mac_camera(id, "");
    }
    if nova.canal != atual.canal {
        let _ = app
            .store
            .definir_gravador_canal(id, &nova.gravador, nova.canal);
    } else if nova.fonte != "rtsp" && atual.canal > 0 {
        let _ = app.store.definir_gravador_canal(id, "", 0);
    }
    let nova = app.store.camera(id).map_err(internal)?.unwrap_or(nova);
    // Reinicia o vídeo com o endereço novo (ou desliga, se ficou inativa).
    if nova.ativa {
        app.video.iniciar(id, &nova.fonte, &nova.url);
    } else {
        app.video.parar(id);
    }
    Ok(Json(json!({
        "id": nova.id,
        "nome": nova.nome,
        "fonte": nova.fonte,
        "url_sem_senha": rtsp::redact(&nova.url),
        "ativa": nova.ativa,
        "auto_ip": nova.auto_ip,
        "mac_conhecido": nova.mac_conhecido(),
        "canal": (nova.canal > 0).then_some(nova.canal),
    })))
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

/// Para a observação (Cancelar, ou depois de aceitar uma regra: uma regra por
/// pesquisa) e descarta as sugestões.
async fn delete_sugestoes(State(app): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    let canal = app
        .video
        .canal(id)
        .ok_or_else(|| bad(StatusCode::NOT_FOUND, "câmera não encontrada"))?;
    *canal.observacao.lock().unwrap() = None;
    Ok(Json(json!({ "observando": false })))
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

/// Stream principal para ver ao vivo: `None` = não se aplica (não é RTSP ou
/// a câmera já está no principal); `Some(false)` = RTSP sem padrão
/// conhecido (não dá para derivar o principal); `Some(true)` = disponível.
fn principal_disponivel(fonte: &str, url: &str) -> Option<bool> {
    if fonte != "rtsp" {
        return None;
    }
    match rtsp::canal_da_url(url) {
        Some((_, rtsp::Stream::Principal)) => None,
        Some(_) => Some(rtsp::url_principal(url).is_some()),
        None => Some(false),
    }
}

/// `?stream=principal` nas rotas de imagem ao vivo.
#[derive(Deserialize, Default)]
struct QueryStream {
    #[serde(default)]
    stream: String,
}

/// Leitor do principal pedido pela tela. `Ok(None)`: usar o leitor da IA
/// (pediu o extra, ou a câmera já está no principal).
fn visor_pedido(
    app: &crate::App,
    id: i64,
    q: &QueryStream,
) -> Result<Option<Arc<crate::visor::Visor>>, (StatusCode, &'static str)> {
    if q.stream != "principal" {
        return Ok(None);
    }
    let cam = app
        .store
        .camera(id)
        .ok()
        .flatten()
        .filter(|c| c.ativa && app.video.canal(id).is_some())
        .ok_or((StatusCode::NOT_FOUND, "câmera não encontrada"))?;
    match principal_disponivel(&cam.fonte, &cam.url) {
        None => Ok(None),
        Some(true) => Ok(rtsp::url_principal(&cam.url).map(|u| crate::visor::abrir(id, &u))),
        Some(false) => Err((
            StatusCode::BAD_REQUEST,
            "este endereço RTSP não segue um padrão conhecido; só dá para ver o stream extra",
        )),
    }
}

fn resposta_jpeg(j: Option<Arc<Vec<u8>>>) -> Response {
    match j {
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

async fn get_foto(
    State(app): State<Shared>,
    Path(id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<QueryStream>,
) -> Response {
    match visor_pedido(&app, id, &q) {
        Err(r) => r.into_response(),
        Ok(Some(v)) => {
            v.tocar();
            let ja = v.ultimo.lock().unwrap().clone();
            if ja.is_some() {
                return resposta_jpeg(ja);
            }
            // Leitor recém-aberto: espera o primeiro quadro um pouco.
            let mut rx = v.tx.subscribe();
            let j = tokio::time::timeout(Duration::from_secs(6), rx.recv())
                .await
                .ok()
                .and_then(Result::ok);
            resposta_jpeg(j)
        }
        Ok(None) => resposta_jpeg(
            app.video
                .canal(id)
                .and_then(|c| c.ultimo.lock().unwrap().clone()),
        ),
    }
}

/// Transmissão ao vivo em MJPEG (funciona direto num <img>). Com
/// `?stream=principal`, abre o leitor do principal sob demanda.
async fn get_mjpeg(
    State(app): State<Shared>,
    Path(id): Path<i64>,
    axum::extract::Query(q): axum::extract::Query<QueryStream>,
) -> Response {
    let (rx, primeiro) = match visor_pedido(&app, id, &q) {
        Err(r) => return r.into_response(),
        Ok(Some(v)) => {
            let ultimo = v.ultimo.lock().unwrap().clone();
            (v.tx.subscribe(), ultimo)
        }
        Ok(None) => {
            let Some(canal) = app.video.canal(id) else {
                return (StatusCode::NOT_FOUND, "câmera não encontrada").into_response();
            };
            let ultimo = canal.ultimo.lock().unwrap().clone();
            (canal.tx.subscribe(), ultimo)
        }
    };
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
pub(crate) struct NovaRegra {
    pub nome: String,
    pub forma: String,
    pub pontos: Vec<cf_core::visao::Ponto>,
    #[serde(default)]
    pub rotulo_on: String,
    #[serde(default)]
    pub rotulo_off: String,
}

/// Valida e grava uma regra (usado pela dashboard e pelo MCP).
pub(crate) fn criar_regra(
    app: &crate::App,
    cam: i64,
    r: &NovaRegra,
) -> Result<i64, (StatusCode, String)> {
    let erro = |s: StatusCode, m: &str| (s, m.to_string());
    if app
        .store
        .camera(cam)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .is_none()
    {
        return Err(erro(StatusCode::NOT_FOUND, "câmera não encontrada"));
    }
    let nome: String = r.nome.trim().chars().take(40).collect();
    let min = match r.forma.as_str() {
        "zona" => 3,
        "linha" => 2,
        _ => {
            return Err(erro(
                StatusCode::BAD_REQUEST,
                "forma deve ser zona ou linha",
            ))
        }
    };
    if nome.is_empty() {
        return Err(erro(StatusCode::BAD_REQUEST, "dê um nome para a regra"));
    }
    if r.pontos.len() < min || r.pontos.len() > 24 {
        return Err(erro(StatusCode::BAD_REQUEST, "desenho incompleto"));
    }
    if r.pontos
        .iter()
        .any(|p| !(0.0..=1.0).contains(&p.x) || !(0.0..=1.0).contains(&p.y))
    {
        return Err(erro(StatusCode::BAD_REQUEST, "pontos fora da imagem"));
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    app.video.recarregar_regras(cam);
    Ok(id)
}

async fn post_regra(
    State(app): State<Shared>,
    Path(cam): Path<i64>,
    Json(r): Json<NovaRegra>,
) -> ApiResult {
    let id = criar_regra(&app, cam, &r).map_err(|(s, m)| bad(s, m))?;
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
        "suportado": app.ia.suportado(),
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
    if !app.ia.suportado() {
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
    termos::exigir_tecnico(&app)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use cf_core::store::Store;

    fn app_teste(nome: &str) -> (Shared, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("cf-api-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Arc::new(Store::open(&dir.join("t.db")).unwrap());
        let app = Arc::new(crate::App {
            elo: crate::elo::Elo::carregar(store.clone()),
            video: crate::video::Video::new(store.clone()),
            store,
            tailscale: crate::tailscale::Estado::default(),
            telegram: std::sync::OnceLock::new(),
            whatsapp: std::sync::OnceLock::new(),
            webhook: std::sync::OnceLock::new(),
            arquivo: std::sync::OnceLock::new(),
            ia: Arc::new(crate::ia::Ia::new(&dir)),
            estudio: crate::estudio::Estudio::new(&dir),
            coleta: Arc::default(),
        });
        (app, dir)
    }

    #[test]
    fn principal_para_ver() {
        let d = "rtsp://a:b@h:554/cam/realmonitor?channel=2&subtype=1";
        assert_eq!(principal_disponivel("rtsp", d), Some(true));
        assert_eq!(
            principal_disponivel("rtsp", "rtsp://a:b@h/Streaming/Channels/101"),
            None,
            "já está no principal"
        );
        assert_eq!(
            principal_disponivel("rtsp", "rtsp://a:b@h/live/ch0"),
            Some(false)
        );
        assert_eq!(principal_disponivel("demo", ""), None);
        assert_eq!(nome_canal("NVR Loja", 3, None), "NVR Loja · Canal 3");
        assert_eq!(nome_canal("NVR Loja", 3, Some(" PORTÃO ")), "PORTÃO");
    }

    #[tokio::test]
    async fn stream_principal_em_rtsp_manual_e_recusado() {
        let (app, dir) = app_teste("principal");
        let id = app
            .store
            .criar_camera("Manual", "rtsp", "rtsp://a:b@127.0.0.1:9/live/ch0")
            .unwrap();
        app.video
            .iniciar(id, "rtsp", "rtsp://a:b@127.0.0.1:9/live/ch0");
        let q = QueryStream {
            stream: "principal".into(),
        };
        let r = visor_pedido(&app, id, &q).err().expect("recusa");
        assert_eq!(r.0, StatusCode::BAD_REQUEST);
        assert!(visor_pedido(&app, id, &QueryStream::default())
            .unwrap()
            .is_none());
        app.video.parar(id);
        let _ = std::fs::remove_dir_all(dir);
    }

    fn edicao(v: Value) -> Json<EdicaoCamera> {
        Json(serde_json::from_value(v).unwrap())
    }

    #[tokio::test]
    async fn edita_camera_sem_vazar_senha() {
        let (app, dir) = app_teste("editar");
        let url = "rtsp://admin:segredo@192.168.1.10:554/live";
        let id = app.store.criar_camera("Porta", "rtsp", url).unwrap();

        // Muda nome e IP pela URL mascarada que a tela mostra: a senha fica.
        let Json(r) = patch_camera(
            State(app.clone()),
            Path(id),
            edicao(json!({ "nome": " Portão ", "url": "rtsp://admin:***@192.168.1.20:554/live" })),
        )
        .await
        .unwrap();
        assert_eq!(r["nome"], "Portão");
        assert!(!r.to_string().contains("segredo"));
        let c = app.store.camera(id).unwrap().unwrap();
        assert_eq!(c.url, "rtsp://admin:segredo@192.168.1.20:554/live");
        assert!(app.video.canal(id).is_some(), "vídeo reiniciado");

        // Senha em branco não troca; preenchida troca.
        let _ = patch_camera(State(app.clone()), Path(id), edicao(json!({ "senha": "" })))
            .await
            .unwrap();
        assert!(app
            .store
            .camera(id)
            .unwrap()
            .unwrap()
            .url
            .contains("segredo"));
        let _ = patch_camera(
            State(app.clone()),
            Path(id),
            edicao(json!({ "senha": "n0va" })),
        )
        .await
        .unwrap();
        assert_eq!(
            app.store.camera(id).unwrap().unwrap().url,
            "rtsp://admin:n0va@192.168.1.20:554/live"
        );

        // A lista também nunca devolve a senha.
        let Json(l) = get_cameras(State(app.clone())).await.unwrap();
        assert!(!l.to_string().contains("n0va"));
        assert_eq!(l["cameras"][0]["ativa"], true);

        // Desativar para o vídeo; reativar religa.
        let _ = patch_camera(
            State(app.clone()),
            Path(id),
            edicao(json!({ "ativa": false })),
        )
        .await
        .unwrap();
        assert!(!app.store.camera(id).unwrap().unwrap().ativa);
        assert!(app.video.canal(id).is_none());
        let _ = patch_camera(
            State(app.clone()),
            Path(id),
            edicao(json!({ "ativa": true })),
        )
        .await
        .unwrap();
        assert!(app.video.canal(id).is_some());
        app.video.parar(id);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn edicao_invalida_e_remocao() {
        let (app, dir) = app_teste("invalida");
        let id = app.store.criar_camera("Demo", "demo", "").unwrap();
        let erro = |r: ApiResult| r.err().map(|(s, _)| s);
        let r = patch_camera(
            State(app.clone()),
            Path(id),
            edicao(json!({ "nome": "  " })),
        )
        .await;
        assert_eq!(erro(r), Some(StatusCode::BAD_REQUEST));
        let r = patch_camera(
            State(app.clone()),
            Path(id),
            edicao(json!({ "fonte": "rtsp", "url": "http://x" })),
        )
        .await;
        assert_eq!(erro(r), Some(StatusCode::BAD_REQUEST));
        let r = patch_camera(
            State(app.clone()),
            Path(id),
            edicao(json!({ "senha": "x" })),
        )
        .await;
        assert_eq!(erro(r), Some(StatusCode::BAD_REQUEST));
        let r = patch_camera(
            State(app.clone()),
            Path(999),
            edicao(json!({ "nome": "x" })),
        )
        .await;
        assert_eq!(erro(r), Some(StatusCode::NOT_FOUND));

        // Troca a fonte de demo para RTSP com senha.
        let _ = patch_camera(
            State(app.clone()),
            Path(id),
            edicao(json!({ "fonte": "rtsp", "url": "rtsp://10.0.0.9/s", "senha": "p@ss" })),
        )
        .await
        .unwrap();
        assert_eq!(
            app.store.camera(id).unwrap().unwrap().url,
            "rtsp://admin:p%40ss@10.0.0.9/s"
        );

        // Remover funciona e some da lista.
        let _ = delete_camera(State(app.clone()), Path(id)).await.unwrap();
        assert!(app.store.camera(id).unwrap().is_none());
        let r = delete_camera(State(app.clone()), Path(id)).await;
        assert_eq!(erro(r), Some(StatusCode::NOT_FOUND));
        let _ = std::fs::remove_dir_all(dir);
    }
}
