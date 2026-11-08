//! Tela Rede (SDD-05, fase 4): foto da rede, inventário, diferença entre fotos,
//! vigília de troca de IP por MAC e diagnóstico das câmeras.
//!
//! A varredura e o diagnóstico rodam em tarefas próprias; a UI acompanha por
//! polling. A única coisa que roda sozinha é a vigília leve: a cada 5 min, só
//! quando há câmera RTSP cadastrada, varre a porta 554 da /24 e lê a ARP para
//! ver se o MAC de alguma câmera apareceu em outro IP.

use crate::{App, Shared};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use cf_core::rede::{self, CameraRede, Diagnostico, Dica, Foto, Mudanca};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

/// Intervalo da vigília.
const VIGILIA: Duration = Duration::from_secs(5 * 60);

struct EstadoFoto {
    rodando: bool,
    progresso: Arc<AtomicUsize>,
    erro: Option<String>,
}

struct EstadoDiag {
    rodando: bool,
    quando: Option<i64>,
    resultado: Vec<Diagnostico>,
}

/// Câmera cujo MAC apareceu em outro IP; some quando o endereço é atualizado.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MudancaIp {
    pub camera_id: i64,
    pub nome: String,
    pub de: String,
    pub para: String,
    pub mac: String,
    pub quando: i64,
}

pub struct Rede {
    app: Weak<App>,
    foto: Mutex<EstadoFoto>,
    diag: Mutex<EstadoDiag>,
    mudancas: Mutex<Vec<MudancaIp>>,
}

static REDE: OnceLock<Arc<Rede>> = OnceLock::new();

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Chamado uma vez no início do programa: guarda o estado e sobe a vigília.
pub fn iniciar(app: Shared) {
    let r = Arc::new(Rede {
        app: Arc::downgrade(&app),
        foto: Mutex::new(EstadoFoto {
            rodando: false,
            progresso: Arc::new(AtomicUsize::new(0)),
            erro: None,
        }),
        diag: Mutex::new(EstadoDiag {
            rodando: false,
            quando: None,
            resultado: Vec::new(),
        }),
        mudancas: Mutex::new(Vec::new()),
    });
    if REDE.set(r.clone()).is_ok() {
        tokio::spawn(vigiar(r));
    }
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

// ---------------- MACs aprendidos ----------------

/// MAC aprendido por câmera (coluna `cameras.mac`; vazio fica de fora).
fn macs_conhecidos(app: &App) -> BTreeMap<i64, String> {
    app.store
        .cameras()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.mac_conhecido())
        .map(|c| (c.id, c.mac))
        .collect()
}

fn salvar_macs(app: &App, macs: &BTreeMap<i64, String>) {
    for (id, mac) in macs {
        let _ = app.store.definir_mac_camera(*id, mac);
    }
}

/// Aprende o MAC das câmeras cadastradas a partir de uma foto.
fn aprender_da_foto(app: &App, foto: &Foto) {
    let macs = macs_conhecidos(app);
    let novos: BTreeMap<i64, String> = foto
        .dispositivos
        .iter()
        .filter_map(|d| Some((d.camera_id?, d.mac.clone()?)))
        .filter(|(id, mac)| macs.get(id) != Some(mac))
        .collect();
    salvar_macs(app, &novos);
}

/// Aprende o MAC de uma câmera recém-ligada pela tabela ARP (a conexão RTSP
/// que acabou de dar certo deixa o IP dela na ARP).
pub async fn aprender_mac_agora(app: &App, id: i64, host: &str) -> Option<String> {
    let arp = tokio::task::spawn_blocking(rede::arp_table)
        .await
        .unwrap_or_default();
    let mac = arp.get(host)?.clone();
    let _ = app.store.definir_mac_camera(id, &mac);
    Some(mac)
}

// ---------------- Vigília: câmera mudou de IP? ----------------

/// Decide, para cada câmera com MAC conhecido, se ela apareceu em outro IP.
/// Função pura: `arp` é IP → MAC; `respondem_554` são os IPs com RTSP aberto.
pub fn detectar_mudancas(
    cams: &[CameraRede],
    macs: &BTreeMap<i64, String>,
    arp: &BTreeMap<String, String>,
    respondem_554: &[String],
    quando: i64,
) -> (Vec<MudancaIp>, BTreeMap<i64, String>) {
    let mut novas = Vec::new();
    let mut aprendidos = BTreeMap::new();
    for cam in cams {
        let mac_no_ip = arp.get(&cam.host);
        match macs.get(&cam.id) {
            None => {
                // Ainda não sabemos o MAC: aprende se a câmera responde no IP
                // dela (a vigília só varre a 554; noutra porta basta a ARP).
                let responde = cam.porta != 554 || respondem_554.contains(&cam.host);
                if let (Some(m), true) = (mac_no_ip, responde) {
                    aprendidos.insert(cam.id, m.clone());
                }
            }
            Some(mac) => {
                if mac_no_ip == Some(mac) {
                    continue; // no lugar certo
                }
                if let Some((ip, _)) = arp.iter().find(|(ip, m)| *m == mac && **ip != cam.host) {
                    novas.push(MudancaIp {
                        camera_id: cam.id,
                        nome: cam.nome.clone(),
                        de: cam.host.clone(),
                        para: ip.clone(),
                        mac: mac.clone(),
                        quando,
                    });
                } else if let (Some(m), true) = (mac_no_ip, respondem_554.contains(&cam.host)) {
                    // Outro aparelho (MAC novo) respondendo no IP: câmera trocada.
                    aprendidos.insert(cam.id, m.clone());
                }
            }
        }
    }
    (novas, aprendidos)
}

async fn vigiar(r: Arc<Rede>) {
    tokio::time::sleep(Duration::from_secs(60)).await;
    loop {
        if let Some(app) = r.app.upgrade() {
            let cams = cameras_rede(&app);
            if !cams.is_empty() {
                let respondem = cf_core::conectar::varrer_rtsp(554).await;
                let arp = tokio::task::spawn_blocking(rede::arp_table)
                    .await
                    .unwrap_or_default();
                let macs = macs_conhecidos(&app);
                let (novas, aprendidos) =
                    detectar_mudancas(&cams, &macs, &arp, &respondem, agora());
                salvar_macs(&app, &aprendidos);
                r.tratar_mudancas(&app, novas).await;
            }
        } else {
            return;
        }
        tokio::time::sleep(VIGILIA).await;
    }
}

// ---------------- Autocorreção (o que é do programa de verdade) ----------------

/// Rede, vídeo e avisos de verdade para a autocorreção.
struct AmbienteReal<'a> {
    app: &'a App,
}

impl crate::autoip::Ambiente for AmbienteReal<'_> {
    async fn porta_abre(&self, ip: &str, porta: u16) -> bool {
        let Ok(ip) = ip.parse::<std::net::IpAddr>() else {
            return false;
        };
        for _ in 0..2 {
            if rede::sondar(ip, porta, Duration::from_millis(1500))
                .await
                .is_some()
            {
                return true;
            }
        }
        false
    }

    async fn mac_no_ip(&self, ip: &str) -> Option<String> {
        let ip = ip.to_string();
        tokio::task::spawn_blocking(move || rede::arp_table().get(&ip).cloned())
            .await
            .ok()
            .flatten()
    }

    fn reiniciar_video(&self, id: i64, url: &str) {
        self.app.video.iniciar(id, "rtsp", url);
    }

    async fn imagem_voltou(&self, id: i64, limite: Duration) -> bool {
        let fim = std::time::Instant::now() + limite;
        while std::time::Instant::now() < fim {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Some(c) = self.app.video.canal(id) {
                let tem_quadro = c.ultimo.lock().map(|u| u.is_some()).unwrap_or(false);
                if tem_quadro && c.estado().online {
                    return true;
                }
            }
        }
        false
    }

    fn notificar(&self, chave: String, texto: String) {
        crate::video::notificar(chave, texto, None);
    }
}

/// IPs em que a autocorreção não mexe sozinha: conflito visto nas últimas
/// fotos da rede.
fn ips_em_conflito(app: &App) -> Vec<String> {
    let fotos: Vec<Foto> = app
        .store
        .fotos_rede(rede::FOTOS_CONFLITO as i64)
        .unwrap_or_default()
        .into_iter()
        .map(|(_, f)| f)
        .collect();
    rede::conflitos_ip(&fotos)
        .into_iter()
        .map(|c| c.ip)
        .collect()
}

impl Rede {
    /// Para cada mudança: com autocorreção ligada, tenta corrigir sozinho;
    /// o resto (e o que falhou) vira aviso com o botão de confirmar.
    async fn tratar_mudancas(&self, app: &App, novas: Vec<MudancaIp>) {
        if novas.is_empty() {
            return;
        }
        let conflitos = ips_em_conflito(app);
        let cams = cameras_rede(app);
        let amb = AmbienteReal { app };
        let mut manuais = Vec::new();
        for m in novas {
            // Já pendente (ex.: falhou antes): não tenta de novo a cada 5 min.
            let ja = self
                .mudancas_pendentes()
                .iter()
                .any(|p| p.camera_id == m.camera_id && p.para == m.para);
            if ja {
                manuais.push(m);
                continue;
            }
            // Dois MACs no IP novo, ou o IP já é de outra câmera cadastrada.
            let conflito = conflitos.contains(&m.para)
                || cams.iter().any(|c| c.id != m.camera_id && c.host == m.para);
            match crate::autoip::tentar(&amb, &app.store, &m, conflito, agora()).await {
                crate::autoip::Desfecho::Corrigida(_) => {
                    self.mudancas
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .retain(|p| p.camera_id != m.camera_id);
                }
                crate::autoip::Desfecho::Manual(_) => manuais.push(m),
            }
        }
        self.registrar_mudancas(app, manuais);
    }

    /// Guarda as mudanças novas e gera um evento `rede` por câmera/IP novo.
    fn registrar_mudancas(&self, app: &App, novas: Vec<MudancaIp>) {
        let mut pend = self.mudancas.lock().unwrap_or_else(|p| p.into_inner());
        let ids: Vec<i64> = cameras_rede(app).iter().map(|c| c.id).collect();
        pend.retain(|m| ids.contains(&m.camera_id));
        for n in novas {
            if pend
                .iter()
                .any(|m| m.camera_id == n.camera_id && m.para == n.para)
            {
                continue;
            }
            let detalhe = format!("câmera mudou de IP: {} → {}", n.de, n.para);
            let _ = app
                .store
                .registrar_evento(n.camera_id, None, "rede", &detalhe, n.quando);
            pend.retain(|m| m.camera_id != n.camera_id);
            pend.push(n);
        }
    }

    fn mudancas_pendentes(&self) -> Vec<MudancaIp> {
        self.mudancas
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

// ---------------- API: foto ----------------

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn erro(status: StatusCode, msg: impl ToString) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "erro": msg.to_string() })))
}

fn interno<E: std::fmt::Display>(e: E) -> (StatusCode, Json<Value>) {
    erro(StatusCode::INTERNAL_SERVER_ERROR, e)
}

fn rede_ou_erro() -> Result<Arc<Rede>, (StatusCode, Json<Value>)> {
    rede().ok_or_else(|| erro(StatusCode::SERVICE_UNAVAILABLE, "rede não iniciada"))
}

/// `POST /api/rede/foto`: começa a varredura (409 se já estiver rodando).
pub async fn post_foto(State(app): State<Shared>) -> ApiResult {
    let r = rede_ou_erro()?;
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
        aprender_da_foto(&app, &foto);
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
        .fotos_rede(rede::FOTOS_CONFLITO as i64)
        .map_err(interno)?;
    let so_fotos: Vec<Foto> = fotos.iter().map(|(_, f)| f.clone()).collect();
    let conflitos = rede::conflitos_ip(&so_fotos);
    let ultima: Option<&Foto> = fotos.first().map(|(_, f)| f);
    let anterior: Option<&Foto> = fotos.get(1).map(|(_, f)| f);
    let mudancas: Vec<Mudanca> = match (anterior, ultima) {
        (Some(a), Some(u)) => rede::comparar(a, u),
        _ => Vec::new(),
    };
    let historico = app.store.fotos_rede_resumo(30).map_err(interno)?;
    Ok(Json(json!({
        "rodando": rodando,
        "progresso": progresso,
        "total": rede::total_sondas(),
        "erro": erro_txt,
        "ultima": ultima,
        "anterior_quando": anterior.map(|a| a.quando),
        "mudancas": mudancas,
        "conflitos": conflitos,
        "historico": historico,
    })))
}

/// `GET /api/rede/fotos/{id}`: uma foto antiga inteira.
pub async fn get_foto_id(
    State(app): State<Shared>,
    Path(id): Path<i64>,
) -> axum::response::Response {
    let fotos = app.store.fotos_rede(30).unwrap_or_default();
    match fotos.into_iter().find(|(i, _)| *i == id) {
        Some((_, f)) => Json(json!({ "foto": f })).into_response(),
        None => (StatusCode::NOT_FOUND, "foto não encontrada").into_response(),
    }
}

// ---------------- API: diagnóstico e mudança de IP ----------------

/// `POST /api/rede/diagnostico`: diagnostica todas as câmeras RTSP em segundo plano.
pub async fn post_diagnostico(State(app): State<Shared>) -> ApiResult {
    let r = rede_ou_erro()?;
    {
        let mut d = r.diag.lock().unwrap_or_else(|p| p.into_inner());
        if d.rodando {
            return Err(erro(StatusCode::CONFLICT, "já estou diagnosticando"));
        }
        d.rodando = true;
    }
    let cams = cameras_rede(&app);
    tokio::spawn(async move {
        let meu = cf_core::conectar::ip_local();
        let (arp, gw) = tokio::task::spawn_blocking(|| (rede::arp_table(), rede::gateway_padrao()))
            .await
            .unwrap_or_default();
        let macs = macs_conhecidos(&app);
        let pendentes = r.mudancas_pendentes();
        let mut lista = Vec::with_capacity(cams.len());
        for cam in &cams {
            let mut d =
                rede::diagnosticar(cam, meu, gw, &arp, macs.get(&cam.id).map(String::as_str)).await;
            // A vigília pode já saber o IP novo: entra como dica principal.
            if d.novo_ip.is_none() {
                if let Some(p) = pendentes.iter().find(|p| p.camera_id == cam.id) {
                    d.novo_ip = Some(p.para.clone());
                    d.dicas
                        .retain(|x| !matches!(x.codigo, "sem_resposta" | "tudo_ok"));
                    d.dicas.insert(
                        0,
                        Dica {
                            codigo: "mudou_ip",
                            params: [("de", p.de.clone()), ("para", p.para.clone())]
                                .into_iter()
                                .collect(),
                        },
                    );
                }
            } else if !pendentes.iter().any(|p| p.camera_id == cam.id) {
                // O diagnóstico achou o MAC em outro IP: registra como a vigília faria.
                if let (Some(mac), Some(novo)) = (&d.mac, &d.novo_ip) {
                    r.tratar_mudancas(
                        &app,
                        vec![MudancaIp {
                            camera_id: cam.id,
                            nome: cam.nome.clone(),
                            de: cam.host.clone(),
                            para: novo.clone(),
                            mac: mac.clone(),
                            quando: agora(),
                        }],
                    )
                    .await;
                }
            }
            lista.push(d);
        }
        let mut d = r.diag.lock().unwrap_or_else(|p| p.into_inner());
        d.rodando = false;
        d.quando = Some(agora());
        d.resultado = lista;
    });
    Ok(Json(json!({ "iniciado": true })))
}

/// `GET /api/rede/diagnostico`: último resultado, mudanças de IP pendentes e dados da rede.
pub async fn get_diagnostico(State(app): State<Shared>) -> ApiResult {
    let r = rede_ou_erro()?;
    let (rodando, quando, resultado) = {
        let d = r.diag.lock().unwrap_or_else(|p| p.into_inner());
        (d.rodando, d.quando, d.resultado.clone())
    };
    let cams = cameras_rede(&app);
    Ok(Json(json!({
        "rodando": rodando,
        "quando": quando,
        "cameras_rtsp": cams.len(),
        "resultado": resultado,
        "mudancas": r.mudancas_pendentes(),
        "meu_ip": cf_core::conectar::ip_local().map(|i| i.to_string()),
        // Autocorreção de IP: estado por câmera RTSP e registro das correções.
        "autocorrecao": app.store.cameras().unwrap_or_default().iter()
            .filter(|c| c.fonte == "rtsp")
            .map(|c| json!({ "camera_id": c.id, "auto_ip": c.auto_ip, "mac_conhecido": c.mac_conhecido() }))
            .collect::<Vec<_>>(),
        "auto_log": crate::autoip::ler_log(&app.store).into_iter().take(20).collect::<Vec<_>>(),
    })))
}

/// `GET /api/rede/resumo`: uma linha para o bloco de saúde da tela Início.
pub async fn get_resumo(State(app): State<Shared>) -> ApiResult {
    let r = rede_ou_erro()?;
    let (quando, alcancaveis, total) = {
        let d = r.diag.lock().unwrap_or_else(|p| p.into_inner());
        (
            d.quando,
            d.resultado.iter().filter(|x| x.alcance).count(),
            d.resultado.len(),
        )
    };
    let ultima_foto = app
        .store
        .fotos_rede_resumo(1)
        .map_err(interno)?
        .first()
        .map(|f| f.quando);
    Ok(Json(json!({
        "cameras_rtsp": cameras_rede(&app).len(),
        "diagnostico_quando": quando,
        "alcancaveis": alcancaveis,
        "diagnosticadas": total,
        "mudancas": r.mudancas_pendentes().len(),
        "ultima_foto": ultima_foto,
        // Correções automáticas (e desligamentos por oscilação) das últimas 24 h.
        "auto_avisos": crate::autoip::avisos_recentes(&crate::autoip::ler_log(&app.store), agora()),
    })))
}

// ---------------- API: diagrama unifilar ----------------

fn traduzir(k: &str) -> String {
    crate::i18n::t(k)
}

#[derive(Deserialize)]
pub struct DiagramaQuery {
    foto: Option<i64>,
    #[serde(default)]
    baixar: bool,
}

/// `GET /api/rede/diagrama.svg?foto=ID&baixar=true`: diagrama unifilar da
/// foto pedida (ou da última). Na tela usa os tokens; baixado, cores de papel.
pub async fn get_diagrama(
    State(app): State<Shared>,
    axum::extract::Query(q): axum::extract::Query<DiagramaQuery>,
) -> axum::response::Response {
    use axum::http::header;
    let fotos = app.store.fotos_rede(30).unwrap_or_default();
    let achada = match q.foto {
        Some(id) => fotos.into_iter().find(|(i, _)| *i == id),
        None => fotos.into_iter().next(),
    };
    let Some((id, foto)) = achada else {
        return (StatusCode::NOT_FOUND, "foto não encontrada").into_response();
    };
    let tema = if q.baixar {
        cf_core::unifilar::Tema::Papel
    } else {
        cf_core::unifilar::Tema::Pagina
    };
    let offset = cf_notify::servico::offset_local_s(&app.store);
    let svg = cf_core::unifilar::desenhar(&foto, tema, offset, &traduzir);
    let mut r = (
        [
            (
                header::CONTENT_TYPE,
                "image/svg+xml; charset=utf-8".to_string(),
            ),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        svg,
    )
        .into_response();
    if q.baixar {
        let nome = format!(
            "attachment; filename=\"unifilar-{}-foto{id}.svg\"",
            foto.rede.split('/').next().unwrap_or("rede")
        );
        if let Ok(v) = header::HeaderValue::from_str(&nome) {
            r.headers_mut().insert(header::CONTENT_DISPOSITION, v);
        }
    }
    r
}

#[derive(Deserialize)]
pub struct Atualizar {
    para: String,
}

/// `POST /api/rede/cameras/{id}/atualizar`: reescreve o host da URL RTSP
/// (mantendo usuário, senha, porta e caminho) e reinicia o vídeo.
pub async fn post_atualizar(
    State(app): State<Shared>,
    Path(id): Path<i64>,
    Json(a): Json<Atualizar>,
) -> ApiResult {
    let r = rede_ou_erro()?;
    let para = a.para.trim();
    if para.parse::<std::net::IpAddr>().is_err() {
        return Err(erro(StatusCode::BAD_REQUEST, "IP novo inválido"));
    }
    let cam = app
        .store
        .camera(id)
        .map_err(interno)?
        .ok_or_else(|| erro(StatusCode::NOT_FOUND, "câmera não encontrada"))?;
    if cam.fonte != "rtsp" {
        return Err(erro(
            StatusCode::BAD_REQUEST,
            "só câmeras RTSP têm endereço de rede",
        ));
    }
    let (de, _) = rede::host_da_url(&cam.url)
        .ok_or_else(|| erro(StatusCode::BAD_REQUEST, "endereço RTSP inválido"))?;
    let nova = rede::trocar_host(&cam.url, para)
        .ok_or_else(|| erro(StatusCode::BAD_REQUEST, "endereço RTSP inválido"))?;
    app.store.atualizar_url_camera(id, &nova).map_err(interno)?;
    app.video.parar(id);
    app.video.iniciar(id, "rtsp", &nova);
    let _ = app.store.registrar_evento(
        id,
        None,
        "rede",
        &format!("endereço atualizado: {de} → {para}"),
        agora(),
    );
    r.mudancas
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .retain(|m| m.camera_id != id);
    Ok(Json(json!({
        "atualizada": true,
        "url_sem_senha": cf_core::rtsp::redact(&nova),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam(id: i64, host: &str) -> CameraRede {
        CameraRede {
            id,
            nome: format!("Cam {id}"),
            host: host.into(),
            porta: 554,
        }
    }

    #[test]
    fn traducoes_do_diagrama_existem() {
        for k in [
            "rede.diagrama.titulo",
            "rede.diagrama.resumo",
            "rede.diagrama.switch_presumido",
            "rede.tipo.camera",
            "rede.eu",
        ] {
            assert_ne!(traduzir(k), k, "falta a chave {k}");
        }
    }

    #[test]
    fn vigilia_detecta_troca_de_ip_e_aprende_mac() {
        let cams = vec![
            cam(1, "192.168.1.108"),
            cam(2, "192.168.1.109"),
            cam(3, "192.168.1.110"),
        ];
        let mut macs = BTreeMap::new();
        macs.insert(1, "58:10:8C:00:00:01".to_string());
        macs.insert(3, "58:10:8C:00:00:03".to_string());
        let mut arp = BTreeMap::new();
        arp.insert("192.168.1.120".to_string(), "58:10:8C:00:00:01".to_string()); // cam 1 mudou
        arp.insert("192.168.1.109".to_string(), "3C:EF:8C:00:00:02".to_string()); // cam 2: aprende
        arp.insert("192.168.1.110".to_string(), "58:10:8C:00:00:03".to_string()); // cam 3: no lugar
        let respondem = vec!["192.168.1.109".to_string(), "192.168.1.110".to_string()];
        let (novas, aprendidos) = detectar_mudancas(&cams, &macs, &arp, &respondem, 10);
        assert_eq!(novas.len(), 1);
        assert_eq!(novas[0].camera_id, 1);
        assert_eq!(novas[0].de, "192.168.1.108");
        assert_eq!(novas[0].para, "192.168.1.120");
        assert_eq!(
            aprendidos.get(&2).map(String::as_str),
            Some("3C:EF:8C:00:00:02")
        );
        assert!(!aprendidos.contains_key(&3));
        // Câmera sem resposta e MAC em lugar nenhum: nada a dizer.
        let (novas, aprendidos) = detectar_mudancas(&cams, &macs, &BTreeMap::new(), &[], 10);
        assert!(novas.is_empty() && aprendidos.is_empty());
    }
}
