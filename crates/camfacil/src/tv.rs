//! Modo TV (quiosque): a página `/tv` que fica na TV ou no monitor do cliente
//! (SDD-06). Só mostra o que os módulos produzem: mosaico das câmeras com os
//! balões das regras por cima, relógio, logo e QR do WhatsApp. Sem menus,
//! sem caixas do YOLO.
//!
//! A configuração (quais câmeras, ordem, som, WhatsApp) fica em `settings`
//! (`tv_config`, JSON) e entra no backup; a logo é um PNG pequeno em
//! `data_dir/logo.png`.

use crate::Shared;
use axum::{
    body::Bytes,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const CHAVE: &str = "tv_config";
const LOGO: &str = "logo.png";
/// Tamanho máximo da logo (SDD-06: PNG até 200 KB).
pub const LOGO_MAX: usize = 200 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigTv {
    /// Câmeras exibidas, na ordem do mosaico. Vazio = todas as ativas.
    #[serde(default)]
    pub cameras: Vec<i64>,
    /// Bipe curto quando uma regra entra em alerta.
    #[serde(default)]
    pub som: bool,
    /// Piscada da imagem no alerta.
    #[serde(default = "sim")]
    pub piscar: bool,
    /// Relógio no canto.
    #[serde(default = "sim")]
    pub relogio: bool,
    /// Número do WhatsApp (só dígitos, com DDI; ex.: 5511999990000).
    #[serde(default)]
    pub whatsapp: String,
    /// Texto curto ao lado do QR (ex.: "Fale com a Segurança Ltda").
    #[serde(default)]
    pub chamada: String,
}

fn sim() -> bool {
    true
}

impl Default for ConfigTv {
    fn default() -> Self {
        Self {
            cameras: Vec::new(),
            som: false,
            piscar: true,
            relogio: true,
            whatsapp: String::new(),
            chamada: String::new(),
        }
    }
}

pub fn config(store: &cf_core::store::Store) -> ConfigTv {
    store
        .get(CHAVE)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Normaliza um número de WhatsApp digitado de qualquer jeito: só dígitos e,
/// se vier sem DDI (10 ou 11 dígitos, padrão brasileiro), prefixa 55.
pub fn normalizar_whatsapp(entrada: &str) -> String {
    let d: String = entrada.chars().filter(|c| c.is_ascii_digit()).collect();
    let d = d.trim_start_matches('0').to_string();
    if d.len() == 10 || d.len() == 11 {
        format!("55{d}")
    } else {
        d
    }
}

pub fn link_whatsapp(numero: &str) -> Option<String> {
    (numero.len() >= 10).then(|| format!("https://wa.me/{numero}"))
}

pub(crate) fn caminho_logo() -> std::path::PathBuf {
    cf_core::paths::data_dir().join(LOGO)
}

pub fn tem_logo() -> bool {
    caminho_logo().is_file()
}

fn eh_png(b: &[u8]) -> bool {
    b.len() > 8 && b[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
}

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn erro(status: StatusCode, msg: impl ToString) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "erro": msg.to_string() })))
}

fn interno<E: std::fmt::Display>(e: E) -> (StatusCode, Json<Value>) {
    erro(StatusCode::INTERNAL_SERVER_ERROR, e)
}

fn resposta_config(app: &Shared) -> Result<Value, (StatusCode, Json<Value>)> {
    let c = config(&app.store);
    let cams: Vec<Value> = app
        .store
        .cameras()
        .map_err(interno)?
        .into_iter()
        .filter(|k| k.ativa)
        .map(|k| {
            let online = app
                .video
                .canal(k.id)
                .map(|c| c.estado().online)
                .unwrap_or(false);
            json!({ "id": k.id, "nome": k.nome, "fonte": k.fonte, "online": online })
        })
        .collect();
    let porta = app
        .store
        .get("port")
        .ok()
        .flatten()
        .unwrap_or_else(|| "7070".into());
    Ok(json!({
        "config": c,
        "cameras": cams,
        "logo": tem_logo(),
        "whatsapp_link": link_whatsapp(&c.whatsapp),
        "porta": porta,
        "ip_rede": cf_core::conectar::ip_local().map(|i| i.to_string()),
        "lan": std::env::args().any(|a| a == "--lan"),
    }))
}

/// `GET /api/tv`: configuração + câmeras disponíveis (para a tela Manutenção).
pub async fn get_config(State(app): State<Shared>) -> ApiResult {
    Ok(Json(resposta_config(&app)?))
}

/// `POST /api/tv`: salva a configuração.
pub async fn post_config(State(app): State<Shared>, Json(mut c): Json<ConfigTv>) -> ApiResult {
    c.whatsapp = normalizar_whatsapp(&c.whatsapp);
    c.chamada = c.chamada.trim().chars().take(60).collect();
    c.cameras.dedup();
    c.cameras.truncate(36);
    let s = serde_json::to_string(&c).map_err(interno)?;
    app.store.set(CHAVE, &s).map_err(interno)?;
    Ok(Json(resposta_config(&app)?))
}

/// `GET /api/tv/estado`: tudo que a TV precisa a cada segundo, numa chamada:
/// câmeras na ordem configurada, regras e estado dos balões.
pub async fn get_estado(State(app): State<Shared>) -> ApiResult {
    let c = config(&app.store);
    let todas = app.store.cameras().map_err(interno)?;
    let ordem: Vec<i64> = if c.cameras.is_empty() {
        todas.iter().filter(|k| k.ativa).map(|k| k.id).collect()
    } else {
        c.cameras
            .iter()
            .copied()
            .filter(|id| todas.iter().any(|k| k.id == *id && k.ativa))
            .collect()
    };
    let mut cams = Vec::with_capacity(ordem.len());
    for id in ordem {
        let Some(k) = todas.iter().find(|k| k.id == id) else {
            continue;
        };
        let regras = app.store.regras(id).map_err(interno)?;
        let estado = app.video.canal(id).map(|c| c.estado());
        cams.push(json!({
            "id": id,
            "nome": k.nome,
            "regras": regras,
            "estado": estado,
        }));
    }
    Ok(Json(json!({
        "cameras": cams,
        "som": c.som,
        "piscar": c.piscar,
        "relogio": c.relogio,
        "logo": tem_logo(),
        "whatsapp": link_whatsapp(&c.whatsapp).is_some(),
        "chamada": c.chamada,
    })))
}

/// `POST /api/tv/logo`: recebe o PNG cru (≤ 200 KB) e guarda na pasta de dados.
pub async fn post_logo(body: Bytes) -> ApiResult {
    if body.len() > LOGO_MAX {
        return Err(erro(
            StatusCode::PAYLOAD_TOO_LARGE,
            "a logo precisa ter até 200 KB",
        ));
    }
    if !eh_png(&body) {
        return Err(erro(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "envie um arquivo PNG",
        ));
    }
    let p = caminho_logo();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(interno)?;
    }
    std::fs::write(&p, &body).map_err(interno)?;
    Ok(Json(json!({ "logo": true, "bytes": body.len() })))
}

pub async fn delete_logo() -> ApiResult {
    let p = caminho_logo();
    if p.is_file() {
        std::fs::remove_file(&p).map_err(interno)?;
    }
    Ok(Json(json!({ "logo": false })))
}

pub async fn get_logo() -> Response {
    match std::fs::read(caminho_logo()) {
        Ok(b) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            b,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "sem logo").into_response(),
    }
}

/// QR do WhatsApp (reaproveita o gerador do elo).
pub async fn get_whatsapp_qr(State(app): State<Shared>) -> Response {
    let c = config(&app.store);
    match link_whatsapp(&c.whatsapp).and_then(|l| crate::elo::qr_svg(&l)) {
        Some(svg) => (
            [
                (header::CONTENT_TYPE, "image/svg+xml"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            svg,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "WhatsApp não configurado").into_response(),
    }
}

/// `GET /tv`: a página do quiosque (embutida no binário).
pub async fn pagina() -> Response {
    match crate::assets::find("/tv.html") {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whatsapp_normalizado() {
        assert_eq!(normalizar_whatsapp("(11) 99999-0000"), "5511999990000");
        assert_eq!(normalizar_whatsapp("+55 11 99999-0000"), "5511999990000");
        assert_eq!(normalizar_whatsapp("1733334444"), "551733334444");
        assert_eq!(normalizar_whatsapp("351 912 345 678"), "351912345678");
        assert_eq!(normalizar_whatsapp("abc"), "");
        assert_eq!(link_whatsapp(""), None);
        assert_eq!(
            link_whatsapp("5511999990000").as_deref(),
            Some("https://wa.me/5511999990000")
        );
    }

    #[test]
    fn config_padrao_e_json() {
        let c = ConfigTv::default();
        assert!(c.piscar && c.relogio && !c.som);
        let lido: ConfigTv = serde_json::from_str(r#"{"cameras":[2,1],"som":true}"#).unwrap();
        assert_eq!(lido.cameras, vec![2, 1]);
        assert!(lido.som && lido.piscar);
    }

    #[test]
    fn so_png() {
        assert!(eh_png(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]));
        assert!(!eh_png(b"GIF89a..."));
    }
}
