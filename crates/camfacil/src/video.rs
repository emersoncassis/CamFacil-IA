//! Ingestão de vídeo e análise por câmera (SDD-01, SDD-02, ADR-0003).
//!
//! Um processo ffmpeg por câmera entrega MJPEG leve (640 px, 5 quadros/s). Cada
//! quadro vira a imagem ao vivo da dashboard e, reduzido em cinza, alimenta o
//! analisador de visão. Uma thread por câmera, isolada do servidor web.

use cf_core::deteccao::{self, Deteccao, EventoMedio, Rastreador};
use cf_core::saude::{self, Situacao};
use cf_core::store::{RegraSalva, Store};
use cf_core::visao::{Analisador, Evento, Quadro, Regra};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
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
    if cfg!(target_os = "android") {
        return Err(comando_instalar().to_string());
    }
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
    } else if cfg!(target_os = "android") {
        "o ffmpeg vem dentro do app; reinstale o CamFácil IA pelo APK oficial"
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
    /// Complemento do balão enquanto em alerta (ex.: "parado 65 s").
    pub aviso: Option<String>,
}

#[derive(Default, Clone, Serialize)]
pub struct EstadoCamera {
    pub online: bool,
    pub fps: f32,
    pub erro: Option<String>,
    /// Saúde da imagem: `ok`, `escura`, `coberta`, `desfocada` ou `sem_sinal`.
    pub saude: String,
    /// Há quantos segundos está nessa situação.
    pub saude_desde_s: u64,
    /// Segundos desde o começo do último problema (`None` = nunca).
    pub ultimo_problema_s: Option<u64>,
    pub movimento_ms: u64,
    pub regras: HashMap<i64, EstadoRegra>,
    /// Nível médio: o que o detector viu por último ("2 pessoas · 1 carro").
    pub objetos: Vec<(String, usize)>,
    /// Há quantos ms o detector olhou esta câmera (None = nunca).
    pub ia_ms: Option<u64>,
}

/// Estado do nível médio (detector) de uma câmera.
#[derive(Default)]
pub struct Medio {
    pub rastreador: Rastreador,
    pub deteccoes: Vec<Deteccao>,
    pub quando: Option<Instant>,
    /// Nomes das classes do modelo de estudo que gerou `deteccoes` (None = COCO).
    pub estudo: Option<Arc<Vec<String>>>,
}

/// Nome da classe para o balão: COCO em pt-BR ou o nome da receita de estudo.
pub fn nome_objeto(classe: usize, estudo: Option<&Vec<String>>) -> String {
    if classe >= deteccao::CLASSE_ESTUDO {
        return estudo
            .and_then(|n| n.get(classe - deteccao::CLASSE_ESTUDO))
            .cloned()
            .unwrap_or_else(|| "objeto".into());
    }
    deteccao::nome_classe(classe).unwrap_or("objeto").into()
}

/// "2 pessoas" (COCO) ou "2 × caixa de banana" (estudo: o nome vem da receita).
fn rotulo_contagem(classe: usize, n: usize, estudo: Option<&Vec<String>>) -> String {
    if classe >= deteccao::CLASSE_ESTUDO {
        format!("{n} × {}", nome_objeto(classe, estudo))
    } else {
        deteccao::plural(classe, n)
    }
}

#[derive(Default)]
struct Interno {
    online: bool,
    fps: f32,
    erro: Option<String>,
    saude: saude::Monitor,
    quadros: u64,
    movimento: Option<Instant>,
    ocupadas: HashMap<i64, bool>,
    alertas: HashMap<i64, Instant>,
    contagens: HashMap<i64, u64>,
    avisos: HashMap<i64, String>,
}

/// "IA, sugira para mim": observação em andamento ou concluída.
pub struct Observacao {
    pub obs: cf_core::sugestao::Observador,
    /// "Avaliar a regra criada": só existe quando o técnico escolheu uma regra.
    pub aval: Option<cf_core::avaliacao::Avaliador>,
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
    pub medio: Mutex<Medio>,
    parar: AtomicBool,
    filho: Mutex<Option<Child>>,
    /// Canal do teste de estresse: analisa igual, mas não grava evento, não
    /// avisa ninguém e não aparece na TV, no painel nem no detector.
    pub ensaio: bool,
}

impl Canal {
    pub fn regras(&self) -> Vec<Regra> {
        self.regras.read().unwrap().clone()
    }

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
                        aviso: if resta(i.alertas.get(&r.id), ALERTA) > 0 {
                            i.avisos.get(&r.id).cloned()
                        } else {
                            None
                        },
                    },
                )
            })
            .collect();
        let agora = Instant::now();
        let (objetos, ia_ms) = {
            let m = self.medio.lock().unwrap();
            (
                deteccao::contar(&m.deteccoes)
                    .into_iter()
                    .map(|(c, n)| (rotulo_contagem(c, n, m.estudo.as_deref()), n))
                    .collect(),
                m.quando.map(|q| q.elapsed().as_millis() as u64),
            )
        };
        EstadoCamera {
            online: i.online,
            fps: (i.fps * 10.0).round() / 10.0,
            erro: i.erro.clone(),
            saude: i.saude.atual().chave().to_string(),
            saude_desde_s: i.saude.ha_quanto(agora).as_secs(),
            ultimo_problema_s: i.saude.desde_ultimo_problema(agora).map(|d| d.as_secs()),
            movimento_ms: resta(i.movimento.as_ref(), Duration::from_secs(4)),
            regras,
            objetos,
            ia_ms,
        }
    }

    /// Aplica os eventos do detector (nível médio) ao estado dos balões,
    /// grava no banco e avisa o Telegram. Chamado pela thread `ia-detector`.
    pub fn aplicar_medio(&self, store: &Store, eventos: Vec<EventoMedio>) {
        if eventos.is_empty() {
            return;
        }
        let regras = self.regras();
        let nome = |r: i64| {
            regras
                .iter()
                .find(|x| x.id == r)
                .map(|x| x.nome.clone())
                .unwrap_or_else(|| "regra".into())
        };
        let t = agora();
        let hora = cf_notify::servico::hora_local(store);
        let cam = nome_camera(store, self.id);
        let foto = self.ultimo.lock().unwrap().clone();
        let estudo = self.medio.lock().unwrap().estudo.clone();
        let mut avisos: Vec<(String, String)> = Vec::new();
        let mut selo = false;
        {
            let mut i = self.interno.lock().unwrap();
            for e in &eventos {
                match e {
                    EventoMedio::LinhaCruzada {
                        regra,
                        sentido,
                        classe,
                    } => {
                        i.alertas.insert(*regra, Instant::now());
                        *i.contagens.entry(*regra).or_default() += 1;
                        let que = nome_objeto(*classe, estudo.as_deref());
                        // O balão diz o que passou (pessoa, carro…).
                        i.avisos.insert(*regra, que.clone());
                        let det =
                            format!("{que} — {}", if *sentido > 0 { "entrada" } else { "saída" });
                        let _ =
                            store.registrar_evento(self.id, Some(*regra), "linha_cruzada", &det, t);
                        avisos.push((
                            format!("regra:{regra}"),
                            format!("{hora} — {}: passou {det} · {cam}", nome(*regra)),
                        ));
                        selo = true;
                    }
                    EventoMedio::ZonaOcupada {
                        regra,
                        quantas,
                        classe,
                    } => {
                        i.ocupadas.insert(*regra, true);
                        i.alertas.insert(*regra, Instant::now());
                        *i.contagens.entry(*regra).or_default() += 1;
                        let det = rotulo_contagem(*classe, *quantas, estudo.as_deref());
                        i.avisos.insert(*regra, det.clone());
                        let _ =
                            store.registrar_evento(self.id, Some(*regra), "zona_ocupada", &det, t);
                        avisos.push((
                            format!("regra:{regra}"),
                            format!("{hora} — {}: {det} · {cam}", nome(*regra)),
                        ));
                        selo = true;
                    }
                    EventoMedio::ZonaLivre { regra } => {
                        i.ocupadas.insert(*regra, false);
                        let _ = store.registrar_evento(self.id, Some(*regra), "zona_livre", "", t);
                    }
                    EventoMedio::Permanencia {
                        regra,
                        segundos,
                        classe,
                    } => {
                        let que = nome_objeto(*classe, estudo.as_deref());
                        i.alertas.insert(*regra, Instant::now());
                        i.avisos.insert(*regra, format!("parado há {segundos} s"));
                        let det = format!("{segundos} s");
                        let _ =
                            store.registrar_evento(self.id, Some(*regra), "permanencia", &det, t);
                        avisos.push((
                            format!("permanencia:{regra}"),
                            format!(
                                "{hora} — {}: {que} parado há {segundos} s · {cam}",
                                nome(*regra)
                            ),
                        ));
                    }
                    EventoMedio::Densidade {
                        regra,
                        quantas,
                        classe,
                    } => {
                        i.ocupadas.insert(*regra, true);
                        i.alertas.insert(*regra, Instant::now());
                        *i.contagens.entry(*regra).or_default() += 1;
                        let det = rotulo_contagem(*classe, *quantas, estudo.as_deref());
                        i.avisos.insert(*regra, det.clone());
                        let _ = store.registrar_evento(self.id, Some(*regra), "densidade", &det, t);
                        avisos.push((
                            format!("densidade:{regra}"),
                            format!("{hora} — {}: {det} ao mesmo tempo · {cam}", nome(*regra)),
                        ));
                    }
                }
            }
        }
        if selo {
            verificar_selo_cerca(store);
        }
        for (chave, texto) in avisos {
            notificar(chave, texto, foto.clone());
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

// ---------------------------------------------------------------------------
// Notificações (Telegram): gancho global, definido uma vez pelo binário
// ---------------------------------------------------------------------------

type Notificador = Arc<dyn Fn(cf_notify::Alerta) + Send + Sync>;
static NOTIFICADOR: OnceLock<Notificador> = OnceLock::new();

/// Liga o envio de alertas (uma vez, na inicialização).
pub fn definir_notificador(n: Notificador) {
    let _ = NOTIFICADOR.set(n);
}

pub(crate) fn notificar(chave: String, texto: String, foto: Option<Arc<Vec<u8>>>) {
    if let Some(n) = NOTIFICADOR.get() {
        n(cf_notify::Alerta { chave, texto, foto });
    }
}

/// Módulos ligados (lidos das configurações a cada quadro, sem custo relevante).
#[derive(Clone, Copy)]
pub struct Modulos {
    pub movimento: bool,
    pub cerca: bool,
    pub saude: bool,
    /// Alertas que a própria câmera manda (movimento, linha…).
    pub camera: bool,
    pub pessoas: bool,
    pub contagem: bool,
    pub permanencia: bool,
    pub calor: bool,
}

impl Modulos {
    pub fn ligado(&self, id: &str) -> bool {
        match id {
            "movimento" => self.movimento,
            "cerca" => self.cerca,
            "saude" => self.saude,
            "camera" => self.camera,
            "pessoas" => self.pessoas,
            "contagem" => self.contagem,
            "permanencia" => self.permanencia,
            "calor" => self.calor,
            _ => false,
        }
    }

    /// Algum módulo que precisa do detector (nível médio) está ligado?
    pub fn qualquer_medio(&self) -> bool {
        self.pessoas || self.contagem || self.permanencia || self.calor
    }
}

/// Catálogo: (id, nome falado, apelidos aceitos nos comandos, nível).
pub const MODULOS: &[(&str, &str, &[&str], &str)] = &[
    ("movimento", "Movimento inteligente", &["movimento"], "leve"),
    (
        "cerca",
        "Cerca e linha virtual",
        &["cerca", "linha", "zona"],
        "leve",
    ),
    ("saude", "Saúde das câmeras", &["saude", "saúde"], "leve"),
    (
        "camera",
        "Alertas da câmera",
        &["camera", "câmera", "alertas da camera"],
        "leve",
    ),
    (
        "pessoas",
        "Pessoas e veículos",
        &["pessoas", "pessoa", "veiculos", "veículos"],
        "medio",
    ),
    ("contagem", "Contagem", &["contagem", "contar"], "medio"),
    (
        "permanencia",
        "Permanência",
        &["permanencia", "permanência"],
        "medio",
    ),
    ("calor", "Mapa de calor", &["calor", "mapa"], "medio"),
];

/// Nível que o teste da máquina liberou (0 leve, 1 médio, 2 pesado).
fn nivel_liberado(store: &Store) -> Option<u8> {
    match store.get("bench_level").ok().flatten()?.as_str() {
        "leve" => Some(0),
        "medio" => Some(1),
        "pesado" => Some(2),
        _ => None,
    }
}

/// Liga/desliga um módulo pelo id ou apelido. Devolve (id, nome).
pub fn definir_modulo(
    store: &Store,
    nome: &str,
    ligado: bool,
) -> Result<(&'static str, &'static str), String> {
    let n = nome.trim().to_lowercase();
    let (id, bonito, _, nivel) = MODULOS
        .iter()
        .find(|(id, _, apelidos, _)| *id == n || apelidos.contains(&n.as_str()))
        .ok_or_else(|| {
            format!(
                "módulo desconhecido: {nome}. Tenho: {}",
                MODULOS
                    .iter()
                    .map(|(id, _, _, _)| *id)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    if ligado && *nivel == "medio" {
        match nivel_liberado(store) {
            None => {
                return Err(
                    "faça o teste da máquina (tela Início) antes de ligar um módulo médio".into(),
                )
            }
            Some(0) => {
                return Err(format!(
                    "esta máquina não liberou o nível médio; {bonito} precisa de um computador mais forte ou de câmera com IA de borda"
                ))
            }
            _ => {}
        }
    }
    store
        .set(&format!("modulo_{id}"), if ligado { "1" } else { "0" })
        .map_err(|e| e.to_string())?;
    Ok((id, bonito))
}

pub fn modulos(store: &Store) -> Modulos {
    // Nada vem ligado: quem escolhe os alertas é o técnico (os médios ainda
    // precisam do download do modelo).
    let on = |k: &str| store.get(k).ok().flatten().as_deref() == Some("1");
    let on_medio = on;
    Modulos {
        movimento: on("modulo_movimento"),
        cerca: on("modulo_cerca"),
        saude: on("modulo_saude"),
        camera: on("modulo_camera"),
        pessoas: on_medio("modulo_pessoas"),
        contagem: on_medio("modulo_contagem"),
        permanencia: on_medio("modulo_permanencia"),
        calor: on_medio("modulo_calor"),
    }
}

pub(crate) fn nome_camera(store: &Store, id: i64) -> String {
    store
        .camera(id)
        .ok()
        .flatten()
        .map(|c| c.nome)
        .unwrap_or_else(|| format!("Câmera {id}"))
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
    ensaios: Mutex<Vec<Arc<Canal>>>,
}

impl Video {
    pub fn new(store: Arc<Store>) -> Arc<Self> {
        Arc::new(Self {
            store,
            canais: Mutex::new(HashMap::new()),
            ensaios: Mutex::new(Vec::new()),
        })
    }

    pub fn canal(&self, id: i64) -> Option<Arc<Canal>> {
        self.canais.lock().unwrap().get(&id).cloned()
    }

    /// Todos os canais ativos, em ordem de id.
    pub fn canais(&self) -> Vec<Arc<Canal>> {
        let mut v: Vec<Arc<Canal>> = self.canais.lock().unwrap().values().cloned().collect();
        v.sort_by_key(|c| c.id);
        v
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
        // O leitor do principal (só para ver) vai junto: o endereço pode mudar.
        crate::visor::fechar(id);
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
        let canal = novo_canal(id, false);
        if let Ok(r) = self.store.regras(id) {
            canal.definir_regras(&r);
        }
        self.canais.lock().unwrap().insert(id, canal.clone());
        self.disparar(canal, fonte, url);
    }

    fn disparar(&self, canal: Arc<Canal>, fonte: &str, url: &str) {
        let store = self.store.clone();
        let (fonte, url) = (fonte.to_string(), url.to_string());
        let _ = std::thread::Builder::new()
            .name(format!("camera-{}", canal.id))
            .spawn(move || laco(canal, store, fonte, url));
    }

    /// Teste de estresse: mais uma câmera de demonstração com as regras dadas,
    /// fora da lista normal. Devolve o canal para medir os quadros/s.
    pub fn iniciar_ensaio(&self, regras: Vec<Regra>) -> Arc<Canal> {
        let mut lista = self.ensaios.lock().unwrap();
        let canal = novo_canal(-1 - lista.len() as i64, true);
        *canal.regras.write().unwrap() = regras;
        lista.push(canal.clone());
        drop(lista);
        self.disparar(canal.clone(), "demo", "");
        canal
    }

    pub fn parar_ensaios(&self) {
        for c in self.ensaios.lock().unwrap().drain(..) {
            c.encerrar();
        }
    }
}

fn novo_canal(id: i64, ensaio: bool) -> Arc<Canal> {
    let (tx, _) = broadcast::channel(4);
    Arc::new(Canal {
        id,
        observacao: Mutex::new(None),
        ultimo: Mutex::new(None),
        tx,
        regras: RwLock::new(Vec::new()),
        interno: Mutex::new(Interno::default()),
        medio: Mutex::new(Medio::default()),
        parar: AtomicBool::new(false),
        filho: Mutex::new(None),
        ensaio,
    })
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
            esperar_sem_sinal(&canal, &store, Duration::from_secs(5));
            continue;
        };
        let mut cmd = Command::new(ff);
        crate::filhos::morrer_com_o_programa(&mut cmd);
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
                esperar_sem_sinal(&canal, &store, espera);
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
            esperar_sem_sinal(&canal, &store, espera);
        }
    }
}

/// Espera entre tentativas avisando o monitor de saúde, a cada segundo, que a
/// câmera está sem sinal (a histerese decide quando isso vira evento).
fn esperar_sem_sinal(canal: &Canal, store: &Store, total: Duration) {
    let fim = Instant::now() + total;
    while !canal.parar.load(Ordering::Relaxed) {
        observar_saude(canal, store, Situacao::SemSinal);
        let resta = fim.saturating_duration_since(Instant::now());
        if resta.is_zero() {
            break;
        }
        std::thread::sleep(resta.min(Duration::from_secs(1)));
    }
}

/// Alimenta o monitor de saúde; grava o evento `saude` só na mudança confirmada.
fn observar_saude(canal: &Canal, store: &Store, s: Situacao) {
    if canal.ensaio || !modulos(store).saude {
        return;
    }
    let mudanca = canal
        .interno
        .lock()
        .unwrap()
        .saude
        .observar(s, Instant::now());
    if let Some(m) = mudanca {
        let _ = store.registrar_evento(canal.id, None, "saude", m.para.chave(), agora());
        if m.para.problema() {
            let texto = format!(
                "{} — {}: {}",
                cf_notify::servico::hora_local(store),
                nome_camera(store, canal.id),
                crate::notificacoes::saude_pt(m.para.chave())
            );
            let foto = if m.para == Situacao::SemSinal {
                None
            } else {
                canal.ultimo.lock().unwrap().clone()
            };
            notificar(format!("saude:{}", canal.id), texto, foto);
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

pub(crate) fn proximo_jpeg(b: &[u8]) -> Option<(usize, usize)> {
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
    // Com o detector cuidando de zonas (pessoas) ou linhas (contagem), o
    // analisador por movimento deixa essas regras para ele (sem contar duas vezes).
    let detector_ativo = canal.medio.lock().unwrap().quando.is_some();
    let regras: Vec<Regra> = if m.cerca || canal.ensaio {
        canal
            .regras
            .read()
            .unwrap()
            .iter()
            .filter(|r| {
                !detector_ativo
                    || match r.forma {
                        cf_core::visao::Forma::Zona => !m.pessoas,
                        cf_core::visao::Forma::Linha => !m.contagem,
                    }
            })
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    // Saúde: uma avaliação por segundo basta (5 quadros/s) e custa quase nada.
    let n = {
        let mut i = canal.interno.lock().unwrap();
        i.quadros += 1;
        i.quadros
    };
    if n.is_multiple_of(5) {
        observar_saude(canal, store, saude::classificar(&q));
    }
    let eventos = an.processar(&q, &regras, m.movimento || canal.ensaio);
    if canal.ensaio {
        return;
    }
    if let Some(o) = canal.observacao.lock().unwrap().as_mut() {
        // Com o detector olhando, ele alimenta (sabe a classe); sem ele, o movimento.
        let detector_vivo = canal
            .medio
            .lock()
            .unwrap()
            .quando
            .is_some_and(|q| q.elapsed() < Duration::from_secs(3));
        if o.inicio.elapsed() < o.duracao && !detector_vivo {
            let centros = an.centros();
            o.obs.alimentar(&centros);
            if let Some(av) = o.aval.as_mut() {
                let v: Vec<(u32, f32, f32, usize)> =
                    centros.iter().map(|&(i, x, y)| (i, x, y, 0)).collect();
                av.alimentar(&v, 1);
            }
        }
    }
    if eventos.is_empty() {
        return;
    }
    let t = agora();
    let mut selo = false;
    let mut avisos: Vec<(i64, String)> = Vec::new();
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
        };
        let detalhe = match &e {
            Evento::LinhaCruzada { sentido, .. } => {
                if *sentido > 0 { "entrada" } else { "saída" }.to_string()
            }
            _ => String::new(),
        };
        let _ = store.registrar_evento(canal.id, regra, tipo, &detalhe, t);
        if let (Some(r), true) = (regra, matches!(tipo, "zona_ocupada" | "linha_cruzada")) {
            avisos.push((r, detalhe));
        }
    }
    drop(i);
    if selo {
        verificar_selo_cerca(store);
    }
    if !avisos.is_empty() {
        let hora = cf_notify::servico::hora_local(store);
        let cam = nome_camera(store, canal.id);
        let foto = canal.ultimo.lock().unwrap().clone();
        for (r, detalhe) in avisos {
            let nome = regras
                .iter()
                .find(|x| x.id == r)
                .map(|x| x.nome.clone())
                .unwrap_or_else(|| "regra".into());
            let texto = if detalhe.is_empty() {
                format!("{hora} — {nome}: ocupada · {cam}")
            } else {
                format!("{hora} — {nome}: passagem ({detalhe}) · {cam}")
            };
            notificar(format!("regra:{r}"), texto, foto.clone());
        }
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
