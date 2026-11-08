//! Ver uma câmera no stream **principal**, sob demanda.
//!
//! A IA continua analisando no stream extra (o leitor de sempre, em
//! `video.rs`). Quando alguém pede `?stream=principal` na rota MJPEG ou na
//! foto, abre-se um segundo ffmpeg só para mostrar a imagem (até 1280 px,
//! 10 quadros/s, sem análise). Ele se encerra sozinho quando ninguém assiste
//! há [`OCIOSO`]: nenhum MJPEG aberto e nenhuma foto pedida nesse tempo.
//!
//! Por que um leitor sob demanda e não trocar o leitor da IA: o principal é
//! pesado (1080p ou mais) e a análise não ganha nada com ele. Assim o custo
//! só existe enquanto alguém está olhando, e some sozinho.

use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// Sem ninguém assistindo por esse tempo, o leitor do principal fecha.
pub const OCIOSO: Duration = Duration::from_secs(30);

pub struct Visor {
    pub url: String,
    pub ultimo: Mutex<Option<Arc<Vec<u8>>>>,
    pub tx: broadcast::Sender<Arc<Vec<u8>>>,
    uso: Mutex<Instant>,
    parar: AtomicBool,
    filho: Mutex<Option<Child>>,
    pub erro: Mutex<Option<String>>,
}

impl Visor {
    fn novo(url: &str) -> Arc<Self> {
        let (tx, _) = broadcast::channel(4);
        Arc::new(Self {
            url: url.to_string(),
            ultimo: Mutex::new(None),
            tx,
            uso: Mutex::new(Instant::now()),
            parar: AtomicBool::new(false),
            filho: Mutex::new(None),
            erro: Mutex::new(None),
        })
    }

    /// Alguém olhou (foto pedida ou MJPEG aberto agora).
    pub fn tocar(&self) {
        *self.uso.lock().unwrap() = Instant::now();
    }

    fn ocioso(&self, agora: Instant) -> bool {
        self.tx.receiver_count() == 0
            && agora.saturating_duration_since(*self.uso.lock().unwrap()) >= OCIOSO
    }

    fn vivo(&self) -> bool {
        !self.parar.load(Ordering::Relaxed)
    }

    fn encerrar(&self) {
        self.parar.store(true, Ordering::Relaxed);
        if let Some(mut c) = self.filho.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

fn mapa() -> &'static Mutex<HashMap<i64, Arc<Visor>>> {
    static M: OnceLock<Mutex<HashMap<i64, Arc<Visor>>>> = OnceLock::new();
    M.get_or_init(Default::default)
}

/// Leitor do principal da câmera `id` (abre se ainda não há um vivo).
pub fn abrir(id: i64, url: &str) -> Arc<Visor> {
    let mut m = mapa().lock().unwrap();
    if let Some(v) = m.get(&id) {
        if v.vivo() && v.url == url {
            v.tocar();
            return v.clone();
        }
        v.encerrar();
    }
    let v = Visor::novo(url);
    m.insert(id, v.clone());
    drop(m);
    let w = v.clone();
    let _ = std::thread::Builder::new()
        .name(format!("principal-{id}"))
        .spawn(move || laco(id, w));
    v
}

/// Fecha o leitor do principal (câmera parada, editada ou removida).
pub fn fechar(id: i64) {
    if let Some(v) = mapa().lock().unwrap().remove(&id) {
        v.encerrar();
    }
}

fn argumentos(url: &str) -> Vec<String> {
    [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-rtsp_transport",
        "tcp",
        "-timeout",
        "5000000",
        "-i",
        url,
        "-an",
        "-vf",
        "fps=10,scale='min(1280,iw)':-2",
        "-f",
        "mjpeg",
        "-q:v",
        "5",
        "pipe:1",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn laco(id: i64, v: Arc<Visor>) {
    let mut espera = Duration::from_secs(2);
    while v.vivo() && !v.ocioso(Instant::now()) {
        let Some(ff) = crate::video::achar_ffmpeg() else {
            *v.erro.lock().unwrap() = Some("ffmpeg não encontrado".into());
            break;
        };
        let mut cmd = Command::new(ff);
        cmd.args(argumentos(&v.url))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let Ok(mut filho) = cmd.spawn() else {
            break;
        };
        let mut saida = filho.stdout.take().expect("stdout");
        *v.filho.lock().unwrap() = Some(filho);
        let recebeu = ler(&v, &mut saida);
        if let Some(mut c) = v.filho.lock().unwrap().take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        espera = if recebeu {
            Duration::from_secs(2)
        } else {
            (espera * 2).min(Duration::from_secs(20))
        };
        let fim = Instant::now() + espera;
        while v.vivo() && !v.ocioso(Instant::now()) && Instant::now() < fim {
            std::thread::sleep(Duration::from_millis(500));
        }
    }
    v.parar.store(true, Ordering::Relaxed);
    let mut m = mapa().lock().unwrap();
    if m.get(&id).is_some_and(|x| Arc::ptr_eq(x, &v)) {
        m.remove(&id);
    }
}

/// Lê quadros até o ffmpeg acabar ou ninguém mais assistir.
fn ler(v: &Visor, saida: &mut impl Read) -> bool {
    let mut buf: Vec<u8> = Vec::with_capacity(512 * 1024);
    let mut bloco = vec![0u8; 64 * 1024];
    let mut recebeu = false;
    loop {
        if !v.vivo() || v.ocioso(Instant::now()) {
            return recebeu;
        }
        let n = match saida.read(&mut bloco) {
            Ok(0) | Err(_) => return recebeu,
            Ok(n) => n,
        };
        buf.extend_from_slice(&bloco[..n]);
        while let Some((ini, fim)) = crate::video::proximo_jpeg(&buf) {
            let jpeg = Arc::new(buf[ini..fim].to_vec());
            buf.drain(..fim);
            recebeu = true;
            *v.erro.lock().unwrap() = None;
            *v.ultimo.lock().unwrap() = Some(jpeg.clone());
            let _ = v.tx.send(jpeg);
        }
        if buf.len() > 8 * 1024 * 1024 {
            buf.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fecha_so_quando_ninguem_assiste_ha_30_s() {
        let v = Visor::novo("rtsp://h/cam/realmonitor?channel=1&subtype=0");
        let agora = Instant::now();
        assert!(!v.ocioso(agora), "acabou de abrir");
        let rx = v.tx.subscribe();
        let depois = agora + OCIOSO + Duration::from_secs(5);
        assert!(!v.ocioso(depois), "MJPEG aberto segura o leitor");
        drop(rx);
        assert!(v.ocioso(depois), "sem MJPEG e sem foto há mais de 30 s");
        v.tocar();
        assert!(!v.ocioso(Instant::now()), "foto pedida renova");
    }

    #[test]
    fn argumentos_so_para_ver() {
        let a = argumentos("rtsp://h/x");
        assert!(a.iter().any(|x| x.contains("min(1280,iw)")));
        assert!(a.contains(&"pipe:1".to_string()));
    }

    #[test]
    fn fechar_tira_do_mapa() {
        let v = Visor::novo("rtsp://h/y");
        mapa().lock().unwrap().insert(-999, v.clone());
        fechar(-999);
        assert!(!v.vivo());
        assert!(!mapa().lock().unwrap().contains_key(&-999));
    }
}
