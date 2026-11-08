//! Serviço do Telegram: alertas com foto, comandos e contatos autorizados.
//!
//! Duas tarefas leves no runtime do programa:
//! - **envio**: recebe alertas das threads de vídeo por um canal, guarda numa
//!   fila em memória com limite (offline descarta os mais velhos) e manda para
//!   todos os contatos autorizados;
//! - **escuta**: long polling (`getUpdates`, 30 s) só enquanto há token;
//!   registra quem fala com o bot para o técnico aprovar na dashboard e atende
//!   os comandos dos autorizados.

use crate::cofre::Cofre;
use crate::telegram::{Atualizacao, Bot, Usuario};
use cf_core::store::Store;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Notify};

/// Mínimo entre dois alertas da mesma regra (antirrepique).
pub const ANTIRREPIQUE: Duration = Duration::from_secs(30);
/// Alertas guardados enquanto não dá para enviar; acima disso, o mais velho sai.
pub const LIMITE_FILA: usize = 40;
const CHAVE_TOKEN: &str = "telegram_token";
const CHAVE_FOTOS: &str = "telegram_fotos";

/// Um alerta pronto para enviar.
#[derive(Debug, Clone)]
pub struct Alerta {
    /// Chave do antirrepique: `regra:<id>` ou `saude:<camera_id>`.
    pub chave: String,
    /// Texto da mensagem/legenda (já em pt-BR, com hora e nome da regra).
    pub texto: String,
    /// Foto do momento (JPEG), se houver.
    pub foto: Option<Arc<Vec<u8>>>,
}

/// O que o programa oferece ao bot (implementado pelo binário).
pub trait Anfitriao: Send + Sync {
    /// Texto do `/status`.
    fn status(&self) -> String;
    /// Texto do `/resumo` (alertas do dia).
    fn resumo(&self) -> String;
    /// Câmeras ativas: (id, nome), na ordem da dashboard.
    fn cameras(&self) -> Vec<(i64, String)>;
    /// Última imagem da câmera.
    fn foto(&self, camera_id: i64) -> Option<Vec<u8>>;
    /// Liga/desliga um módulo pelo nome falado; devolve a frase de resposta.
    fn modulo(&self, nome: &str, ligado: bool) -> Result<String, String>;
}

/// Antirrepique por chave.
pub struct Antirrepique {
    ultimo: HashMap<String, Instant>,
    minimo: Duration,
}

impl Antirrepique {
    pub fn new(minimo: Duration) -> Self {
        Self {
            ultimo: HashMap::new(),
            minimo,
        }
    }

    /// Diz se a chave pode disparar agora (e marca o instante se sim).
    pub fn permite(&mut self, chave: &str, agora: Instant) -> bool {
        if let Some(t) = self.ultimo.get(chave) {
            if agora.saturating_duration_since(*t) < self.minimo {
                return false;
            }
        }
        self.ultimo.insert(chave.to_string(), agora);
        if self.ultimo.len() > 1000 {
            self.ultimo
                .retain(|_, t| agora.saturating_duration_since(*t) < self.minimo);
        }
        true
    }
}

/// Fila em memória com limite: offline, guarda os mais recentes.
pub struct Fila<T> {
    itens: VecDeque<T>,
    limite: usize,
    pub descartados: u64,
}

impl<T> Fila<T> {
    pub fn new(limite: usize) -> Self {
        Self {
            itens: VecDeque::new(),
            limite,
            descartados: 0,
        }
    }
    pub fn empurrar(&mut self, t: T) {
        if self.itens.len() >= self.limite {
            self.itens.pop_front();
            self.descartados += 1;
        }
        self.itens.push_back(t);
    }
    pub fn frente(&self) -> Option<&T> {
        self.itens.front()
    }
    pub fn tirar(&mut self) -> Option<T> {
        self.itens.pop_front()
    }
    pub fn len(&self) -> usize {
        self.itens.len()
    }
    pub fn is_empty(&self) -> bool {
        self.itens.is_empty()
    }
    pub fn limpar(&mut self) {
        self.itens.clear();
    }
}

#[derive(Default, Clone, Serialize)]
pub struct Situacao {
    pub configurado: bool,
    pub bot: Option<String>,
    pub escutando: bool,
    pub ultimo_erro: Option<String>,
    pub na_fila: usize,
    pub enviados: u64,
    pub descartados: u64,
    pub fotos: bool,
}

#[derive(Default)]
struct Interno {
    bot: Option<String>,
    escutando: bool,
    ultimo_erro: Option<String>,
    na_fila: usize,
    enviados: u64,
    descartados: u64,
}

pub struct Telegram {
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

impl Telegram {
    /// Cria o serviço e sobe as duas tarefas no runtime atual.
    pub fn iniciar(store: Arc<Store>, anfitriao: Arc<dyn Anfitriao>) -> Result<Arc<Self>, String> {
        let (tx, rx) = mpsc::channel(64);
        let s = Arc::new(Self {
            store,
            cofre: Cofre::abrir()?,
            anfitriao,
            tx,
            acordar: Notify::new(),
            interno: Mutex::new(Interno::default()),
            antirrepique: Mutex::new(Antirrepique::new(ANTIRREPIQUE)),
        });
        tokio::spawn(s.clone().laco_envio(rx));
        tokio::spawn(s.clone().laco_escuta());
        Ok(s)
    }

    pub fn situacao(&self) -> Situacao {
        let i = self.interno.lock().unwrap();
        Situacao {
            configurado: self.token().is_some(),
            bot: i.bot.clone(),
            escutando: i.escutando,
            ultimo_erro: i.ultimo_erro.clone(),
            na_fila: i.na_fila,
            enviados: i.enviados,
            descartados: i.descartados,
            fotos: self.fotos(),
        }
    }

    fn token(&self) -> Option<String> {
        let g = self.store.get(CHAVE_TOKEN).ok().flatten()?;
        self.cofre.decifrar(&g)
    }

    /// Cliente da Bot API com o token salvo (usado também pelo arquivo de alertas).
    pub fn bot(&self) -> Option<Bot> {
        self.token().map(|t| Bot::new(&t))
    }

    pub fn fotos(&self) -> bool {
        self.store.get(CHAVE_FOTOS).ok().flatten().as_deref() != Some("0")
    }

    pub fn definir_fotos(&self, ligado: bool) -> Result<(), String> {
        self.store
            .set(CHAVE_FOTOS, if ligado { "1" } else { "0" })
            .map_err(|e| e.to_string())
    }

    /// Testa o token no Telegram e, se válido, guarda cifrado.
    pub async fn definir_token(&self, token: &str) -> Result<Usuario, String> {
        if !Bot::token_parece_valido(token) {
            return Err("isso não parece um token de bot (formato 123456:ABC…)".into());
        }
        let bot = Bot::new(token);
        let eu = bot.get_me().await?;
        self.store
            .set(CHAVE_TOKEN, &self.cofre.cifrar(token.trim()))
            .map_err(|e| e.to_string())?;
        {
            let mut i = self.interno.lock().unwrap();
            i.bot = eu.username.clone();
            i.ultimo_erro = None;
        }
        self.acordar.notify_waiters();
        Ok(eu)
    }

    /// Só testa (sem gravar).
    pub async fn testar(&self) -> Result<Usuario, String> {
        let bot = self.bot().ok_or("nenhum token salvo")?;
        let eu = bot.get_me().await?;
        self.interno.lock().unwrap().bot = eu.username.clone();
        Ok(eu)
    }

    pub fn remover_token(&self) -> Result<(), String> {
        self.store.set(CHAVE_TOKEN, "").map_err(|e| e.to_string())?;
        let mut i = self.interno.lock().unwrap();
        i.bot = None;
        i.escutando = false;
        drop(i);
        self.acordar.notify_waiters();
        Ok(())
    }

    /// Manda uma mensagem de teste a todos os autorizados.
    pub async fn enviar_teste(&self) -> Result<usize, String> {
        let bot = self.bot().ok_or("nenhum token salvo")?;
        let chats = self
            .store
            .telegram_autorizados()
            .map_err(|e| e.to_string())?;
        if chats.is_empty() {
            return Err("nenhum contato autorizado ainda".into());
        }
        for c in &chats {
            bot.send_message(*c, "CamFácil IA: teste de alerta. Tudo certo por aqui.")
                .await?;
        }
        Ok(chats.len())
    }

    /// Manda um arquivo (relatório PDF, backup) a todos os autorizados, na
    /// hora, sem passar pela fila. Devolve quantos receberam.
    pub async fn enviar_arquivo(
        &self,
        nome: &str,
        bytes: Vec<u8>,
        legenda: &str,
    ) -> Result<usize, String> {
        let bot = self.bot().ok_or("nenhum token do Telegram salvo")?;
        let chats = self
            .store
            .telegram_autorizados()
            .map_err(|e| e.to_string())?;
        if chats.is_empty() {
            return Err("nenhum contato autorizado no Telegram".into());
        }
        for c in &chats {
            bot.send_document(*c, nome, &bytes, legenda).await?;
        }
        self.interno.lock().unwrap().enviados += chats.len() as u64;
        Ok(chats.len())
    }

    /// Entrada dos alertas (pode ser chamada de qualquer thread). Aplica o
    /// antirrepique por chave e entrega à fila; nunca bloqueia.
    pub fn notificar(&self, alerta: Alerta) {
        if self.token().is_none() {
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
                None => return, // canal fechado: programa encerrando
                Some(Some(a)) => fila.empurrar(a),
                Some(None) => {}
            }
            // Esvazia tudo que chegou junto.
            while let Ok(a) = rx.try_recv() {
                fila.empurrar(a);
            }
            let Some(bot) = self.bot() else {
                fila.limpar();
                self.atualizar_fila(&fila);
                continue;
            };
            let chats = self.store.telegram_autorizados().unwrap_or_default();
            if chats.is_empty() {
                fila.limpar();
                self.atualizar_fila(&fila);
                continue;
            }
            while let Some(a) = fila.frente() {
                let mut rede_caiu = false;
                for c in &chats {
                    let r = match &a.foto {
                        Some(f) => bot.send_photo(*c, f, &a.texto).await,
                        None => bot.send_message(*c, &a.texto).await,
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

    async fn laco_escuta(self: Arc<Self>) {
        let mut offset: Option<i64> = None;
        let mut espera = Duration::from_secs(5);
        loop {
            let Some(bot) = self.bot() else {
                self.interno.lock().unwrap().escutando = false;
                // Sem token: dorme até alguém salvar um (ou confere a cada minuto).
                let _ =
                    tokio::time::timeout(Duration::from_secs(60), self.acordar.notified()).await;
                continue;
            };
            self.interno.lock().unwrap().escutando = true;
            let r = tokio::select! {
                r = bot.get_updates(offset, 30) => r,
                _ = self.acordar.notified() => { offset = None; continue; }
            };
            match r {
                Ok(lista) => {
                    espera = Duration::from_secs(5);
                    for u in lista {
                        offset = Some(u.update_id + 1);
                        self.atender(&bot, u).await;
                    }
                }
                Err(e) => {
                    let token_ruim = e.contains("token recusado");
                    {
                        let mut i = self.interno.lock().unwrap();
                        i.ultimo_erro = Some(e);
                        i.escutando = false;
                    }
                    let d = if token_ruim {
                        Duration::from_secs(300)
                    } else {
                        espera
                    };
                    let _ = tokio::time::timeout(d, self.acordar.notified()).await;
                    espera = (espera * 2).min(Duration::from_secs(60));
                }
            }
        }
    }

    async fn atender(&self, bot: &Bot, u: Atualizacao) {
        let Some(m) = u.message else { return };
        let chat = m.chat;
        let texto = m.text.unwrap_or_default();
        let _ = self
            .store
            .telegram_chat_visto(chat.id, &chat.nome(), agora_unix());
        let autorizado = self
            .store
            .telegram_autorizados()
            .unwrap_or_default()
            .contains(&chat.id);
        if !autorizado {
            let r = format!(
                "Oi! Sou o CamFácil IA.\nPara receber alertas e usar comandos, peça ao técnico para autorizar este contato na dashboard (Integrações → Telegram).\nNúmero deste chat: {}",
                chat.id
            );
            let _ = bot.send_message(chat.id, &r).await;
            return;
        }
        let (cmd, arg) = separar(&texto);
        let resposta = match cmd.as_str() {
            "/start" | "/ajuda" | "/help" => AJUDA.to_string(),
            "/status" => self.anfitriao.status(),
            "/resumo" => self.anfitriao.resumo(),
            "/liga" | "/desliga" => {
                let ligar = cmd == "/liga";
                if arg.is_empty() {
                    "Diga qual: /liga movimento · cerca · saude · pessoas · contagem · permanencia · calor".into()
                } else {
                    let r = self.anfitriao.modulo(&arg, ligar);
                    let _ = self.store.registrar_evento(
                        0,
                        None,
                        "comando",
                        &format!("{} {} via Telegram ({})", cmd, arg, chat.id),
                        agora_unix(),
                    );
                    r.unwrap_or_else(|e| e)
                }
            }
            "/foto" => {
                let cams = self.anfitriao.cameras();
                match escolher_camera(&cams, &arg) {
                    Ok((id, nome)) => match self.anfitriao.foto(id) {
                        Some(jpeg) => {
                            let leg = format!("{nome} · {}", hora_local(&self.store));
                            if let Err(e) = bot.send_photo(chat.id, &jpeg, &leg).await {
                                let _ = bot.send_message(chat.id, &e).await;
                            }
                            return;
                        }
                        None => format!("{nome}: ainda sem imagem (câmera fora do ar?)."),
                    },
                    Err(msg) => msg,
                }
            }
            _ if cmd.starts_with('/') => {
                format!("Não conheço {cmd}.\n\n{AJUDA}")
            }
            _ => return,
        };
        let _ = bot.send_message(chat.id, &resposta).await;
    }
}

const AJUDA: &str = "Comandos:\n/status — câmeras, saúde e módulos\n/foto <nome ou nº> — foto da câmera agora\n/resumo — alertas de hoje\n/liga <módulo> · /desliga <módulo>\n/ajuda — esta lista";

/// Separa `/comando@bot argumento` em (`/comando`, `argumento`).
pub fn separar(texto: &str) -> (String, String) {
    let t = texto.trim();
    let (c, a) = t.split_once(char::is_whitespace).unwrap_or((t, ""));
    let c = c.split('@').next().unwrap_or(c).to_lowercase();
    (c, a.trim().to_string())
}

/// `/foto 2`, `/foto portão` ou `/foto` (uma câmera só).
pub fn escolher_camera(cams: &[(i64, String)], arg: &str) -> Result<(i64, String), String> {
    if cams.is_empty() {
        return Err("Nenhuma câmera ligada.".into());
    }
    let lista = || {
        cams.iter()
            .enumerate()
            .map(|(i, (_, n))| format!("{}. {n}", i + 1))
            .collect::<Vec<_>>()
            .join("\n")
    };
    if arg.is_empty() {
        return if cams.len() == 1 {
            Ok(cams[0].clone())
        } else {
            Err(format!("Qual câmera? Use /foto <nº ou nome>:\n{}", lista()))
        };
    }
    if let Ok(n) = arg.parse::<usize>() {
        if n >= 1 && n <= cams.len() {
            return Ok(cams[n - 1].clone());
        }
    }
    let a = arg.to_lowercase();
    let achadas: Vec<&(i64, String)> = cams
        .iter()
        .filter(|(_, n)| n.to_lowercase().contains(&a))
        .collect();
    match achadas.len() {
        1 => Ok(achadas[0].clone()),
        0 => Err(format!("Não achei a câmera \"{arg}\". Tenho:\n{}", lista())),
        _ => Err(format!(
            "Mais de uma câmera combina com \"{arg}\":\n{}",
            lista()
        )),
    }
}

/// Hora local `HH:MM` sem crate de datas: relógio UTC mais o fuso que a
/// dashboard informa (`settings.fuso_min`, o do navegador do técnico).
pub fn hora_local(store: &Store) -> String {
    hora_com_offset(agora_unix(), offset_local_s(store))
}

pub fn hora_com_offset(unix: i64, offset_s: i64) -> String {
    let t = (unix + offset_s).rem_euclid(86_400);
    format!("{:02}:{:02}", t / 3600, (t % 3600) / 60)
}

/// Deslocamento do fuso local em segundos (`fuso_min`, em minutos, gravado
/// pela dashboard). Sem informação, o padrão de Brasília (UTC−3).
pub fn offset_local_s(store: &Store) -> i64 {
    store
        .get("fuso_min")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<i64>().ok())
        .map(|m| m * 60)
        .unwrap_or(-3 * 3600)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn antirrepique_por_regra() {
        let t0 = Instant::now();
        let mut a = Antirrepique::new(Duration::from_secs(30));
        assert!(a.permite("regra:1", t0));
        assert!(!a.permite("regra:1", t0 + Duration::from_secs(10)));
        assert!(a.permite("regra:2", t0 + Duration::from_secs(10)));
        assert!(a.permite("regra:1", t0 + Duration::from_secs(30)));
    }

    #[test]
    fn fila_descarta_o_mais_velho() {
        let mut f = Fila::new(3);
        for i in 1..=5 {
            f.empurrar(i);
        }
        assert_eq!(f.len(), 3);
        assert_eq!(f.descartados, 2);
        assert_eq!(f.tirar(), Some(3));
        assert_eq!(f.frente(), Some(&4));
    }

    #[test]
    fn comandos_e_cameras() {
        assert_eq!(
            separar("/Foto@CamFacilBot  Portão "),
            ("/foto".into(), "Portão".into())
        );
        let cams = vec![(7, "Loja".to_string()), (9, "Portão".to_string())];
        assert_eq!(escolher_camera(&cams, "2").unwrap().0, 9);
        assert_eq!(escolher_camera(&cams, "port").unwrap().0, 9);
        assert!(escolher_camera(&cams, "").unwrap_err().contains("1. Loja"));
        assert!(escolher_camera(&cams, "x").is_err());
        assert_eq!(escolher_camera(&cams[..1], "").unwrap().0, 7);
        assert_eq!(hora_com_offset(0, -3 * 3600), "21:00");
    }
}
