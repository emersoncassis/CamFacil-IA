//! Alertas que a própria câmera ou gravador manda (movimento, cruzou a linha…).
//!
//! Só escuta com o módulo "Alertas da câmera" ligado (desligado por padrão).
//! Uma conexão por aparelho (host), com login Digest/Basic, para Intelbras/
//! Dahua e Hikvision (`cf_core::eventos_camera`). Cada alerta vira um evento
//! `camera` no histórico ("recebido da câmera") e vai para o Telegram como os
//! demais. Desligar o módulo fecha as conexões.

use crate::App;
use cf_core::eventos_camera::{self as ev, EventoCamera};
use cf_core::{digest, rede, rtsp};
use std::collections::HashMap;
use std::sync::Weak;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Mesmo alerta da mesma câmera dentro deste prazo conta uma vez só.
const ENTRE_ALERTAS: Duration = Duration::from_secs(10);
/// Sem receber nada (nem batimento) por este tempo, reconecta.
const SILENCIO: Duration = Duration::from_secs(60);

#[derive(Clone, PartialEq)]
struct Alvo {
    fabricante: &'static str,
    usuario: String,
    senha: String,
}

pub fn iniciar(app: Weak<App>) {
    let _ = std::thread::Builder::new()
        .name("alertas-camera".into())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(supervisor(app));
        });
}

/// Aparelhos (host) a escutar, vistos nas câmeras RTSP de fabricante conhecido.
fn alvos(app: &App) -> HashMap<String, Alvo> {
    let mut m = HashMap::new();
    for c in app.store.cameras().unwrap_or_default() {
        if c.fonte != "rtsp" || !c.ativa {
            continue;
        }
        let (Some(fab), Some((host, _))) = (
            rtsp::fabricante_do_caminho(&c.url),
            rede::host_da_url(&c.url),
        ) else {
            continue;
        };
        let (usuario, senha) = rtsp::credenciais(&c.url);
        m.entry(host).or_insert(Alvo {
            fabricante: fab,
            usuario,
            senha,
        });
    }
    m
}

async fn supervisor(app: Weak<App>) {
    let mut tarefas: HashMap<String, (Alvo, tokio::task::JoinHandle<()>)> = HashMap::new();
    loop {
        let Some(a) = app.upgrade() else { return };
        let ligado = crate::video::modulos(&a.store).camera;
        let querem = if ligado { alvos(&a) } else { HashMap::new() };
        drop(a);
        // Fecha o que saiu ou mudou de login.
        tarefas.retain(|host, (alvo, h)| {
            let fica = querem.get(host) == Some(alvo);
            if !fica {
                h.abort();
            }
            fica
        });
        for (host, alvo) in querem {
            if let std::collections::hash_map::Entry::Vacant(v) = tarefas.entry(host.clone()) {
                let h = tokio::spawn(escutar(app.clone(), host, alvo.clone()));
                v.insert((alvo, h));
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn escutar(app: Weak<App>, host: String, alvo: Alvo) {
    let mut ultimos: HashMap<(i64, String), Instant> = HashMap::new();
    loop {
        let _ = sessao(&app, &host, &alvo, &mut ultimos).await;
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

fn pedido(host: &str, caminho: &str, auth: Option<&str>) -> String {
    let mut r = format!(
        "GET {caminho} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: CamFacilIA\r\nAccept: */*\r\nConnection: keep-alive\r\n"
    );
    if let Some(a) = auth {
        r.push_str(&format!("Authorization: {a}\r\n"));
    }
    r.push_str("\r\n");
    r
}

async fn sessao(
    app: &Weak<App>,
    host: &str,
    alvo: &Alvo,
    ultimos: &mut HashMap<(i64, String), Instant>,
) -> Option<()> {
    let caminho = ev::caminho(alvo.fabricante)?;
    let xml = alvo.fabricante == "hikvision";
    let mut auth: Option<String> = None;
    for _ in 0..2 {
        let mut s = tokio::time::timeout(Duration::from_secs(8), TcpStream::connect((host, 80)))
            .await
            .ok()?
            .ok()?;
        s.write_all(pedido(host, caminho, auth.as_deref()).as_bytes())
            .await
            .ok()?;
        // Cabeçalho da resposta.
        let mut buf: Vec<u8> = Vec::new();
        let mut parte = [0u8; 4096];
        let fim_cab = loop {
            if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break i;
            }
            let n = tokio::time::timeout(Duration::from_secs(10), s.read(&mut parte))
                .await
                .ok()?
                .ok()?;
            if n == 0 || buf.len() > 16 * 1024 {
                return None;
            }
            buf.extend_from_slice(&parte[..n]);
        };
        let cab = String::from_utf8_lossy(&buf[..fim_cab]).to_string();
        let codigo: u16 = cab.split_whitespace().nth(1)?.parse().ok()?;
        if codigo == 401 && auth.is_none() {
            let desafios = digest::desafios_dos_cabecalhos(&cab);
            auth = Some(digest::autorizacao_para(
                &desafios,
                "GET",
                caminho,
                &alvo.usuario,
                &alvo.senha,
            )?);
            continue;
        }
        if !(200..300).contains(&codigo) {
            return None;
        }
        // Fluxo de alertas.
        let mut texto = String::from_utf8_lossy(&buf[fim_cab + 4..]).to_string();
        loop {
            for bloco in ev::cortar_blocos(&mut texto, xml) {
                let e = if xml {
                    ev::ler_xml_hikvision(&bloco)
                } else {
                    ev::ler_linha_dahua(&bloco)
                };
                if let Some(e) = e {
                    registrar(app, host, &e, ultimos);
                }
            }
            let n = tokio::time::timeout(SILENCIO, s.read(&mut parte))
                .await
                .ok()?
                .ok()?;
            if n == 0 {
                return None;
            }
            texto.push_str(&String::from_utf8_lossy(&parte[..n]));
        }
    }
    None
}

/// Guarda o alerta como "recebido da câmera" nas câmeras que ele diz respeito.
fn registrar(
    app: &Weak<App>,
    host: &str,
    e: &EventoCamera,
    ultimos: &mut HashMap<(i64, String), Instant>,
) {
    let Some(app) = app.upgrade() else { return };
    // Quem se importa com este alerta: o canal bate, ou é câmera avulsa.
    let cams: Vec<_> = app
        .store
        .cameras()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.fonte == "rtsp" && rede::host_da_url(&c.url).is_some_and(|(h, _)| h == host))
        .collect();
    let avulsa = cams.len() == 1 && cams[0].gravador.is_empty() && cams[0].canal == 0;
    let hora = cf_notify::servico::hora_local(&app.store);
    for c in cams {
        let canal = rtsp::canal_da_url(&c.url)
            .map(|x| x.0)
            .unwrap_or(c.canal.max(1));
        if !avulsa && canal != e.canal {
            continue;
        }
        let chave = (c.id, e.codigo.clone());
        if ultimos
            .get(&chave)
            .is_some_and(|t| t.elapsed() < ENTRE_ALERTAS)
        {
            continue;
        }
        ultimos.insert(chave, Instant::now());
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let detalhe = format!("{} (recebido da câmera)", e.texto());
        let _ = app
            .store
            .registrar_evento(c.id, None, "camera", &detalhe, t);
        let foto = app
            .video
            .canal(c.id)
            .and_then(|k| k.ultimo.lock().unwrap().clone());
        crate::video::notificar(
            format!("camera:{}:{}", c.id, e.codigo),
            format!(
                "{hora} — {}: {} (alerta da própria câmera)",
                c.nome,
                e.texto()
            ),
            foto,
        );
    }
}
