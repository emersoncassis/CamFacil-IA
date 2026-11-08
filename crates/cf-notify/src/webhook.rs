//! Webhook genérico (SDD-04): um `POST` com JSON por alerta para a URL que o
//! técnico configurar (Home Assistant, n8n, Node-RED…). Cabeçalho opcional
//! (ex.: `Authorization: Bearer …`) guardado cifrado. Retentativa 3× com
//! espera crescente; antirrepique de 30 s por regra, como nos outros canais.

use crate::cofre::Cofre;
use crate::servico::{hora_com_offset, offset_local_s, Alerta, Antirrepique, ANTIRREPIQUE};
use cf_core::store::Store;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

const CHAVE_URL: &str = "wh_url";
const CHAVE_CABECALHO: &str = "wh_cabecalho";
/// Tentativas por evento (a primeira mais duas repetições).
pub const TENTATIVAS: u32 = 3;

/// O JSON que sai em cada alerta.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Evento {
    pub programa: &'static str,
    pub versao: &'static str,
    /// `regra`, `permanencia`, `saude` ou `teste`.
    pub tipo: String,
    pub camera_id: i64,
    pub camera: String,
    pub regra_id: Option<i64>,
    pub regra: Option<String>,
    /// Unix (segundos) e hora local `HH:MM`.
    pub quando: i64,
    pub hora: String,
    /// Texto pronto em pt-BR, igual ao do Telegram.
    pub texto: String,
    /// Link da foto do instante na API local (quando o arquivo guardou uma).
    pub foto_url: Option<String>,
}

fn agora_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Lê a origem de um alerta pela chave do antirrepique (`regra:<id>`,
/// `permanencia:<id>`, `saude:<camera_id>`) e monta o evento.
pub fn evento_de(store: &Store, a: &Alerta, foto_url: Option<String>) -> Evento {
    let (tipo, id) = a.chave.split_once(':').unwrap_or((a.chave.as_str(), "0"));
    let id: i64 = id.parse().unwrap_or(0);
    let cams = store.cameras().unwrap_or_default();
    let nome_cam = |cid: i64| {
        cams.iter()
            .find(|c| c.id == cid)
            .map(|c| c.nome.clone())
            .unwrap_or_else(|| format!("câmera {cid}"))
    };
    let (camera_id, regra) = match tipo {
        "saude" => (id, None),
        _ => {
            let r = cams
                .iter()
                .flat_map(|c| store.regras(c.id).unwrap_or_default())
                .find(|r| r.id == id);
            match r {
                Some(r) => (r.camera_id, Some((r.id, r.nome))),
                None => (0, Some((id, "regra".to_string()))),
            }
        }
    };
    let quando = agora_unix();
    Evento {
        programa: "CamFácil IA",
        versao: cf_core::VERSION,
        tipo: tipo.to_string(),
        camera_id,
        camera: nome_cam(camera_id),
        regra_id: regra.as_ref().map(|(i, _)| *i),
        regra: regra.map(|(_, n)| n),
        quando,
        hora: hora_com_offset(quando, offset_local_s(store)),
        texto: a.texto.clone(),
        foto_url,
    }
}

#[derive(Default, Clone, Serialize)]
pub struct Situacao {
    pub configurado: bool,
    pub url: Option<String>,
    pub cabecalho_nome: Option<String>,
    pub enviados: u64,
    pub falhas: u64,
    pub ultimo_erro: Option<String>,
    pub na_fila: usize,
}

#[derive(Default)]
struct Interno {
    enviados: u64,
    falhas: u64,
    ultimo_erro: Option<String>,
    na_fila: usize,
}

pub struct Webhook {
    store: Arc<Store>,
    cofre: Cofre,
    tx: mpsc::Sender<Evento>,
    interno: Mutex<Interno>,
    antirrepique: Mutex<Antirrepique>,
    http: reqwest::Client,
}

impl Webhook {
    pub fn iniciar(store: Arc<Store>) -> Result<Arc<Self>, String> {
        Self::iniciar_com(store, Cofre::abrir()?)
    }

    pub fn iniciar_com(store: Arc<Store>, cofre: Cofre) -> Result<Arc<Self>, String> {
        let (tx, rx) = mpsc::channel(64);
        let s = Arc::new(Self {
            store,
            cofre,
            tx,
            interno: Mutex::new(Interno::default()),
            antirrepique: Mutex::new(Antirrepique::new(ANTIRREPIQUE)),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent(concat!("CamFacilIA/", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap_or_default(),
        });
        tokio::spawn(s.clone().laco(rx));
        Ok(s)
    }

    pub fn url(&self) -> Option<String> {
        self.store
            .get(CHAVE_URL)
            .ok()
            .flatten()
            .filter(|u| !u.is_empty())
    }

    /// Cabeçalho extra `(nome, valor)`, decifrado.
    fn cabecalho(&self) -> Option<(String, String)> {
        let g = self.store.get(CHAVE_CABECALHO).ok().flatten()?;
        let claro = self.cofre.decifrar(&g)?;
        let (n, v) = claro.split_once(':')?;
        Some((n.trim().to_string(), v.trim().to_string()))
    }

    pub fn situacao(&self) -> Situacao {
        let i = self.interno.lock().unwrap();
        Situacao {
            configurado: self.url().is_some(),
            url: self.url(),
            cabecalho_nome: self.cabecalho().map(|(n, _)| n),
            enviados: i.enviados,
            falhas: i.falhas,
            ultimo_erro: i.ultimo_erro.clone(),
            na_fila: i.na_fila,
        }
    }

    /// Guarda URL e cabeçalho (`Nome: valor`, cifrado; vazio remove).
    pub fn configurar(&self, url: &str, cabecalho: &str) -> Result<(), String> {
        let u = url.trim();
        if !(u.starts_with("http://") || u.starts_with("https://")) {
            return Err("a URL do webhook precisa começar com http:// ou https://".into());
        }
        let c = cabecalho.trim();
        if !c.is_empty() {
            let Some((n, v)) = c.split_once(':') else {
                return Err(
                    "cabeçalho no formato Nome: valor (ex.: Authorization: Bearer abc)".into(),
                );
            };
            if n.trim().is_empty()
                || v.trim().is_empty()
                || !n
                    .trim()
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
            {
                return Err("nome do cabeçalho inválido".into());
            }
        }
        self.store.set(CHAVE_URL, u).map_err(|e| e.to_string())?;
        let guardado = if c.is_empty() {
            String::new()
        } else {
            self.cofre.cifrar(c)
        };
        self.store
            .set(CHAVE_CABECALHO, &guardado)
            .map_err(|e| e.to_string())?;
        self.interno.lock().unwrap().ultimo_erro = None;
        Ok(())
    }

    pub fn remover(&self) -> Result<(), String> {
        self.store.set(CHAVE_URL, "").map_err(|e| e.to_string())?;
        self.store
            .set(CHAVE_CABECALHO, "")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Entrada dos alertas (qualquer thread): antirrepique e fila; nunca bloqueia.
    pub fn notificar(&self, a: &Alerta, foto_url: Option<String>) {
        if self.url().is_none() {
            return;
        }
        if !self
            .antirrepique
            .lock()
            .unwrap()
            .permite(&a.chave, Instant::now())
        {
            return;
        }
        let ev = evento_de(&self.store, a, foto_url);
        if self.tx.try_send(ev).is_err() {
            self.interno.lock().unwrap().falhas += 1;
        }
    }

    /// Manda um evento `teste` agora (sem fila), devolvendo o status HTTP.
    pub async fn enviar_teste(&self) -> Result<u16, String> {
        let quando = agora_unix();
        let ev = Evento {
            programa: "CamFácil IA",
            versao: cf_core::VERSION,
            tipo: "teste".into(),
            camera_id: 0,
            camera: "teste".into(),
            regra_id: None,
            regra: None,
            quando,
            hora: hora_com_offset(quando, offset_local_s(&self.store)),
            texto: "CamFácil IA: teste de webhook. Tudo certo por aqui.".into(),
            foto_url: None,
        };
        self.postar(&ev).await
    }

    async fn postar(&self, ev: &Evento) -> Result<u16, String> {
        let url = self.url().ok_or("webhook não configurado")?;
        let mut req = self.http.post(&url).json(ev);
        if let Some((n, v)) = self.cabecalho() {
            req = req.header(n, v);
        }
        let r = req.send().await.map_err(|e| {
            if e.is_timeout() {
                "sem resposta do webhook (10 s)".to_string()
            } else if e.is_connect() {
                "sem conexão com o webhook".to_string()
            } else {
                format!("falha de rede: {e}")
            }
        })?;
        let status = r.status().as_u16();
        if (200..300).contains(&status) {
            Ok(status)
        } else {
            Err(format!("webhook respondeu {status}"))
        }
    }

    async fn laco(self: Arc<Self>, mut rx: mpsc::Receiver<Evento>) {
        while let Some(ev) = rx.recv().await {
            self.interno.lock().unwrap().na_fila = rx.len();
            let mut espera = Duration::from_secs(1);
            let mut ok = false;
            for tentativa in 1..=TENTATIVAS {
                match self.postar(&ev).await {
                    Ok(_) => {
                        ok = true;
                        break;
                    }
                    Err(e) => {
                        self.interno.lock().unwrap().ultimo_erro =
                            Some(format!("{e} (tentativa {tentativa}/{TENTATIVAS})"));
                        if tentativa < TENTATIVAS {
                            tokio::time::sleep(espera).await;
                            espera *= 3;
                        }
                    }
                }
            }
            let mut i = self.interno.lock().unwrap();
            if ok {
                i.enviados += 1;
                i.ultimo_erro = None;
            } else {
                i.falhas += 1;
            }
            i.na_fila = rx.len();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::whatsapp::tests::servidor;

    #[test]
    fn monta_evento_pela_chave() {
        let s = Store::memory().unwrap();
        let cam = s.criar_camera("Portão", "demo", "").unwrap();
        let r = s.criar_regra(cam, "Cerca", "linha", &[], "", "").unwrap();
        let a = Alerta {
            chave: format!("regra:{r}"),
            texto: "10:00 — Cerca: passagem".into(),
            foto: None,
        };
        let e = evento_de(&s, &a, Some("http://x/foto.jpg".into()));
        assert_eq!(e.tipo, "regra");
        assert_eq!(e.camera_id, cam);
        assert_eq!(e.camera, "Portão");
        assert_eq!(e.regra.as_deref(), Some("Cerca"));
        assert_eq!(e.foto_url.as_deref(), Some("http://x/foto.jpg"));
        let a = Alerta {
            chave: format!("saude:{cam}"),
            texto: "x".into(),
            foto: None,
        };
        let e = evento_de(&s, &a, None);
        assert_eq!(e.tipo, "saude");
        assert_eq!(e.camera, "Portão");
        assert!(e.regra.is_none());
    }

    #[tokio::test]
    async fn envia_com_cabecalho_e_repete_tres_vezes() {
        let dir = std::env::temp_dir().join(format!("cf-wh-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // 1 teste ok · 3 tentativas com 500 · 1 ok para o alerta seguinte.
        let (base, h) = servidor(vec![
            (200, "{}"),
            (500, "{}"),
            (500, "{}"),
            (500, "{}"),
            (204, ""),
        ]);
        let store = Arc::new(Store::memory().unwrap());
        let wh = Webhook::iniciar_com(store.clone(), Cofre::abrir_em(&dir).unwrap()).unwrap();
        assert!(wh.configurar("ftp://x", "").is_err());
        assert!(wh.configurar(&base, "sem dois pontos").is_err());
        wh.configurar(&format!("{base}/hook"), "Authorization: Bearer segredo-123")
            .unwrap();
        let g = store.get("wh_cabecalho").unwrap().unwrap();
        assert!(g.starts_with("enc:") && !g.contains("segredo-123"));
        assert_eq!(
            wh.situacao().cabecalho_nome.as_deref(),
            Some("Authorization")
        );
        assert_eq!(wh.enviar_teste().await.unwrap(), 200);
        let a = Alerta {
            chave: "regra:1".into(),
            texto: "t".into(),
            foto: None,
        };
        wh.notificar(&a, None);
        wh.notificar(&a, None); // antirrepique: descartado
                                // Espera as 3 tentativas (1 s + 3 s de espera).
        for _ in 0..60 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if wh.situacao().falhas == 1 {
                break;
            }
        }
        assert_eq!(wh.situacao().falhas, 1);
        assert!(wh.situacao().ultimo_erro.unwrap().contains("3/3"));
        let b = Alerta {
            chave: "saude:9".into(),
            texto: "s".into(),
            foto: None,
        };
        wh.notificar(&b, Some("http://cam/foto.jpg".into()));
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if wh.situacao().enviados == 1 {
                break;
            }
        }
        assert_eq!(wh.situacao().enviados, 1);
        let vistos = h.join().unwrap();
        assert_eq!(vistos.len(), 5);
        assert!(vistos[0].starts_with("POST /hook"));
        assert!(vistos[0].contains("authorization: Bearer segredo-123"));
        assert!(vistos[0].contains(r#""tipo":"teste""#));
        assert!(vistos[1].contains(r#""tipo":"regra""#));
        assert!(vistos[4].contains(r#""foto_url":"http://cam/foto.jpg""#));
        wh.remover().unwrap();
        assert!(!wh.situacao().configurado);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
