//! Arquivo de alertas (SDD-04 "Telegram como arquivo de alertas").
//!
//! Desligado por padrão e só liga depois do aceite do termo do técnico. Ligado:
//! - guarda a foto do instante de cada alerta em `data_dir/fotos/cam_<id>/`,
//!   com índice na tabela `arquivo_alertas` e retenção por câmera (as mais
//!   velhas saem do disco);
//! - se houver um canal/grupo privado configurado, manda a foto para lá pelo
//!   mesmo bot do Telegram e guarda o `message_id` (link "ver no Telegram");
//! - o webhook recebe `foto_url` apontando para a foto guardada.
//!
//! Custo: uma tarefa tokio ociosa e um canal de 32 posições; nada roda sem alerta.

use crate::api::{bad, internal, ApiResult};
use crate::termos::{exigir_tecnico, PASTA_FOTOS};
use crate::Shared;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use cf_core::store::Store;
use cf_notify::{Alerta, Telegram};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc;

const CHAVE_LIGADO: &str = "arquivo_ligado";
const CHAVE_CHAT: &str = "arquivo_chat_id";
const CHAVE_MANTER: &str = "arquivo_manter";
/// Fotos guardadas por câmera quando o técnico não escolheu outro número.
pub const MANTER_PADRAO: i64 = 200;
const MANTER_MAX: i64 = 5000;

struct Envio {
    id: i64,
    chat: i64,
    jpeg: Arc<Vec<u8>>,
    legenda: String,
}

pub struct Arquivo {
    store: Arc<Store>,
    telegram: OnceLock<Arc<Telegram>>,
    tx: mpsc::Sender<Envio>,
    ultimo_erro: std::sync::Mutex<Option<String>>,
}

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn pasta_cam(camera_id: i64) -> PathBuf {
    cf_core::paths::data_dir()
        .join(PASTA_FOTOS)
        .join(format!("cam_{camera_id}"))
}

/// Link `t.me/c/…` de uma mensagem num canal/supergrupo privado (`-100…`).
pub fn link_telegram(chat_id: i64, message_id: i64) -> Option<String> {
    let s = chat_id.to_string();
    let interno = s.strip_prefix("-100")?;
    (message_id > 0 && !interno.is_empty())
        .then(|| format!("https://t.me/c/{interno}/{message_id}"))
}

/// Nome do arquivo: instante + nanossegundos (dois alertas no mesmo segundo
/// não colidem).
fn nome_foto(quando: i64, nanos: u32) -> String {
    format!("{quando}_{nanos:09}.jpg")
}

impl Arquivo {
    /// Precisa de runtime tokio (cria a tarefa de envio ao canal).
    pub fn iniciar(store: Arc<Store>) -> Arc<Self> {
        let (tx, rx) = mpsc::channel(32);
        let a = Arc::new(Self {
            store,
            telegram: OnceLock::new(),
            tx,
            ultimo_erro: std::sync::Mutex::new(None),
        });
        tokio::spawn(a.clone().laco(rx));
        a
    }

    pub fn definir_telegram(&self, t: Arc<Telegram>) {
        let _ = self.telegram.set(t);
    }

    pub fn ligado(&self) -> bool {
        self.store.get(CHAVE_LIGADO).ok().flatten().as_deref() == Some("1")
    }

    pub fn chat(&self) -> Option<i64> {
        self.store
            .get(CHAVE_CHAT)
            .ok()
            .flatten()
            .and_then(|v| v.trim().parse().ok())
            .filter(|v: &i64| *v != 0)
    }

    pub fn manter(&self) -> i64 {
        self.store
            .get(CHAVE_MANTER)
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .filter(|v: &i64| *v > 0)
            .unwrap_or(MANTER_PADRAO)
    }

    /// Guarda o alerta (se o arquivo estiver ligado e houver foto). Devolve o
    /// id no índice. Pode ser chamada de qualquer thread; o envio ao canal fica
    /// para a tarefa assíncrona.
    pub fn registrar(&self, a: &Alerta) -> Option<i64> {
        if !self.ligado() {
            return None;
        }
        let jpeg = a.foto.clone()?;
        let ev = cf_notify::webhook::evento_de(&self.store, a, None);
        let quando = ev.quando;
        let pasta = pasta_cam(ev.camera_id);
        std::fs::create_dir_all(&pasta).ok()?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let nome = nome_foto(quando, nanos);
        if std::fs::write(pasta.join(&nome), jpeg.as_slice()).is_err() {
            return None;
        }
        let id = self
            .store
            .arquivar_alerta(ev.camera_id, ev.regra_id, &a.chave, quando, &nome)
            .ok()?;
        for velho in self
            .store
            .arquivo_excedentes(ev.camera_id, self.manter())
            .unwrap_or_default()
        {
            let _ = std::fs::remove_file(pasta.join(velho));
        }
        if let Some(chat) = self.chat() {
            let _ = self.tx.try_send(Envio {
                id,
                chat,
                jpeg,
                legenda: a.texto.clone(),
            });
        }
        Some(id)
    }

    async fn laco(self: Arc<Self>, mut rx: mpsc::Receiver<Envio>) {
        while let Some(e) = rx.recv().await {
            let Some(bot) = self.telegram.get().and_then(|t| t.bot()) else {
                *self.ultimo_erro.lock().unwrap() =
                    Some("configure o bot do Telegram para usar o canal do arquivo".into());
                continue;
            };
            match bot.send_photo_id(e.chat, &e.jpeg, &e.legenda).await {
                Ok(msg) => {
                    let _ = self.store.arquivo_mensagem(e.id, e.chat, msg);
                    *self.ultimo_erro.lock().unwrap() = None;
                }
                Err(err) => *self.ultimo_erro.lock().unwrap() = Some(err),
            }
        }
    }

    pub fn ultimo_erro(&self) -> Option<String> {
        self.ultimo_erro.lock().unwrap().clone()
    }
}

// ---------- Rotas ----------

type Falha = (StatusCode, Json<serde_json::Value>);

fn arquivo(app: &Shared) -> Result<&Arc<Arquivo>, Falha> {
    app.arquivo
        .get()
        .ok_or_else(|| bad(StatusCode::SERVICE_UNAVAILABLE, "arquivo indisponível"))
}

pub async fn get_arquivo(State(app): State<Shared>) -> ApiResult {
    let a = arquivo(&app)?;
    let lista = app.store.arquivo_alertas(50).map_err(internal)?;
    let cams = app.store.cameras().map_err(internal)?;
    let nome_cam = |id: i64| {
        cams.iter()
            .find(|c| c.id == id)
            .map(|c| c.nome.clone())
            .unwrap_or_else(|| format!("câmera {id}"))
    };
    let regras: Vec<_> = cams
        .iter()
        .flat_map(|c| app.store.regras(c.id).unwrap_or_default())
        .collect();
    let off = cf_notify::servico::offset_local_s(&app.store);
    let itens: Vec<_> = lista
        .iter()
        .map(|x| {
            let (d, m, _) = crate::termos::data_local(x.quando, off);
            json!({
                "id": x.id,
                "quando": x.quando,
                "data": format!("{d:02}/{m:02} {}", cf_notify::servico::hora_com_offset(x.quando, off)),
                "camera": nome_cam(x.camera_id),
                "regra": x.regra_id.and_then(|r| regras.iter().find(|g| g.id == r).map(|g| g.nome.clone())),
                "foto": (!x.arquivo.is_empty()).then(|| format!("/api/arquivo/{}.jpg", x.id)),
                "telegram": x.chat_id.zip(x.message_id).and_then(|(c, m)| link_telegram(c, m)),
            })
        })
        .collect();
    Ok(Json(json!({
        "ligado": a.ligado(),
        "chat_id": a.chat(),
        "manter": a.manter(),
        "telegram_configurado": app.telegram.get().map(|t| t.situacao().configurado) == Some(true),
        "ultimo_erro": a.ultimo_erro(),
        "alertas": itens,
    })))
}

#[derive(Deserialize)]
pub struct ConfigArquivo {
    ligado: bool,
    #[serde(default)]
    chat_id: String,
    #[serde(default)]
    manter: Option<i64>,
}

pub async fn post_arquivo(State(app): State<Shared>, Json(c): Json<ConfigArquivo>) -> ApiResult {
    if c.ligado {
        exigir_tecnico(&app)?;
    }
    let a = arquivo(&app)?;
    let chat = c.chat_id.trim();
    let chat_id: Option<i64> = if chat.is_empty() {
        None
    } else {
        Some(chat.parse().map_err(|_| {
            bad(
                StatusCode::UNPROCESSABLE_ENTITY,
                "o id do canal é um número (ex.: -1001234567890)",
            )
        })?)
    };
    if let Some(id) = chat_id {
        // Confere se o bot enxerga o canal antes de gravar.
        let bot = app.telegram.get().and_then(|t| t.bot()).ok_or_else(|| {
            bad(
                StatusCode::PRECONDITION_FAILED,
                "configure o bot do Telegram antes de escolher o canal do arquivo",
            )
        })?;
        bot.get_chat(id)
            .await
            .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    }
    let manter = c.manter.unwrap_or(MANTER_PADRAO).clamp(1, MANTER_MAX);
    app.store
        .set(CHAVE_LIGADO, if c.ligado { "1" } else { "0" })
        .map_err(internal)?;
    app.store
        .set(
            CHAVE_CHAT,
            &chat_id.map(|v| v.to_string()).unwrap_or_default(),
        )
        .map_err(internal)?;
    app.store
        .set(CHAVE_MANTER, &manter.to_string())
        .map_err(internal)?;
    let _ = app.store.registrar_evento(
        0,
        None,
        "lgpd",
        if c.ligado {
            "arquivo de alertas ligado"
        } else {
            "arquivo de alertas desligado"
        },
        agora(),
    );
    *a.ultimo_erro.lock().unwrap() = None;
    Ok(Json(
        json!({ "ligado": c.ligado, "chat_id": chat_id, "manter": manter }),
    ))
}

/// Foto guardada: `/api/arquivo/<id>.jpg`.
pub async fn get_foto_arquivo(State(app): State<Shared>, Path(nome): Path<String>) -> Response {
    let Some(id) = nome
        .strip_suffix(".jpg")
        .and_then(|v| v.parse::<i64>().ok())
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(x) = app
        .store
        .arquivo_alerta(id)
        .ok()
        .flatten()
        .filter(|x| !x.arquivo.is_empty())
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match std::fs::read(pasta_cam(x.camera_id).join(&x.arquivo)) {
        Ok(b) => (
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (header::CACHE_CONTROL, "private, max-age=86400"),
            ],
            b,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_do_canal_privado() {
        assert_eq!(
            link_telegram(-1001234567890, 42).as_deref(),
            Some("https://t.me/c/1234567890/42")
        );
        assert_eq!(link_telegram(12345, 42), None);
        assert_eq!(link_telegram(-1001234567890, 0), None);
    }

    #[test]
    fn nome_da_foto() {
        assert_eq!(nome_foto(1_700_000_000, 7), "1700000000_000000007.jpg");
    }
}
