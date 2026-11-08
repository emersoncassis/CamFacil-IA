//! Backup cifrado da configuração (PRD §5.7): exportar e importar.
//!
//! O arquivo `.cfbkp` leva câmeras, regras, módulos, modo TV (com a logo),
//! contatos do Telegram e selos; não leva eventos, identidade da máquina,
//! porta nem aparelhos pareados. É protegido por uma senha escolhida na hora:
//! PBKDF2-HMAC-SHA256 (feito aqui com o `sha2` que já está no binário) deriva
//! a chave e o ChaCha20-Poly1305 do cofre cifra. Segredos que no banco estão
//! cifrados com a chave desta máquina (token do Telegram) vão em claro
//! *dentro* do arquivo cifrado e são recifrados com a chave da máquina que
//! importa, para o backup servir também numa máquina nova.
//!
//! Formato: `CFIABKP1` · iterações (u32 BE) · sal (16) · nonce (12) ·
//! cifra(deflate(JSON)) + tag.

use crate::Shared;
use axum::{
    body::Bytes,
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use cf_core::store::ConfigBackup;
use cf_notify::cofre::{self, Cofre};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MAGICO: &[u8; 8] = b"CFIABKP1";
const ITERACOES: u32 = 200_000;
const SENHA_MIN: usize = 8;
/// Teto do arquivo aceito na importação (logo de 200 KB + configuração).
pub const TAMANHO_MAX: usize = 4 * 1024 * 1024;

// ---------------------------------------------------------------------------
// PBKDF2-HMAC-SHA256 (RFC 8018), uma saída de 32 bytes.
// ---------------------------------------------------------------------------

fn hmac_sha256(chave: &[u8], msg: &[&[u8]]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if chave.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(chave));
    } else {
        k[..chave.len()].copy_from_slice(chave);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut h = Sha256::new();
    h.update(ipad);
    for m in msg {
        h.update(m);
    }
    let interno = h.finalize();
    let mut h = Sha256::new();
    h.update(opad);
    h.update(interno);
    h.finalize().into()
}

pub fn pbkdf2_sha256(senha: &[u8], sal: &[u8], iteracoes: u32) -> [u8; 32] {
    let mut u = hmac_sha256(senha, &[sal, &1u32.to_be_bytes()]);
    let mut t = u;
    for _ in 1..iteracoes {
        u = hmac_sha256(senha, &[&u]);
        for (a, b) in t.iter_mut().zip(u.iter()) {
            *a ^= b;
        }
    }
    t
}

// ---------------------------------------------------------------------------
// Conteúdo
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct Conteudo {
    versao: u32,
    app: String,
    criado_em: i64,
    config: ConfigBackup,
    /// Chaves de `settings` que estavam cifradas com a chave da máquina.
    #[serde(default)]
    segredos: Vec<String>,
    /// Logo do modo TV em hexadecimal (o deflate compensa o dobro).
    #[serde(default)]
    logo_hex: String,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn de_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Cifra o conteúdo com a senha (iterações configuráveis para os testes).
fn selar(conteudo: &[u8], senha: &str, iteracoes: u32) -> Vec<u8> {
    let sal = cofre::aleatorio(16);
    let chave = pbkdf2_sha256(senha.as_bytes(), &sal, iteracoes);
    let comprimido = miniz_oxide::deflate::compress_to_vec(conteudo, 6);
    let mut out = MAGICO.to_vec();
    out.extend_from_slice(&iteracoes.to_be_bytes());
    out.extend_from_slice(&sal);
    out.extend(cofre::selar(&chave, &comprimido));
    out
}

fn abrir(arquivo: &[u8], senha: &str) -> Result<Vec<u8>, String> {
    if arquivo.len() < 8 + 4 + 16 + 12 + 16 || &arquivo[..8] != MAGICO {
        return Err("este arquivo não é um backup do CamFácil IA".into());
    }
    let iteracoes = u32::from_be_bytes([arquivo[8], arquivo[9], arquivo[10], arquivo[11]]);
    if !(1_000..=5_000_000).contains(&iteracoes) {
        return Err("backup corrompido".into());
    }
    let sal = &arquivo[12..28];
    let chave = pbkdf2_sha256(senha.as_bytes(), sal, iteracoes);
    let comprimido =
        cofre::abrir_selado(&chave, &arquivo[28..]).ok_or("senha errada ou arquivo alterado")?;
    miniz_oxide::inflate::decompress_to_vec_with_limit(&comprimido, 16 * 1024 * 1024)
        .map_err(|_| "backup corrompido".to_string())
}

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Monta o arquivo de backup desta máquina.
pub fn exportar(app: &crate::App, senha: &str) -> Result<Vec<u8>, String> {
    exportar_com(app, senha, ITERACOES)
}

fn exportar_com(app: &crate::App, senha: &str, iteracoes: u32) -> Result<Vec<u8>, String> {
    validar_senha(senha)?;
    let mut config = app.store.exportar_config().map_err(|e| e.to_string())?;
    let cofre = Cofre::abrir()?;
    let mut segredos = Vec::new();
    for (k, v) in config.settings.iter_mut() {
        if v.starts_with("enc:") {
            if let Some(claro) = cofre.decifrar(v) {
                *v = claro;
                segredos.push(k.clone());
            }
        }
    }
    let logo = std::fs::read(crate::tv::caminho_logo()).unwrap_or_default();
    let c = Conteudo {
        versao: 1,
        app: cf_core::VERSION.to_string(),
        criado_em: agora(),
        config,
        segredos,
        logo_hex: hex(&logo),
    };
    let json = serde_json::to_vec(&c).map_err(|e| e.to_string())?;
    Ok(selar(&json, senha, iteracoes))
}

/// Resumo do que foi restaurado.
#[derive(Serialize)]
pub struct Restaurado {
    pub cameras: usize,
    pub regras: usize,
    pub contatos: usize,
    pub logo: bool,
    pub criado_em: i64,
    pub app: String,
}

/// Restaura a configuração e religa as câmeras.
pub fn importar(app: &crate::App, arquivo: &[u8], senha: &str) -> Result<Restaurado, String> {
    let json = abrir(arquivo, senha)?;
    let mut c: Conteudo =
        serde_json::from_slice(&json).map_err(|_| "backup corrompido".to_string())?;
    if c.versao != 1 {
        return Err("backup de uma versão mais nova do CamFácil IA; atualize o programa".into());
    }
    let logo = de_hex(&c.logo_hex).ok_or("backup corrompido")?;
    if logo.len() > crate::tv::LOGO_MAX {
        return Err("backup corrompido".into());
    }
    let cofre = Cofre::abrir()?;
    for (k, v) in c.config.settings.iter_mut() {
        if c.segredos.contains(k) {
            *v = cofre.cifrar(v);
        }
    }
    for k in app.video.canais() {
        app.video.parar(k.id);
    }
    app.store
        .importar_config(&c.config, agora())
        .map_err(|e| e.to_string())?;
    let caminho = crate::tv::caminho_logo();
    if logo.is_empty() {
        let _ = std::fs::remove_file(&caminho);
    } else {
        std::fs::write(&caminho, &logo).map_err(|e| e.to_string())?;
    }
    app.video.iniciar_todas();
    for k in app.video.canais() {
        app.video.recarregar_regras(k.id);
    }
    Ok(Restaurado {
        cameras: c.config.cameras.len(),
        regras: c.config.regras.len(),
        contatos: c.config.telegram.iter().filter(|t| t.autorizado).count(),
        logo: !logo.is_empty(),
        criado_em: c.criado_em,
        app: c.app,
    })
}

fn validar_senha(senha: &str) -> Result<(), String> {
    if senha.chars().count() < SENHA_MIN {
        return Err(format!(
            "use uma senha com pelo menos {SENHA_MIN} caracteres"
        ));
    }
    Ok(())
}

fn nome_arquivo(app: &crate::App) -> String {
    let off = cf_notify::servico::offset_local_s(&app.store);
    let t = agora();
    let (a, m, d) = crate::relatorio::data_civil(t, off);
    let h = cf_notify::servico::hora_com_offset(t, off).replace(':', "");
    format!("camfacil-backup-{a}{m:02}{d:02}-{h}.cfbkp")
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn erro(status: StatusCode, msg: impl ToString) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "erro": msg.to_string() })))
}

#[derive(Deserialize)]
pub struct PedidoSenha {
    senha: String,
}

async fn gerar(app: &Shared, senha: String) -> Result<Vec<u8>, (StatusCode, Json<Value>)> {
    validar_senha(&senha).map_err(|e| erro(StatusCode::BAD_REQUEST, e))?;
    let app2 = app.clone();
    tokio::task::spawn_blocking(move || exportar(&app2, &senha))
        .await
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e))
}

/// `POST /api/backup`: devolve o arquivo cifrado para baixar.
pub async fn post_exportar(State(app): State<Shared>, Json(p): Json<PedidoSenha>) -> Response {
    match gerar(&app, p.senha).await {
        Ok(b) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", nome_arquivo(&app)),
                ),
            ],
            b,
        )
            .into_response(),
        Err(e) => e.into_response(),
    }
}

/// `POST /api/backup/telegram`: gera e manda aos contatos autorizados.
pub async fn post_telegram(State(app): State<Shared>, Json(p): Json<PedidoSenha>) -> ApiResult {
    let bytes = gerar(&app, p.senha).await?;
    let tg = app
        .telegram
        .get()
        .ok_or_else(|| erro(StatusCode::SERVICE_UNAVAILABLE, "Telegram indisponível"))?;
    let n = tg
        .enviar_arquivo(
            &nome_arquivo(&app),
            bytes,
            "Backup cifrado da configuração do CamFácil IA. Guarde a senha: sem ela o arquivo não abre.",
        )
        .await
        .map_err(|e| erro(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "enviados": n })))
}

/// `POST /api/backup/importar`: corpo = u16 BE com o tamanho da senha em
/// bytes, a senha (UTF-8) e o arquivo. Binário para não inchar com base64.
pub async fn post_importar(State(app): State<Shared>, corpo: Bytes) -> ApiResult {
    if corpo.len() > TAMANHO_MAX + 1024 {
        return Err(erro(StatusCode::PAYLOAD_TOO_LARGE, "arquivo grande demais"));
    }
    if corpo.len() < 2 {
        return Err(erro(StatusCode::BAD_REQUEST, "envie o arquivo e a senha"));
    }
    let n = u16::from_be_bytes([corpo[0], corpo[1]]) as usize;
    let senha = corpo
        .get(2..2 + n)
        .and_then(|s| std::str::from_utf8(s).ok())
        .ok_or_else(|| erro(StatusCode::BAD_REQUEST, "senha inválida"))?
        .to_string();
    let arquivo = corpo[2 + n..].to_vec();
    let app2 = app.clone();
    let r = tokio::task::spawn_blocking(move || importar(&app2, &arquivo, &senha))
        .await
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map_err(|e| erro(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!(r)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hexs(b: &[u8]) -> String {
        hex(b)
    }

    #[test]
    fn pbkdf2_vetores_rfc7914() {
        // RFC 7914 §11: PBKDF2-HMAC-SHA256, P="passwd", S="salt", c=1.
        assert_eq!(
            hexs(&pbkdf2_sha256(b"passwd", b"salt", 1)),
            "55ac046e56e3089fec1691c22544b605f94185216dde0465e68b9d57c20dacbc"
        );
        // c=80000, P="Password", S="NaCl".
        assert_eq!(
            hexs(&pbkdf2_sha256(b"Password", b"NaCl", 80_000)),
            "4ddcd8f60b98be21830cee5ef22701f9641a4418d04c0414aeff08876b34ab56"
        );
    }

    #[test]
    fn hex_ida_e_volta() {
        let b = [0u8, 1, 0xab, 0xff];
        assert_eq!(de_hex(&hex(&b)).unwrap(), b);
        assert!(de_hex("abc").is_none());
        assert!(de_hex("zz").is_none());
    }

    #[test]
    fn selar_e_abrir() {
        let dados = r#"{"ola":"mundo","acento":"ção"}"#.as_bytes();
        let f = selar(dados, "senha-boa-123", 1_000);
        assert!(f.starts_with(MAGICO));
        assert_eq!(abrir(&f, "senha-boa-123").unwrap(), dados);
        assert!(abrir(&f, "senha-errada").is_err());
        let mut mexido = f.clone();
        let ultimo = mexido.len() - 1;
        mexido[ultimo] ^= 1;
        assert!(abrir(&mexido, "senha-boa-123").is_err());
        assert!(abrir(b"qualquer coisa", "x").is_err());
    }

    #[test]
    fn senha_curta() {
        assert!(validar_senha("1234567").is_err());
        assert!(validar_senha("çãõ12345").is_ok());
    }
}
