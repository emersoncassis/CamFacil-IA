//! Cliente mínimo da Bot API oficial do Telegram (SDD-04).
//!
//! Só o que o CamFácil IA usa: `getMe`, `getUpdates` (long polling, sem porta
//! aberta), `sendMessage` e `sendPhoto`. O multipart do envio de foto é montado
//! à mão para não puxar dependência extra.

use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "https://api.telegram.org";

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Usuario {
    pub id: i64,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Chat {
    pub id: i64,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

impl Chat {
    /// Nome amigável para a lista de contatos.
    pub fn nome(&self) -> String {
        self.title
            .clone()
            .or_else(|| self.first_name.clone())
            .or_else(|| self.username.as_ref().map(|u| format!("@{u}")))
            .unwrap_or_else(|| self.id.to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Mensagem {
    #[serde(default)]
    pub message_id: i64,
    pub chat: Chat,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Atualizacao {
    pub update_id: i64,
    #[serde(default)]
    pub message: Option<Mensagem>,
}

#[derive(Deserialize)]
#[serde(bound = "T: for<'a> Deserialize<'a>")]
struct Resposta<T> {
    ok: bool,
    #[serde(default = "nada")]
    result: Option<T>,
    #[serde(default)]
    description: Option<String>,
}

fn nada<T>() -> Option<T> {
    None
}

#[derive(Clone)]
pub struct Bot {
    token: String,
    http: reqwest::Client,
}

fn cliente(tempo: Duration) -> reqwest::Client {
    crate::tls::raizes_do_sistema(reqwest::Client::builder())
        .timeout(tempo)
        .user_agent(concat!("CamFacilIA/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

impl Bot {
    pub fn new(token: &str) -> Self {
        Self {
            token: token.trim().to_string(),
            // getUpdates espera até 30 s; folga de 15 s acima disso.
            http: cliente(Duration::from_secs(45)),
        }
    }

    pub fn token_parece_valido(token: &str) -> bool {
        let t = token.trim();
        let Some((id, resto)) = t.split_once(':') else {
            return false;
        };
        !id.is_empty()
            && id.chars().all(|c| c.is_ascii_digit())
            && resto.len() >= 30
            && resto
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    }

    fn url(&self, metodo: &str) -> String {
        // `CAMFACIL_TELEGRAM_API` só para testes com um servidor simulado.
        let base = std::env::var("CAMFACIL_TELEGRAM_API").unwrap_or_else(|_| API.to_string());
        format!("{base}/bot{}/{metodo}", self.token)
    }

    async fn chamar<T: for<'de> Deserialize<'de>>(
        &self,
        metodo: &str,
        corpo: Value,
    ) -> Result<T, String> {
        let r = self
            .http
            .post(self.url(metodo))
            .json(&corpo)
            .send()
            .await
            .map_err(traduzir)?;
        ler::<T>(r).await
    }

    /// Dados do bot; serve para testar o token.
    pub async fn get_me(&self) -> Result<Usuario, String> {
        let http = cliente(Duration::from_secs(10));
        let r = http.get(self.url("getMe")).send().await.map_err(traduzir)?;
        ler(r).await
    }

    /// Long polling: espera até `timeout_s` por novas mensagens.
    pub async fn get_updates(
        &self,
        offset: Option<i64>,
        timeout_s: u64,
    ) -> Result<Vec<Atualizacao>, String> {
        let mut corpo = json!({ "timeout": timeout_s, "allowed_updates": ["message"] });
        if let Some(o) = offset {
            corpo["offset"] = json!(o);
        }
        self.chamar("getUpdates", corpo).await
    }

    pub async fn send_message(&self, chat_id: i64, texto: &str) -> Result<(), String> {
        let _: Value = self
            .chamar(
                "sendMessage",
                json!({ "chat_id": chat_id, "text": texto, "disable_web_page_preview": true }),
            )
            .await?;
        Ok(())
    }

    /// Foto JPEG com legenda (multipart montado à mão).
    pub async fn send_photo(&self, chat_id: i64, jpeg: &[u8], legenda: &str) -> Result<(), String> {
        self.send_photo_id(chat_id, jpeg, legenda).await.map(|_| ())
    }

    /// Igual a `send_photo`, devolvendo o `message_id` (para o arquivo de alertas).
    pub async fn send_photo_id(
        &self,
        chat_id: i64,
        jpeg: &[u8],
        legenda: &str,
    ) -> Result<i64, String> {
        let limite = "----CamFacilIA7f3a9c";
        let corpo = multipart(
            limite,
            &[
                ("chat_id", chat_id.to_string().as_bytes(), None),
                ("caption", legenda.as_bytes(), None),
                ("photo", jpeg, Some(("foto.jpg", "image/jpeg"))),
            ],
        );
        let r = self
            .http
            .post(self.url("sendPhoto"))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={limite}"),
            )
            .body(corpo)
            .send()
            .await
            .map_err(traduzir)?;
        let v: Value = ler(r).await?;
        Ok(v.get("message_id").and_then(Value::as_i64).unwrap_or(0))
    }

    /// Apaga uma mensagem (num chat privado o bot pode apagar as recebidas
    /// com menos de 48 h).
    pub async fn delete_message(&self, chat_id: i64, message_id: i64) -> Result<(), String> {
        let _: Value = self
            .chamar(
                "deleteMessage",
                json!({ "chat_id": chat_id, "message_id": message_id }),
            )
            .await?;
        Ok(())
    }

    /// Dados de um chat (título, usuário) para montar o link do arquivo.
    pub async fn get_chat(&self, chat_id: i64) -> Result<Chat, String> {
        self.chamar("getChat", json!({ "chat_id": chat_id })).await
    }
}

impl Bot {
    /// Arquivo (PDF, backup) com legenda, até 50 MB pela Bot API.
    pub async fn send_document(
        &self,
        chat_id: i64,
        nome: &str,
        bytes: &[u8],
        legenda: &str,
    ) -> Result<(), String> {
        let limite = "----CamFacilIA7f3a9d";
        let corpo = multipart(
            limite,
            &[
                ("chat_id", chat_id.to_string().as_bytes(), None),
                ("caption", legenda.as_bytes(), None),
                ("document", bytes, Some((nome, "application/octet-stream"))),
            ],
        );
        let r = self
            .http
            .post(self.url("sendDocument"))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={limite}"),
            )
            .body(corpo)
            .send()
            .await
            .map_err(traduzir)?;
        let _: Value = ler(r).await?;
        Ok(())
    }
}

async fn ler<T: for<'de> Deserialize<'de>>(r: reqwest::Response) -> Result<T, String> {
    let status = r.status();
    let texto = r.text().await.map_err(traduzir)?;
    let resp: Resposta<T> = serde_json::from_str(&texto).map_err(|_| {
        if status.as_u16() == 401 || status.as_u16() == 404 {
            "token recusado pelo Telegram".to_string()
        } else {
            format!("resposta inesperada do Telegram ({status})")
        }
    })?;
    if resp.ok {
        resp.result
            .ok_or_else(|| "resposta vazia do Telegram".to_string())
    } else {
        let d = resp.description.unwrap_or_default();
        Err(if status.as_u16() == 401 || d.contains("Unauthorized") {
            "token recusado pelo Telegram (confira com o @BotFather)".into()
        } else if d.contains("chat not found") || d.contains("blocked") {
            format!("contato indisponível: {d}")
        } else {
            format!("Telegram: {d}")
        })
    }
}

fn traduzir(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "sem resposta do Telegram (internet?)".into()
    } else if e.is_connect() {
        "sem conexão com a internet".into()
    } else {
        format!("falha de rede: {e}")
    }
}

/// Um campo do multipart: (nome, valor, Some((nome_do_arquivo, mime)) se for arquivo).
pub type Campo<'a> = (&'a str, &'a [u8], Option<(&'a str, &'a str)>);

/// Corpo `multipart/form-data`: campos de texto e um arquivo.
pub fn multipart(limite: &str, campos: &[Campo<'_>]) -> Vec<u8> {
    let mut b = Vec::new();
    for (nome, valor, arquivo) in campos {
        b.extend_from_slice(format!("--{limite}\r\n").as_bytes());
        match arquivo {
            Some((fname, mime)) => b.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{nome}\"; filename=\"{fname}\"\r\nContent-Type: {mime}\r\n\r\n"
                )
                .as_bytes(),
            ),
            None => b.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{nome}\"\r\n\r\n").as_bytes(),
            ),
        }
        b.extend_from_slice(valor);
        b.extend_from_slice(b"\r\n");
    }
    b.extend_from_slice(format!("--{limite}--\r\n").as_bytes());
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valida_formato_do_token() {
        assert!(Bot::token_parece_valido(
            "123456789:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw0"
        ));
        assert!(!Bot::token_parece_valido("abc"));
        assert!(!Bot::token_parece_valido("12:curto"));
        assert!(!Bot::token_parece_valido(
            "x123:AAHdqTcvCH1vGWJxfSeofSAs0K5PALDsaw0"
        ));
    }

    #[test]
    fn monta_multipart() {
        let b = multipart(
            "LIM",
            &[
                ("chat_id", b"42", None),
                ("photo", b"\xFF\xD8", Some(("f.jpg", "image/jpeg"))),
            ],
        );
        let s = String::from_utf8_lossy(&b);
        assert!(s.starts_with(
            "--LIM\r\nContent-Disposition: form-data; name=\"chat_id\"\r\n\r\n42\r\n"
        ));
        assert!(s.contains("filename=\"f.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n"));
        assert!(s.ends_with("--LIM--\r\n"));
    }

    #[test]
    fn nome_do_chat() {
        let c: Chat = serde_json::from_str(r#"{"id":1,"username":"ze"}"#).unwrap();
        assert_eq!(c.nome(), "@ze");
        let c: Chat =
            serde_json::from_str(r#"{"id":1,"first_name":"Zé","username":"ze"}"#).unwrap();
        assert_eq!(c.nome(), "Zé");
    }
}
