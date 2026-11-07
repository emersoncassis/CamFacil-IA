//! Ingestão de vídeo e análise por câmera (SDD-01, SDD-02, ADR-0003).
//!
//! Um processo ffmpeg por câmera entrega MJPEG leve (640 px, 5 quadros/s). Cada
//! quadro vira a imagem ao vivo da dashboard e, reduzido em cinza, alimenta o
//! analisador de visão. Uma thread por câmera, isolada do servidor web.

use cf_core::store::{RegraSalva, Store};
use cf_core::visao::{Analisador, Evento, Quadro, Regra};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// Quanto tempo o balão fica "em alerta" depois de um cruzamento de linha.
const ALERTA: Duration = Duration::from_secs(10);
const LARGURA_ANALISE: usize = 160;

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// ffmpeg
// ---------------------------------------------------------------------------

/// Procura o ffmpeg: variável `CAMFACIL_FFMPEG`, pasta de dados, depois o PATH.
pub fn achar_ffmpeg() -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if let Ok(p) = std::env::var("CAMFACIL_FFMPEG") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let local = cf_core::paths::data_dir().join("bin").join(exe);
    if local.is_file() {
        return Some(local);
    }
    // Ao lado do executável (pacote com ffmpeg junto).
    if let Some(p) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join(exe)))
        .filter(|p| p.is_file())
    {
        return Some(p);
    }
    let no_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(exe))
            .find(|p| p.is_file())
    });
    if no_path.is_some() {
        return no_path;
    }
    // Recém-instalado pelo winget: o PATH deste processo ainda não sabe dele.
    if cfg!(windows) {
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            let p = PathBuf::from(l).join(r"Microsoft\WinGet\Links").join(exe);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Instala o ffmpeg sozinho no Windows (winget). Em outros sistemas devolve
/// o comando para o técnico rodar, porque pede senha de administrador.
pub fn instalar_ffmpeg() -> Result<(), String> {
    if !cfg!(windows) {
        return Err(format!("Rode no terminal: {}", comando_instalar()));
    }
    let st = Command::new("winget")
        .args([
            "install",
            "--id",
            "Gyan.FFmpeg",
            "-e",
            "--silent",
            "--accept-source-agreements",
            "--accept-package-agreements",
        ])
        .status()
        .map_err(|_| {
            format!(
                "Não achei o winget neste Windows. Instale o ffmpeg pelo site ffmpeg.org ou rode: {}",
                comando_instalar()
            )
        })?;
    if achar_ffmpeg().is_some() {
        Ok(())
    } else if st.success() {
        Err("Instalado. Feche e abra o CamFácil IA de novo para ele achar o ffmpeg.".into())
    } else {
        Err(format!(
            "O winget não conseguiu instalar. Tente no terminal: {}",
            comando_instalar()
        ))
    }
}

pub fn versao_ffmpeg(p: &PathBuf) -> Option<String> {
    let out = Command::new(p).arg("-version").output().ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .map(|l| l.split(" Copyright").next().unwrap_or(l).to_string())
}

/// Comando sugerido para instalar o ffmpeg neste sistema.
pub fn comando_instalar() -> &'static str {
    if cfg!(windows) {
        "winget install --id Gyan.FFmpeg -e"
    } else if cfg!(target_os = "macos") {
        "brew install ffmpeg"
    } else {
        "sudo apt install -y ffmpeg   (ou: sudo dnf install ffmpeg)"
    }
}

/// Cena sintética de demonstração: uma "pessoa" atravessa a loja, passa pela
/// porta e para no balcão. Não precisa de câmera nem de vídeo baixado.
/// (`overlay` recalcula a posição a cada quadro; `drawbox` não.)
const CENA_DEMO: &str = concat!(
    "color=c=0x16181a:s=640x360:r=10,",
    "drawbox=x=0:y=300:w=640:h=60:color=0x24272a:t=fill,",
    "drawbox=x=250:y=70:w=110:h=230:color=0x3a3f44:t=6,",
    "drawbox=x=470:y=200:w=150:h=100:color=0x2c3135:t=fill[fundo];",
    "color=c=0xc8ccd0:s=36x126:r=10[corpo];",
    "color=c=0xc8ccd0:s=24x26:r=10[cabeca];",
    "[fundo][corpo]overlay=x='if(lt(mod(t,16),9),mod(t,16)*60-40,500)':y=174[a];",
    "[a][cabeca]overlay=x='if(lt(mod(t,16),9),mod(t,16)*60-40,500)+6':y=144[out0]"
);

fn argumentos(fonte: &str, url: &str) -> Vec<String> {
    let mut a: Vec<String> = ["-hide_banner", "-loglevel", "error", "-nostdin"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let entrada: Vec<String> = match fonte {
        "demo" => vec![
            "-re".into(),
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            CENA_DEMO.into(),
        ],
        "arquivo" => vec![
            "-re".into(),
            "-stream_loop".into(),
            "-1".into(),
            "-i".into(),
            url.into(),
        ],
        "webcam" if cfg!(windows) => vec![
            "-f".into(),
            "dshow".into(),
            "-i".into(),
            format!("video={url}"),
        ],
        "webcam" if cfg!(target_os = "macos") => {
            vec![
                "-f".into(),
                "avfoundation".into(),
                "-i".into(),
                if url.is_empty() {
                    "0".into()
                } else {
                    url.into()
                },
            ]
        }
        "webcam" => vec![
            "-f".into(),
            "v4l2".into(),
            "-i".into(),
            if url.is_empty() {
                "/dev/video0".into()
            } else {
                url.into()
            },
        ],
        _ => vec![
            "-rtsp_transport".into(),
            "tcp".into(),
            "-timeout".into(),
            "5000000".into(),
            "-i".into(),
            url.into(),
        ],
    };
    a.extend(entrada);
    a.extend(
        [
            "-an",
            "-vf",
            "fps=5,scale=640:-2",
            "-f",
            "mjpeg",
            "-q:v",
            "7",
            "pipe:1",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    a
}

/// Lista webcams (Windows: DirectShow; Linux: /dev/video*).
pub fn webcams() -> Vec<String> {
    if cfg!(windows) {
        let Some(ff) = achar_ffmpeg() else {
            return vec![];
        };
        let out = Command::new(ff)
            .args([
                "-hide_banner",
                "-list_devices",
                "true",
                "-f",
                "dshow",
                "-i",
                "dummy",
            ])
            .output();
        let Ok(out) = out else { return vec![] };
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .filter(|l| l.contains("(video)"))
            .filter_map(|l| l.split('"').nth(1).map(String::from))
            .collect()
    } else {
        let mut v: Vec<String> = std::fs::read_dir("/dev")
            .map(|d| {
                d.filter_map(|e| e.ok())
                    .map(|e| e.path().display().to_string())
                    .filter(|p| p.starts_with("/dev/video"))
                    .collect()
            })
            .unwrap_or_default();
        v.sort();
        v
    }
}

// ---------------------------------------------------------------------------
// Canal de uma câmera
// ---------------------------------------------------------------------------

#[derive(Default, Clone, Serialize)]
pub struct EstadoRegra {
    pub ocupada: bool,
    /// Em alerta (balão pisca) até este instante, em ms desde agora.
    pub alerta_ms: u64,
    pub contagem: u64,
}

#[derive(Default, Clone, Serialize)]
pub struct EstadoCamera {
    pub online: bool,
    pub fps: f32,
    pub erro: Option<String>,
    pub saude: Option<String>,
    pub movimento_ms: u64,
    pub regras: HashMap<i64, EstadoRegra>,
}

#[derive(Default)]
struct Interno {
    online: bool,
    fps: f32,
    erro: Option<String>,
    saude: Option<String>,
    movimento: Option<Instant>,
    ocupadas: HashMap<i64, bool>,
    alertas: HashMap<i64, Instant>,
    contagens: HashMap<i64, u64>,
}

/// "IA, sugira para mim": observação em andamento ou concluída.
pub struct Observacao {
    pub obs: cf_core::sugestao::Observador,
    pub inicio: Instant,
    pub duracao: Duration,
}

pub struct Canal {
    pub id: i64,
    pub observacao: Mutex<Option<Observacao>>,
    pub ultimo: Mutex<Option<Arc<Vec<u8>>>>,
    pub tx: broadcast::Sender<Arc<Vec<u8>>>,
    regras: RwLock<Vec<Regra>>,
    interno: Mutex<Interno>,
    parar: AtomicBool,
    filho: Mutex<Option<Child>>,
}

impl Canal {
    pub fn estado(&self) -> EstadoCamera {
        let i = self.interno.lock().unwrap();
        let resta = |t: Option<&Instant>, d: Duration| {
            t.map(|t| d.saturating_sub(t.elapsed()).as_millis() as u64)
                .unwrap_or(0)
        };
        let regras = self
            .regras
            .read()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r.id,
                    EstadoRegra {
                        ocupada: i.ocupadas.get(&r.id).copied().unwrap_or(false),
                        alerta_ms: resta(i.alertas.get(&r.id), ALERTA),
                        contagem: i.contagens.get(&r.id).copied().unwrap_or(0),
                    },
                )
            })
            .collect();
        EstadoCamera {
            online: i.online,
            fps: (i.fps * 10.0).round() / 10.0,
            erro: i.erro.clone(),
            saude: i.saude.clone(),
            movimento_ms: resta(i.movimento.as_ref(), Duration::from_secs(4)),
            regras,
        }
    }

    pub fn definir_regras(&self, regras: &[RegraSalva]) {
        *self.regras.write().unwrap() = regras.iter().filter_map(|r| r.para_visao()).collect();
    }

    fn encerrar(&self) {
        self.parar.store(true, Ordering::Relaxed);
        if let Some(mut c) = self.filho.lock().unwrap().take() {
            let _ = c.kill();
        }
    }
}

/// Módulos ligados (lidos das configurações a cada quadro, sem custo relevante).
#[derive(Clone, Copy)]
pub struct Modulos {
    pub movimento: bool,
    pub cerca: bool,
    pub saude: bool,
}

pub fn modulos(store: &Store) -> Modulos {
    let on = |k: &str| store.get(k).ok().flatten().as_deref() != Some("0");
    Modulos {
        movimento: on("modulo_movimento"),
        cerca: on("modulo_cerca"),
        saude: on("modulo_saude"),
    }
}

/// Selo da cerca virtual: uma zona e uma linha criadas e já disparadas.
fn verificar_selo_cerca(store: &Store) {
    let Ok(cams) = store.cameras() else { return };
    let regras: Vec<RegraSalva> = cams
        .iter()
        .flat_map(|c| store.regras(c.id).unwrap_or_default())
        .collect();
    let Ok(ev) = store.eventos(500) else { return };
    let disparou = |forma: &str, tipo: &str| {
        regras.iter().filter(|r| r.forma == forma).any(|r| {
            ev.iter()
                .any(|e| e.regra_id == Some(r.id) && e.tipo == tipo)
        })
    };
    if disparou("zona", "zona_ocupada") && disparou("linha", "linha_cruzada") {
        let _ = store.ganhar_selo("cerca", agora());
    }
}

pub struct Video {
    store: Arc<Store>,
    canais: Mutex<HashMap<i64, Arc<Canal>>>,
}

impl Video {
    pub fn new(store: Arc<Store>) -> Arc<Self> {
        Arc::new(Self {
            store,
            canais: Mutex::new(HashMap::new()),
        })
    }

    pub fn canal(&self, id: i64) -> Option<Arc<Canal>> {
        self.canais.lock().unwrap().get(&id).cloned()
    }

    /// Liga todas as câmeras ativas salvas.
    pub fn iniciar_todas(self: &Arc<Self>) {
        if let Ok(cams) = self.store.cameras() {
            for c in cams.into_iter().filter(|c| c.ativa) {
                self.iniciar(c.id, &c.fonte, &c.url);
            }
        }
    }

    pub fn parar(&self, id: i64) {
        if let Some(c) = self.canais.lock().unwrap().remove(&id) {
            c.encerrar();
        }
    }

    pub fn recarregar_regras(&self, id: i64) {
        if let (Some(c), Ok(r)) = (self.canal(id), self.store.regras(id)) {
            c.definir_regras(&r);
        }
    }

    pub fn iniciar(self: &Arc<Self>, id: i64, fonte: &str, url: &str) {
        self.parar(id);
        let (tx, _) = broadcast::channel(4);
        let canal = Arc::new(Canal {
            id,
            observacao: Mutex::new(None),
            ultimo: Mutex::new(None),
            tx,
            regras: RwLock::new(Vec::new()),
            interno: Mutex::new(Interno::default()),
            parar: AtomicBool::new(false),
            filho: Mutex::new(None),
        });
        if let Ok(r) = self.store.regras(id) {
            canal.definir_regras(&r);
        }
        self.canais.lock().unwrap().insert(id, canal.clone());
        let store = self.store.clone();
        let (fonte, url) = (fonte.to_string(), url.to_string());
        let _ = std::thread::Builder::new()
            .name(format!("camera-{id}"))
            .spawn(move || laco(canal, store, fonte, url));
    }
}

/// Mantém o ffmpeg rodando, com espera crescente entre tentativas.
fn laco(canal: Arc<Canal>, store: Arc<Store>, fonte: String, url: String) {
    let mut espera = Duration::from_secs(2);
    let mut analisador = Analisador::default();
    while !canal.parar.load(Ordering::Relaxed) {
        let Some(ff) = achar_ffmpeg() else {
            canal.interno.lock().unwrap().erro = Some(format!(
                "ffmpeg não encontrado. Instale com: {}",
                comando_instalar()
            ));
            std::thread::sleep(Duration::from_secs(5));
            continue;
        };
        let mut cmd = Command::new(ff);
        cmd.args(argumentos(&fonte, &url))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // sem janela de console
        }
        let mut filho = match cmd.spawn() {
            Ok(f) => f,
            Err(e) => {
                canal.interno.lock().unwrap().erro =
                    Some(format!("não foi possível abrir o ffmpeg: {e}"));
                std::thread::sleep(espera);
                continue;
            }
        };
        let mut saida = filho.stdout.take().expect("stdout");
        let mut erros = filho.stderr.take().expect("stderr");
        *canal.filho.lock().unwrap() = Some(filho);
        let leitor_erro = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = erros.read_to_string(&mut s);
            s
        });

        let recebeu = ler_quadros(&canal, &store, &mut saida, &mut analisador);
        let msg = leitor_erro.join().unwrap_or_default();
        {
            let mut i = canal.interno.lock().unwrap();
            i.online = false;
            i.fps = 0.0;
            if !canal.parar.load(Ordering::Relaxed) {
                i.erro = Some(traduzir_erro(&msg));
            }
        }
        if recebeu {
            espera = Duration::from_secs(2);
        } else {
            espera = (espera * 2).min(Duration::from_secs(30));
        }
        if !canal.parar.load(Ordering::Relaxed) {
            std::thread::sleep(espera);
        }
    }
}

fn traduzir_erro(stderr: &str) -> String {
    let s = stderr.to_lowercase();
    if s.contains("401") || s.contains("unauthorized") {
        "a câmera recusou usuário ou senha".into()
    } else if s.contains("connection refused") {
        "a câmera recusou a conexão (porta ou RTSP desativado)".into()
    } else if s.contains("timed out") || s.contains("timeout") {
        "a câmera não respondeu (IP errado ou fora da rede)".into()
    } else if s.contains("404") || s.contains("not found") {
        "caminho RTSP não encontrado (confira canal e fabricante)".into()
    } else if s.trim().is_empty() {
        "a transmissão foi interrompida".into()
    } else {
        stderr
            .lines()
            .last()
            .unwrap_or("erro no vídeo")
            .chars()
            .take(160)
            .collect()
    }
}

/// Lê o MJPEG, separa os quadros e analisa. Devolve se chegou algum quadro.
fn ler_quadros(canal: &Canal, store: &Store, saida: &mut impl Read, an: &mut Analisador) -> bool {
    let mut buf: Vec<u8> = Vec::with_capacity(256 * 1024);
    let mut bloco = vec![0u8; 64 * 1024];
    let mut recebeu = false;
    let mut janela = Instant::now();
    let mut na_janela = 0u32;
    loop {
        if canal.parar.load(Ordering::Relaxed) {
            return recebeu;
        }
        let n = match saida.read(&mut bloco) {
            Ok(0) | Err(_) => return recebeu,
            Ok(n) => n,
        };
        buf.extend_from_slice(&bloco[..n]);
        while let Some((ini, fim)) = proximo_jpeg(&buf) {
            let jpeg: Vec<u8> = buf[ini..fim].to_vec();
            buf.drain(..fim);
            recebeu = true;
            na_janela += 1;
            if janela.elapsed() >= Duration::from_secs(2) {
                let mut i = canal.interno.lock().unwrap();
                i.fps = na_janela as f32 / janela.elapsed().as_secs_f32();
                janela = Instant::now();
                na_janela = 0;
            }
            {
                let mut i = canal.interno.lock().unwrap();
                i.online = true;
                i.erro = None;
            }
            let jpeg = Arc::new(jpeg);
            *canal.ultimo.lock().unwrap() = Some(jpeg.clone());
            let _ = canal.tx.send(jpeg.clone());
            analisar(canal, store, &jpeg, an);
        }
        // Proteção: lixo sem marcadores não cresce para sempre.
        if buf.len() > 4 * 1024 * 1024 {
            buf.clear();
        }
    }
}

fn proximo_jpeg(b: &[u8]) -> Option<(usize, usize)> {
    let ini = b.windows(2).position(|w| w == [0xFF, 0xD8])?;
    let fim = b[ini + 2..].windows(2).position(|w| w == [0xFF, 0xD9])? + ini + 4;
    Some((ini, fim))
}

fn decodificar_cinza(jpeg: &[u8]) -> Option<Quadro> {
    use zune_core::bytestream::ZCursor;
    use zune_core::colorspace::ColorSpace;
    use zune_core::options::DecoderOptions;
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut d = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(jpeg), opts);
    let px = d.decode().ok()?;
    let (w, h) = d.dimensions()?;
    if px.len() != w * h {
        return None;
    }
    Some(Quadro::reduzir(w, h, &px, LARGURA_ANALISE))
}

fn analisar(canal: &Canal, store: &Store, jpeg: &[u8], an: &mut Analisador) {
    let Some(q) = decodificar_cinza(jpeg) else {
        return;
    };
    let m = modulos(store);
    let regras = if m.cerca {
        canal.regras.read().unwrap().clone()
    } else {
        Vec::new()
    };
    let eventos = an.processar(&q, &regras, m.movimento, m.saude);
    if let Some(o) = canal.observacao.lock().unwrap().as_mut() {
        if o.inicio.elapsed() < o.duracao {
            o.obs.alimentar(&an.centros());
        }
    }
    if eventos.is_empty() {
        return;
    }
    let t = agora();
    let mut selo = false;
    let mut i = canal.interno.lock().unwrap();
    for e in eventos {
        let (tipo, regra) = match &e {
            Evento::Movimento => {
                i.movimento = Some(Instant::now());
                ("movimento", None)
            }
            Evento::ZonaOcupada { regra } => {
                i.ocupadas.insert(*regra, true);
                i.alertas.insert(*regra, Instant::now());
                *i.contagens.entry(*regra).or_default() += 1;
                selo = true;
                ("zona_ocupada", Some(*regra))
            }
            Evento::ZonaLivre { regra } => {
                i.ocupadas.insert(*regra, false);
                ("zona_livre", Some(*regra))
            }
            Evento::LinhaCruzada { regra, .. } => {
                i.alertas.insert(*regra, Instant::now());
                *i.contagens.entry(*regra).or_default() += 1;
                selo = true;
                ("linha_cruzada", Some(*regra))
            }
            Evento::CameraEscura => {
                i.saude = Some("escura".into());
                ("camera_escura", None)
            }
            Evento::CameraTapada => {
                i.saude = Some("tapada".into());
                ("camera_tapada", None)
            }
            Evento::CameraDesfocada => {
                i.saude = Some("desfocada".into());
                ("camera_desfocada", None)
            }
            Evento::CameraNormal => {
                i.saude = None;
                ("camera_normal", None)
            }
        };
        let detalhe = match &e {
            Evento::LinhaCruzada { sentido, .. } => {
                if *sentido > 0 { "entrada" } else { "saída" }.to_string()
            }
            _ => String::new(),
        };
        let _ = store.registrar_evento(canal.id, regra, tipo, &detalhe, t);
    }
    drop(i);
    if selo {
        verificar_selo_cerca(store);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separa_quadros_jpeg() {
        let mut b = vec![1, 2, 0xFF, 0xD8, 9, 9, 0xFF, 0xD9, 0xFF, 0xD8, 7];
        let (i, f) = proximo_jpeg(&b).unwrap();
        assert_eq!(&b[i..f], &[0xFF, 0xD8, 9, 9, 0xFF, 0xD9]);
        b.drain(..f);
        assert!(
            proximo_jpeg(&b).is_none(),
            "quadro incompleto espera mais dados"
        );
    }

    #[test]
    fn argumentos_por_fonte() {
        let a = argumentos("rtsp", "rtsp://x");
        assert!(a.windows(2).any(|w| w == ["-rtsp_transport", "tcp"]));
        assert!(a.ends_with(&["pipe:1".to_string()]));
        assert!(argumentos("demo", "").iter().any(|s| s == "lavfi"));
        assert!(argumentos("arquivo", "/v.mp4")
            .iter()
            .any(|s| s == "-stream_loop"));
    }

    #[test]
    fn erros_em_portugues() {
        assert!(traduzir_erro("method DESCRIBE failed: 401 Unauthorized").contains("senha"));
        assert!(traduzir_erro("Connection refused").contains("recusou"));
    }
}
