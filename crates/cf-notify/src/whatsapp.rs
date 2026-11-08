//! WhatsApp pela **Evolution API** (gateway HTTP; ADR-0010).
//!
//! Nada de biblioteca não oficial dentro do binário: a Evolution API roda como
//! contêiner Docker em outro servidor (ou no mesmo PC do painel) e o CamFácil
//! IA só fala HTTP com ela. Aqui ficam o cliente mínimo (`Evolution`) e o
//! serviço (`WhatsApp`): alertas com foto, antirrepique, fila offline e
//! comandos recebidos pelo webhook `POST /api/whatsapp/webhook/{token}`.
//!
//! Aviso obrigatório (PRD §5): WhatsApp não oficial pode ter o número
//! bloqueado; use um chip dedicado, nunca o seu número principal.

use crate::cofre::Cofre;
use crate::comandos::{self, Resposta};
use crate::servico::{Alerta, Anfitriao, Antirrepique, Fila, ANTIRREPIQUE, LIMITE_FILA};
use base64::prelude::*;
use cf_core::store::Store;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Notify};

const CHAVE_URL: &str = "wa_url";
const CHAVE_APIKEY: &str = "wa_apikey";
const CHAVE_INSTANCIA: &str = "wa_instancia";
const CHAVE_FOTOS: &str = "wa_fotos";
const CHAVE_TOKEN_WEBHOOK: &str = "wa_webhook_token";

/// Texto do aviso permanente da tela (PRD §5).
pub const AVISO: &str = "WhatsApp não oficial pode ter o número bloqueado; use um chip dedicado, nunca o seu número principal.";

// ---------------------------------------------------------------------------
// Cliente mínimo da Evolution API (v2)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Evolution {
    base: String,
    apikey: String,
    instancia: String,
    http: reqwest::Client,
}

fn cliente(tempo: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(tempo)
        .user_agent(concat!("CamFacilIA/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_default()
}

/// Situação da instância na Evolution.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum Conexao {
    /// Número conectado (`open`).
    Conectado,
    /// Instância existe, mas o número não leu o QR (`close`/`connecting`).
    Desconectado,
    /// A instância ainda não foi criada.
    Inexistente,
}

impl Evolution {
    pub fn new(base: &str, apikey: &str, instancia: &str) -> Self {
        Self {
            base: base.trim().trim_end_matches('/').to_string(),
            apikey: apikey.trim().to_string(),
            instancia: instancia.trim().to_string(),
            http: cliente(Duration::from_secs(20)),
        }
    }

    pub fn url_parece_valida(url: &str) -> bool {
        let u = url.trim();
        (u.starts_with("http://") || u.starts_with("https://")) && u.len() > 10
    }

    fn url(&self, caminho: &str) -> String {
        format!("{}/{caminho}", self.base)
    }

    async fn get(&self, caminho: &str) -> Result<(u16, Value), String> {
        let r = self
            .http
            .get(self.url(caminho))
            .header("apikey", &self.apikey)
            .send()
            .await
            .map_err(traduzir)?;
        ler(r).await
    }

    async fn post(&self, caminho: &str, corpo: Value) -> Result<(u16, Value), String> {
        let r = self
            .http
            .post(self.url(caminho))
            .header("apikey", &self.apikey)
            .json(&corpo)
            .send()
            .await
            .map_err(traduzir)?;
        ler(r).await
    }

    /// Testa URL e chave: lista as instâncias e diz como está a nossa.
    pub async fn testar(&self) -> Result<Conexao, String> {
        let (status, v) = self.get("instance/fetchInstances").await?;
        if status == 401 || status == 403 {
            return Err("chave da Evolution API recusada (confira AUTHENTICATION_API_KEY)".into());
        }
        if status >= 400 {
            return Err(format!("Evolution API respondeu {status}"));
        }
        let lista = v.as_array().cloned().unwrap_or_default();
        for i in &lista {
            // v2: { name, connectionStatus } · v1: { instance: { instanceName, status } }
            let nome = i
                .get("name")
                .or_else(|| i.pointer("/instance/instanceName"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if nome == self.instancia {
                let estado = i
                    .get("connectionStatus")
                    .or_else(|| i.pointer("/instance/status"))
                    .or_else(|| i.pointer("/instance/state"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                return Ok(if estado == "open" {
                    Conexao::Conectado
                } else {
                    Conexao::Desconectado
                });
            }
        }
        Ok(Conexao::Inexistente)
    }

    pub async fn estado(&self) -> Result<Conexao, String> {
        let (status, v) = self
            .get(&format!("instance/connectionState/{}", self.instancia))
            .await?;
        if status == 404 {
            return Ok(Conexao::Inexistente);
        }
        if status >= 400 {
            return Err(format!("Evolution API respondeu {status}"));
        }
        Ok(
            if v.pointer("/instance/state").and_then(Value::as_str) == Some("open") {
                Conexao::Conectado
            } else {
                Conexao::Desconectado
            },
        )
    }

    /// Cria a instância (Baileys) se não existir e pede o QR. Devolve o PNG em
    /// base64 (`data:image/png;base64,…`) ou `None` se o número já está conectado.
    pub async fn qr(&self) -> Result<Option<String>, String> {
        let (status, v) = self
            .get(&format!("instance/connect/{}", self.instancia))
            .await?;
        let v = if status == 404 {
            let (s2, criado) = self
                .post(
                    "instance/create",
                    json!({
                        "instanceName": self.instancia,
                        "integration": "WHATSAPP-BAILEYS",
                        "qrcode": true
                    }),
                )
                .await?;
            if s2 >= 400 {
                return Err(format!(
                    "não deu para criar a instância ({s2}): {}",
                    mensagem(&criado)
                ));
            }
            criado.get("qrcode").cloned().unwrap_or(criado)
        } else if status >= 400 {
            return Err(format!(
                "Evolution API respondeu {status}: {}",
                mensagem(&v)
            ));
        } else {
            v
        };
        if v.pointer("/instance/state").and_then(Value::as_str) == Some("open") {
            return Ok(None);
        }
        match v.get("base64").and_then(Value::as_str) {
            Some(b) if !b.is_empty() => Ok(Some(b.to_string())),
            _ => Err("a Evolution não devolveu o QR; tente de novo em alguns segundos".into()),
        }
    }

    /// Registra o webhook de mensagens recebidas apontando para o CamFácil IA.
    pub async fn definir_webhook(&self, url: &str) -> Result<(), String> {
        let (status, v) = self
            .post(
                &format!("webhook/set/{}", self.instancia),
                json!({
                    "webhook": {
                        "enabled": true,
                        "url": url,
                        "webhookByEvents": false,
                        "webhookBase64": false,
                        "events": ["MESSAGES_UPSERT"]
                    }
                }),
            )
            .await?;
        if status >= 400 {
            return Err(format!(
                "não deu para registrar o webhook ({status}): {}",
                mensagem(&v)
            ));
        }
        Ok(())
    }

    pub async fn send_text(&self, jid: &str, texto: &str) -> Result<(), String> {
        let (status, v) = self
            .post(
                &format!("message/sendText/{}", self.instancia),
                json!({ "number": numero(jid), "text": texto }),
            )
            .await?;
        checar_envio(status, &v)
    }

    pub async fn send_image(&self, jid: &str, jpeg: &[u8], legenda: &str) -> Result<(), String> {
        let (status, v) = self
            .post(
                &format!("message/sendMedia/{}", self.instancia),
                json!({
                    "number": numero(jid),
                    "mediatype": "image",
                    "mimetype": "image/jpeg",
                    "fileName": "foto.jpg",
                    "caption": legenda,
                    "media": BASE64_STANDARD.encode(jpeg)
                }),
            )
            .await?;
        checar_envio(status, &v)
    }
}

fn checar_envio(status: u16, v: &Value) -> Result<(), String> {
    if status == 400 && mensagem(v).to_lowercase().contains("exists") {
        return Err(format!("contato indisponível: {}", mensagem(v)));
    }
    if status >= 400 {
        return Err(format!("Evolution API respondeu {status}: {}", mensagem(v)));
    }
    Ok(())
}

/// `5511999999999@s.whatsapp.net` → `5511999999999`; grupos (`@g.us`) vão inteiros.
pub fn numero(jid: &str) -> String {
    match jid.strip_suffix("@s.whatsapp.net") {
        Some(n) => n.to_string(),
        None => jid.to_string(),
    }
}

fn mensagem(v: &Value) -> String {
    v.pointer("/response/message")
        .or_else(|| v.get("message"))
        .or_else(|| v.get("error"))
        .map(|m| match m {
            Value::String(s) => s.clone(),
            Value::Array(a) => a
                .iter()
                .filter_map(|x| x.as_str())
                .collect::<Vec<_>>()
                .join("; "),
            o => o.to_string(),
        })
        .unwrap_or_default()
}

async fn ler(r: reqwest::Response) -> Result<(u16, Value), String> {
    let status = r.status().as_u16();
    let texto = r.text().await.map_err(traduzir)?;
    let v = serde_json::from_str(&texto).unwrap_or(Value::Null);
    if v.is_null() && status < 400 {
        return Err("resposta inesperada da Evolution API (isso é mesmo a URL dela?)".into());
    }
    Ok((status, v))
}

fn traduzir(e: reqwest::Error) -> String {
    if e.is_timeout() {
        "sem resposta da Evolution API (servidor ligado?)".into()
    } else if e.is_connect() {
        "sem conexão com a Evolution API".into()
    } else {
        format!("falha de rede: {e}")
    }
}

// ---------------------------------------------------------------------------
// Mensagem recebida pelo webhook
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recebida {
    pub jid: String,
    pub nome: String,
    pub texto: String,
}

/// Extrai a mensagem de texto de um evento `messages.upsert` da Evolution.
/// Ignora o que foi mandado por nós (`fromMe`), status e tudo que não é texto.
pub fn interpretar_webhook(corpo: &Value) -> Vec<Recebida> {
    let evento = corpo.get("event").and_then(Value::as_str).unwrap_or("");
    if !evento.eq_ignore_ascii_case("messages.upsert")
        && !evento.eq_ignore_ascii_case("MESSAGES_UPSERT")
    {
        return Vec::new();
    }
    let dados: Vec<&Value> = match corpo.get("data") {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(d) => vec![d],
        None => Vec::new(),
    };
    dados
        .into_iter()
        .filter_map(|d| {
            if d.pointer("/key/fromMe").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            let jid = d.pointer("/key/remoteJid")?.as_str()?.to_string();
            if jid.starts_with("status@") {
                return None;
            }
            let texto = d
                .pointer("/message/conversation")
                .or_else(|| d.pointer("/message/extendedTextMessage/text"))
                .and_then(Value::as_str)?
                .to_string();
            Some(Recebida {
                jid,
                nome: d
                    .get("pushName")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                texto,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Serviço
// ---------------------------------------------------------------------------

#[derive(Default, Clone, Serialize)]
pub struct Situacao {
    pub configurado: bool,
    pub url: Option<String>,
    pub instancia: Option<String>,
    pub conexao: Option<Conexao>,
    pub ultimo_erro: Option<String>,
    pub na_fila: usize,
    pub enviados: u64,
    pub descartados: u64,
    pub recebidas: u64,
    pub fotos: bool,
    pub webhook_token: Option<String>,
    pub aviso: &'static str,
}

#[derive(Default)]
struct Interno {
    conexao: Option<Conexao>,
    ultimo_erro: Option<String>,
    na_fila: usize,
    enviados: u64,
    descartados: u64,
    recebidas: u64,
}

pub struct WhatsApp {
    store: Arc<Store>,
    cofre: Cofre,
    anfitriao: Arc<dyn Anfitriao>,
    tx: mpsc::Sender<Alerta>,
    acordar: Notify,
    interno: Mutex<Interno>,
    antirrepique: Mutex<Antirrepique>,
}

fn agora_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl WhatsApp {
    pub fn iniciar(store: Arc<Store>, anfitriao: Arc<dyn Anfitriao>) -> Result<Arc<Self>, String> {
        Self::iniciar_com(store, anfitriao, Cofre::abrir()?)
    }

    /// Mesmo que `iniciar`, com um cofre já aberto (testes).
    pub fn iniciar_com(
        store: Arc<Store>,
        anfitriao: Arc<dyn Anfitriao>,
        cofre: Cofre,
    ) -> Result<Arc<Self>, String> {
        let (tx, rx) = mpsc::channel(64);
        let s = Arc::new(Self {
            store,
            cofre,
            anfitriao,
            tx,
            acordar: Notify::new(),
            interno: Mutex::new(Interno::default()),
            antirrepique: Mutex::new(Antirrepique::new(ANTIRREPIQUE)),
        });
        tokio::spawn(s.clone().laco_envio(rx));
        Ok(s)
    }

    pub fn situacao(&self) -> Situacao {
        let i = self.interno.lock().unwrap();
        Situacao {
            configurado: self.evolution().is_some(),
            url: self
                .store
                .get(CHAVE_URL)
                .ok()
                .flatten()
                .filter(|u| !u.is_empty()),
            instancia: self
                .store
                .get(CHAVE_INSTANCIA)
                .ok()
                .flatten()
                .filter(|u| !u.is_empty()),
            conexao: i.conexao.clone(),
            ultimo_erro: i.ultimo_erro.clone(),
            na_fila: i.na_fila,
            enviados: i.enviados,
            descartados: i.descartados,
            recebidas: i.recebidas,
            fotos: self.fotos(),
            webhook_token: self.token_webhook().ok(),
            aviso: AVISO,
        }
    }

    fn evolution(&self) -> Option<Evolution> {
        let url = self.store.get(CHAVE_URL).ok().flatten()?;
        let inst = self.store.get(CHAVE_INSTANCIA).ok().flatten()?;
        let key = self
            .cofre
            .decifrar(&self.store.get(CHAVE_APIKEY).ok().flatten()?)?;
        if url.is_empty() || inst.is_empty() {
            return None;
        }
        Some(Evolution::new(&url, &key, &inst))
    }

    pub fn fotos(&self) -> bool {
        self.store.get(CHAVE_FOTOS).ok().flatten().as_deref() != Some("0")
    }

    pub fn definir_fotos(&self, ligado: bool) -> Result<(), String> {
        self.store
            .set(CHAVE_FOTOS, if ligado { "1" } else { "0" })
            .map_err(|e| e.to_string())
    }

    /// Token da URL do webhook (gerado uma vez, guardado cifrado).
    pub fn token_webhook(&self) -> Result<String, String> {
        if let Some(g) = self
            .store
            .get(CHAVE_TOKEN_WEBHOOK)
            .map_err(|e| e.to_string())?
        {
            if let Some(t) = self.cofre.decifrar(&g) {
                return Ok(t);
            }
        }
        use chacha20poly1305::aead::{KeyInit, OsRng};
        let t = hex(&chacha20poly1305::ChaCha20Poly1305::generate_key(&mut OsRng)[..16]);
        self.store
            .set(CHAVE_TOKEN_WEBHOOK, &self.cofre.cifrar(&t))
            .map_err(|e| e.to_string())?;
        Ok(t)
    }

    /// Salva URL, chave (cifrada) e instância depois de testar na Evolution.
    pub async fn configurar(
        &self,
        url: &str,
        apikey: &str,
        instancia: &str,
    ) -> Result<Conexao, String> {
        if !Evolution::url_parece_valida(url) {
            return Err("a URL da Evolution API precisa começar com http:// ou https://".into());
        }
        let inst = instancia.trim();
        if inst.is_empty()
            || !inst
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err("nome da instância: só letras, números, - e _".into());
        }
        if apikey.trim().is_empty() {
            return Err("informe a chave da Evolution API".into());
        }
        let ev = Evolution::new(url, apikey, inst);
        let conexao = ev.testar().await?;
        self.store
            .set(CHAVE_URL, url.trim().trim_end_matches('/'))
            .and_then(|_| self.store.set(CHAVE_INSTANCIA, inst))
            .and_then(|_| {
                self.store
                    .set(CHAVE_APIKEY, &self.cofre.cifrar(apikey.trim()))
            })
            .map_err(|e| e.to_string())?;
        {
            let mut i = self.interno.lock().unwrap();
            i.conexao = Some(conexao.clone());
            i.ultimo_erro = None;
        }
        self.acordar.notify_waiters();
        Ok(conexao)
    }

    pub fn remover(&self) -> Result<(), String> {
        for k in [CHAVE_URL, CHAVE_APIKEY, CHAVE_INSTANCIA] {
            self.store.set(k, "").map_err(|e| e.to_string())?;
        }
        let mut i = self.interno.lock().unwrap();
        i.conexao = None;
        i.ultimo_erro = None;
        Ok(())
    }

    /// Confere a conexão e manda uma mensagem de teste aos autorizados.
    pub async fn testar(&self) -> Result<(Conexao, usize), String> {
        let ev = self.evolution().ok_or("Evolution API não configurada")?;
        let c = ev.estado().await?;
        self.interno.lock().unwrap().conexao = Some(c.clone());
        if c != Conexao::Conectado {
            return Ok((c, 0));
        }
        let jids = self
            .store
            .whatsapp_autorizados()
            .map_err(|e| e.to_string())?;
        for j in &jids {
            ev.send_text(j, "CamFácil IA: teste de alerta. Tudo certo por aqui.")
                .await?;
        }
        Ok((c, jids.len()))
    }

    /// QR em base64 para o número ler, ou `None` se já está conectado.
    pub async fn qr(&self) -> Result<Option<String>, String> {
        let ev = self.evolution().ok_or("Evolution API não configurada")?;
        let r = ev.qr().await?;
        if r.is_none() {
            self.interno.lock().unwrap().conexao = Some(Conexao::Conectado);
        }
        Ok(r)
    }

    pub async fn registrar_webhook(&self, url_completa: &str) -> Result<(), String> {
        let ev = self.evolution().ok_or("Evolution API não configurada")?;
        ev.definir_webhook(url_completa).await
    }

    /// Entrada dos alertas (qualquer thread): antirrepique e fila; nunca bloqueia.
    pub fn notificar(&self, alerta: Alerta) {
        if self.evolution().is_none() {
            return;
        }
        if !self
            .antirrepique
            .lock()
            .unwrap()
            .permite(&alerta.chave, Instant::now())
        {
            return;
        }
        let alerta = if self.fotos() {
            alerta
        } else {
            Alerta {
                foto: None,
                ..alerta
            }
        };
        if self.tx.try_send(alerta).is_err() {
            self.interno.lock().unwrap().descartados += 1;
        }
    }

    /// Chegou um webhook da Evolution: registra o contato e atende comandos
    /// de quem está autorizado. Devolve quantas mensagens tratou.
    pub async fn receber_webhook(&self, token: &str, corpo: &Value) -> Result<usize, String> {
        if token.is_empty() || self.token_webhook()? != token {
            return Err("token do webhook não confere".into());
        }
        let Some(ev) = self.evolution() else {
            return Err("Evolution API não configurada".into());
        };
        let msgs = interpretar_webhook(corpo);
        let n = msgs.len();
        for m in msgs {
            self.interno.lock().unwrap().recebidas += 1;
            let _ = self
                .store
                .whatsapp_contato_visto(&m.jid, &m.nome, agora_unix());
            let autorizado = self
                .store
                .whatsapp_autorizados()
                .unwrap_or_default()
                .contains(&m.jid);
            if !autorizado {
                let r = format!(
                    "Oi! Sou o CamFácil IA.\nPara receber alertas e usar comandos, peça ao técnico para autorizar este número na dashboard (Integrações → WhatsApp).\nSeu número aqui: {}",
                    numero(&m.jid)
                );
                let _ = ev.send_text(&m.jid, &r).await;
                continue;
            }
            match comandos::responder(
                self.anfitriao.as_ref(),
                &self.store,
                &m.texto,
                "WhatsApp",
                &numero(&m.jid),
            ) {
                Resposta::Texto(t) => {
                    let _ = ev.send_text(&m.jid, &t).await;
                }
                Resposta::Foto { jpeg, legenda } => {
                    if let Err(e) = ev.send_image(&m.jid, &jpeg, &legenda).await {
                        let _ = ev.send_text(&m.jid, &e).await;
                    }
                }
                Resposta::Nada => {}
            }
        }
        Ok(n)
    }

    async fn laco_envio(self: Arc<Self>, mut rx: mpsc::Receiver<Alerta>) {
        let mut fila: Fila<Alerta> = Fila::new(LIMITE_FILA);
        let mut espera = Duration::from_secs(10);
        loop {
            let proximo = if fila.is_empty() {
                rx.recv().await.map(Some)
            } else {
                match tokio::time::timeout(espera, rx.recv()).await {
                    Ok(r) => r.map(Some),
                    Err(_) => Some(None),
                }
            };
            match proximo {
                None => return,
                Some(Some(a)) => fila.empurrar(a),
                Some(None) => {}
            }
            while let Ok(a) = rx.try_recv() {
                fila.empurrar(a);
            }
            let Some(ev) = self.evolution() else {
                fila.limpar();
                self.atualizar_fila(&fila);
                continue;
            };
            let jids = self.store.whatsapp_autorizados().unwrap_or_default();
            if jids.is_empty() {
                fila.limpar();
                self.atualizar_fila(&fila);
                continue;
            }
            while let Some(a) = fila.frente() {
                let mut rede_caiu = false;
                for j in &jids {
                    let r = match &a.foto {
                        Some(f) => ev.send_image(j, f, &a.texto).await,
                        None => ev.send_text(j, &a.texto).await,
                    };
                    if let Err(e) = r {
                        let indisponivel = e.contains("contato indisponível");
                        self.interno.lock().unwrap().ultimo_erro = Some(e);
                        if !indisponivel {
                            rede_caiu = true;
                            break;
                        }
                    }
                }
                if rede_caiu {
                    break;
                }
                fila.tirar();
                let mut i = self.interno.lock().unwrap();
                i.enviados += 1;
                i.ultimo_erro = None;
                i.conexao = Some(Conexao::Conectado);
            }
            espera = if fila.is_empty() {
                Duration::from_secs(10)
            } else {
                (espera * 2).min(Duration::from_secs(120))
            };
            self.atualizar_fila(&fila);
        }
    }

    fn atualizar_fila(&self, fila: &Fila<Alerta>) {
        let mut i = self.interno.lock().unwrap();
        i.na_fila = fila.len();
        i.descartados = i.descartados.max(fila.descartados);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Servidor HTTP mínimo: responde uma vez por item de `respostas`
    /// (status, corpo JSON) e devolve o que recebeu (linha de pedido + corpo).
    pub(crate) fn servidor(
        respostas: Vec<(u16, &'static str)>,
    ) -> (String, std::thread::JoinHandle<Vec<String>>) {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let mut vistos = Vec::new();
            for (status, corpo) in respostas {
                let (mut s, _) = l.accept().unwrap();
                let mut buf = vec![0u8; 65536];
                let mut n = 0;
                loop {
                    let k = s.read(&mut buf[n..]).unwrap();
                    n += k;
                    let txt = String::from_utf8_lossy(&buf[..n]).to_string();
                    if let Some(p) = txt.find("\r\n\r\n") {
                        let len = txt
                            .lines()
                            .find_map(|l| {
                                l.to_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if n >= p + 4 + len || k == 0 {
                            vistos.push(txt);
                            break;
                        }
                    }
                    if k == 0 {
                        break;
                    }
                }
                let r = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
                    corpo.len()
                );
                s.write_all(r.as_bytes()).unwrap();
            }
            vistos
        });
        (base, h)
    }

    #[test]
    fn interpreta_webhook_da_evolution() {
        let v: Value = serde_json::from_str(
            r#"{"event":"messages.upsert","instance":"loja","data":{"key":{"remoteJid":"5511999@s.whatsapp.net","fromMe":false,"id":"X"},"pushName":"Zé","message":{"conversation":"status"}}}"#,
        )
        .unwrap();
        assert_eq!(
            interpretar_webhook(&v),
            vec![Recebida {
                jid: "5511999@s.whatsapp.net".into(),
                nome: "Zé".into(),
                texto: "status".into()
            }]
        );
        let v: Value = serde_json::from_str(
            r#"{"event":"messages.upsert","data":{"key":{"remoteJid":"5511@s.whatsapp.net","fromMe":true},"message":{"conversation":"eu"}}}"#,
        )
        .unwrap();
        assert!(interpretar_webhook(&v).is_empty());
        let v: Value = serde_json::from_str(r#"{"event":"connection.update","data":{}}"#).unwrap();
        assert!(interpretar_webhook(&v).is_empty());
        assert_eq!(numero("5511@s.whatsapp.net"), "5511");
        assert_eq!(numero("123-456@g.us"), "123-456@g.us");
    }

    #[tokio::test]
    async fn cliente_contra_servidor_simulado() {
        let (base, h) = servidor(vec![
            (200, r#"[{"name":"loja","connectionStatus":"close"}]"#),
            (
                404,
                r#"{"status":404,"error":"Not Found","response":{"message":["loja nao existe"]}}"#,
            ),
            (
                201,
                r#"{"instance":{"instanceName":"loja"},"qrcode":{"base64":"data:image/png;base64,AAAA"}}"#,
            ),
            (201, r#"{"key":{"id":"1"}}"#),
            (201, r#"{"key":{"id":"2"}}"#),
            (200, r#"{"webhook":{"enabled":true}}"#),
        ]);
        let ev = Evolution::new(&base, "chave-secreta", "loja");
        assert_eq!(ev.testar().await.unwrap(), Conexao::Desconectado);
        assert_eq!(
            ev.qr().await.unwrap().as_deref(),
            Some("data:image/png;base64,AAAA")
        );
        ev.send_text("5511@s.whatsapp.net", "oi").await.unwrap();
        ev.send_image("5511@s.whatsapp.net", b"\xFF\xD8", "foto")
            .await
            .unwrap();
        ev.definir_webhook("http://10.0.0.2:7070/api/whatsapp/webhook/abc")
            .await
            .unwrap();
        let vistos = h.join().unwrap();
        assert!(vistos[0].starts_with("GET /instance/fetchInstances"));
        assert!(vistos[0].contains("apikey: chave-secreta"));
        assert!(vistos[1].starts_with("GET /instance/connect/loja"));
        assert!(vistos[2].starts_with("POST /instance/create"));
        assert!(vistos[2].contains("WHATSAPP-BAILEYS"));
        assert!(vistos[3].starts_with("POST /message/sendText/loja"));
        assert!(vistos[3].contains(r#""number":"5511""#));
        assert!(vistos[4].contains(r#""mediatype":"image""#));
        assert!(vistos[4].contains(r#""media":"/9g=""#));
        assert!(vistos[5].contains("MESSAGES_UPSERT"));
    }

    #[tokio::test]
    async fn servico_guarda_chave_cifrada_e_atende_webhook() {
        struct Falso;
        impl Anfitriao for Falso {
            fn status(&self) -> String {
                "tudo ok".into()
            }
            fn resumo(&self) -> String {
                String::new()
            }
            fn cameras(&self) -> Vec<(i64, String)> {
                Vec::new()
            }
            fn foto(&self, _: i64) -> Option<Vec<u8>> {
                None
            }
            fn modulo(&self, _: &str, _: bool) -> Result<String, String> {
                Ok(String::new())
            }
        }
        let dir = std::env::temp_dir().join(format!("cf-wa-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (base, h) = servidor(vec![
            (200, r#"[{"name":"loja","connectionStatus":"open"}]"#),
            (201, r#"{"key":{"id":"1"}}"#), // resposta ao não autorizado
            (201, r#"{"key":{"id":"2"}}"#), // resposta do status
        ]);
        let store = Arc::new(Store::memory().unwrap());
        let wa = WhatsApp::iniciar_com(
            store.clone(),
            Arc::new(Falso),
            Cofre::abrir_em(&dir).unwrap(),
        )
        .unwrap();
        assert!(!wa.situacao().configurado);
        assert_eq!(
            wa.configurar(&base, "chave-secreta", "loja").await.unwrap(),
            Conexao::Conectado
        );
        let guardado = store.get("wa_apikey").unwrap().unwrap();
        assert!(guardado.starts_with("enc:") && !guardado.contains("chave-secreta"));
        assert!(wa.situacao().configurado);
        let tok = wa.token_webhook().unwrap();
        assert_eq!(tok.len(), 32);
        assert_eq!(wa.token_webhook().unwrap(), tok);
        let msg: Value = serde_json::from_str(
            r#"{"event":"messages.upsert","data":{"key":{"remoteJid":"5511@s.whatsapp.net","fromMe":false},"pushName":"Zé","message":{"conversation":"status"}}}"#,
        )
        .unwrap();
        assert!(wa.receber_webhook("errado", &msg).await.is_err());
        assert_eq!(wa.receber_webhook(&tok, &msg).await.unwrap(), 1);
        let c = store.whatsapp_contatos().unwrap();
        assert_eq!(c[0].nome, "Zé");
        assert!(!c[0].autorizado);
        store
            .whatsapp_autorizar("5511@s.whatsapp.net", true)
            .unwrap();
        assert_eq!(wa.receber_webhook(&tok, &msg).await.unwrap(), 1);
        let vistos = h.join().unwrap();
        assert!(vistos[1].contains("autorizar este n"));
        assert!(vistos[2].contains("tudo ok"));
        wa.remover().unwrap();
        assert!(!wa.situacao().configurado);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
