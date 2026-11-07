//! Agente-Cam-Elo-IA: conexão de saída com o site (SDD-08).
//!
//! Só conecta depois que o técnico pede o pareamento pela primeira vez. Nenhuma
//! porta é aberta no roteador: o agente abre um WebSocket de saída para o site.

use cf_core::elo::{self, Identidade};
use cf_core::store::Store;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio_tungstenite::tungstenite::Message;

const VALIDADE_CODIGO: Duration = Duration::from_secs(600);

#[derive(Clone, Serialize)]
pub struct Pedido {
    pub pedido_id: String,
    pub email: String,
    #[serde(skip)]
    pub chave_dispositivo: String,
}

#[derive(Default)]
struct Estado {
    conectado: bool,
    agente_id: Option<String>,
    pareado: bool,
    dono_email: Option<String>,
    codigo: Option<(String, Instant)>,
    pedido: Option<Pedido>,
    erro: Option<String>,
    saida: Option<UnboundedSender<String>>,
}

pub struct Elo {
    pub site: String,
    id: Identidade,
    store: Arc<Store>,
    estado: Mutex<Estado>,
    iniciado: Mutex<bool>,
}

#[derive(Serialize)]
pub struct Status {
    pub ativo: bool,
    pub conectado: bool,
    pub site: String,
    pub impressao: String,
    pub pareado: bool,
    pub dono_email: Option<String>,
    pub link: Option<String>,
    pub expira_em_s: Option<u64>,
    pub pedido: Option<Pedido>,
    pub erro: Option<String>,
}

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Elo {
    /// Carrega (ou cria) a identidade do agente.
    pub fn carregar(store: Arc<Store>) -> Arc<Self> {
        let id = store
            .get("elo_chave")
            .ok()
            .flatten()
            .and_then(|s| Identidade::de_b64(&s))
            .unwrap_or_else(|| {
                let novo = Identidade::nova();
                let _ = store.set("elo_chave", &novo.para_b64());
                novo
            });
        let site = std::env::var("CAMFACIL_SITE")
            .ok()
            .or_else(|| store.get("elo_site").ok().flatten())
            .unwrap_or_else(|| elo::URL_PADRAO.to_string());
        Arc::new(Self {
            site,
            id,
            store,
            estado: Mutex::new(Estado::default()),
            iniciado: Mutex::new(false),
        })
    }

    fn ativo(&self) -> bool {
        self.store.get("elo_ativo").ok().flatten().as_deref() == Some("1")
    }

    /// Liga a conexão (uma vez). Chamado no início se já estava ativo, ou no
    /// primeiro pedido de pareamento.
    pub fn iniciar(self: &Arc<Self>) {
        let mut ini = self.iniciado.lock().unwrap();
        if *ini {
            return;
        }
        *ini = true;
        let _ = self.store.set("elo_ativo", "1");
        let me = self.clone();
        tokio::spawn(async move { me.laco().await });
    }

    pub fn iniciar_se_ativo(self: &Arc<Self>) {
        if self.ativo() {
            self.iniciar();
        }
    }

    fn enviar(&self, m: Value) -> bool {
        match &self.estado.lock().unwrap().saida {
            Some(tx) => tx.send(m.to_string()).is_ok(),
            None => false,
        }
    }

    pub fn status(&self) -> Status {
        let e = self.estado.lock().unwrap();
        let codigo = e
            .codigo
            .as_ref()
            .filter(|(_, t)| t.elapsed() < VALIDADE_CODIGO);
        Status {
            ativo: self.ativo(),
            conectado: e.conectado,
            site: self.site.clone(),
            impressao: self.id.impressao(),
            pareado: e.pareado,
            dono_email: e.dono_email.clone(),
            link: codigo.map(|(c, _)| elo::link_pareamento(&self.site, c, &self.id.impressao())),
            expira_em_s: codigo.map(|(_, t)| VALIDADE_CODIGO.saturating_sub(t.elapsed()).as_secs()),
            pedido: e.pedido.clone(),
            erro: e.erro.clone(),
        }
    }

    /// Gera um código novo e publica o hash no site.
    pub fn novo_codigo(self: &Arc<Self>) -> Result<Status, &'static str> {
        self.iniciar();
        let (codigo, hash) = elo::novo_codigo();
        {
            let mut e = self.estado.lock().unwrap();
            if e.pareado {
                return Err("este computador já está pareado; desparear antes");
            }
            e.codigo = Some((codigo, Instant::now()));
            e.pedido = None;
            e.erro = None;
        }
        if !self.enviar(json!({"tipo": "novo_codigo", "codigo_hash": hash})) {
            // Ainda conectando: o laço reenvia ao conectar.
            self.estado.lock().unwrap().erro = Some("conectando ao site…".into());
        }
        Ok(self.status())
    }

    /// Confirmação na tela local: só aqui um navegador vira autorizado.
    pub fn confirmar(&self, aceito: bool) -> Result<(), &'static str> {
        let pedido = self
            .estado
            .lock()
            .unwrap()
            .pedido
            .clone()
            .ok_or("nenhum pedido de pareamento pendente")?;
        if aceito {
            self.store
                .autorizar_dispositivo(&pedido.chave_dispositivo, &pedido.email, agora())
                .map_err(|_| "não foi possível salvar o dispositivo")?;
        }
        if !self.enviar(json!({"tipo": "confirmar_pareamento", "pedido_id": pedido.pedido_id, "aceito": aceito})) {
            return Err("sem conexão com o site; tente de novo");
        }
        Ok(())
    }

    pub fn desparear(&self) -> Result<(), &'static str> {
        let _ = self.store.limpar_dispositivos();
        if !self.enviar(json!({"tipo": "desparear"})) {
            return Err("sem conexão com o site; o desparear será feito ao reconectar");
        }
        Ok(())
    }

    async fn laco(self: Arc<Self>) {
        let url = elo::url_ws(&self.site);
        let mut espera = Duration::from_secs(2);
        loop {
            match self.sessao(&url).await {
                Ok(()) => espera = Duration::from_secs(2),
                Err(e) => {
                    let mut st = self.estado.lock().unwrap();
                    st.erro = Some(format!("sem conexão com o site ({e})"));
                }
            }
            {
                let mut st = self.estado.lock().unwrap();
                st.conectado = false;
                st.saida = None;
            }
            tokio::time::sleep(espera).await;
            espera = (espera * 2).min(Duration::from_secs(60));
        }
    }

    async fn sessao(&self, url: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (ws, _) = tokio::time::timeout(
            Duration::from_secs(10),
            tokio_tungstenite::connect_async(url),
        )
        .await??;
        let (mut sink, mut stream) = ws.split();

        // Desafio → assinatura.
        let nonce = match stream.next().await {
            Some(Ok(Message::Text(t))) => {
                let v: Value = serde_json::from_str(&t)?;
                v["nonce"].as_str().unwrap_or_default().to_string()
            }
            _ => return Err("site não enviou o desafio".into()),
        };
        let ola = json!({
            "tipo": "ola",
            "chave_publica": self.id.chave_publica(),
            "assinatura": self.id.assinar_desafio(&nonce),
            "versao": cf_core::VERSION,
        });
        sink.send(Message::Text(ola.to_string().into())).await?;

        let (tx, mut rx) = unbounded_channel::<String>();
        self.estado.lock().unwrap().saida = Some(tx.clone());

        let mut ping = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                m = stream.next() => {
                    let Some(m) = m else { break };
                    if let Message::Text(t) = m? {
                        if let Ok(v) = serde_json::from_str::<Value>(&t) {
                            self.tratar(&v, &tx);
                        }
                    }
                }
                Some(saida) = rx.recv() => sink.send(Message::Text(saida.into())).await?,
                _ = ping.tick() => sink.send(Message::Text(json!({"tipo": "ping"}).to_string().into())).await?,
            }
        }
        Ok(())
    }

    fn tratar(&self, v: &Value, tx: &UnboundedSender<String>) {
        let mut e = self.estado.lock().unwrap();
        match v["tipo"].as_str().unwrap_or("") {
            "bem_vindo" => {
                e.conectado = true;
                e.erro = None;
                e.agente_id = v["agente_id"].as_str().map(String::from);
                e.pareado = v["pareado"].as_bool().unwrap_or(false);
                e.dono_email = v["dono_email"].as_str().map(String::from);
                // Reenvia um código que foi gerado enquanto conectava.
                if !e.pareado {
                    if let Some((c, t)) = &e.codigo {
                        if t.elapsed() < VALIDADE_CODIGO {
                            let hash = elo::hash_codigo(c);
                            let _ = tx.send(
                                json!({"tipo": "novo_codigo", "codigo_hash": hash}).to_string(),
                            );
                        }
                    }
                }
            }
            "pedido_pareamento" => {
                e.pedido = Some(Pedido {
                    pedido_id: v["pedido_id"].as_str().unwrap_or_default().to_string(),
                    email: v["email"].as_str().unwrap_or_default().to_string(),
                    chave_dispositivo: v["dispositivo"]["chave_publica"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                });
            }
            "pareamento_concluido" => {
                let aceito = v["aceito"].as_bool().unwrap_or(false);
                if aceito {
                    e.pareado = true;
                    e.dono_email = e.pedido.as_ref().map(|p| p.email.clone());
                } else if let Some(p) = &e.pedido {
                    let _ = self.store.revogar_dispositivo(&p.chave_dispositivo);
                }
                e.pedido = None;
                e.codigo = None;
            }
            "revogar_dispositivo" => {
                if let Some(k) = v["chave_publica"].as_str() {
                    let _ = self.store.revogar_dispositivo(k);
                }
            }
            "despareado" => {
                e.pareado = false;
                e.dono_email = None;
                let _ = self.store.limpar_dispositivos();
            }
            "erro" => e.erro = v["mensagem"].as_str().map(String::from),
            "substituido" => e.erro = Some("outra instância deste agente se conectou".into()),
            _ => {}
        }
    }
}

/// Diagnóstico: abre a conexão segura e espera o desafio do site.
pub async fn testar(site: &str) -> Result<String, String> {
    let url = elo::url_ws(site);
    let (mut ws, _) = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async(&url),
    )
    .await
    .map_err(|_| "tempo esgotado ao conectar".to_string())?
    .map_err(|e| format!("não conectou em {url}: {e}"))?;
    match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
        Ok(Some(Ok(Message::Text(t)))) if t.contains("desafio") => {
            Ok("site respondeu com o desafio; pareamento disponível".into())
        }
        Ok(Some(Ok(_))) => Err("conectou, mas o servidor não é o site do CamFácil IA".into()),
        _ => Err("conectou, mas o site não respondeu".into()),
    }
}

/// QR code em SVG (módulos pretos em fundo branco, para qualquer celular ler).
pub fn qr_svg(texto: &str) -> Option<String> {
    let code = qrcode::QrCode::new(texto.as_bytes()).ok()?;
    Some(
        code.render::<qrcode::render::svg::Color>()
            .min_dimensions(240, 240)
            .quiet_zone(true)
            .dark_color(qrcode::render::svg::Color("#000000"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build(),
    )
}
