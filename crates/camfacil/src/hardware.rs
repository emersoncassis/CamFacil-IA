//! Monitor de hardware com log (PRD §5.4, SDD-05): a cada 60 s mede CPU, RAM,
//! temperatura, disco livre da pasta de dados, uptime e quadros/s por câmera,
//! grava em `data_dir/hardware.db` (tabela `hardware_log`), retém 30 dias e
//! agrega por hora depois de 48 h. Alertas (CPU > 85 % por 10 min, RAM > 90 %,
//! disco < 1 GB, temperatura > 80 °C) geram evento `hardware` e vão ao
//! Telegram com antirrepique de 1 h.
//!
//! Banco separado do `camfacil.db` de propósito: é histórico, não configuração
//! (não entra no backup) e pode ser apagado sem perder nada.

use crate::App;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

pub const INTERVALO: Duration = Duration::from_secs(60);
const RETENCAO_S: i64 = 30 * 86_400;
const DETALHE_S: i64 = 48 * 3600;
pub const LIMITE_CPU: f32 = 85.0;
pub const LIMITE_CPU_MIN: u32 = 10;
pub const LIMITE_RAM: f32 = 90.0;
pub const LIMITE_DISCO_MB: u64 = 1024;
pub const LIMITE_TEMP: f32 = 80.0;
const ANTIRREPIQUE: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Serialize, Default)]
pub struct Amostra {
    pub quando: i64,
    pub cpu: f32,
    pub ram_pct: f32,
    pub ram_mb: u64,
    pub ram_total_mb: u64,
    /// `None` quando o sistema não expõe sensor sem driver (ex.: Windows).
    pub temperatura: Option<f32>,
    pub disco_livre_mb: Option<u64>,
    pub uptime_s: u64,
    /// Quadros por segundo por câmera (id → q/s).
    pub fps: HashMap<i64, f32>,
    /// Linhas agregadas (média da hora) depois de 48 h.
    pub agregada: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Alerta {
    pub chave: &'static str,
    pub texto: String,
    pub quando: i64,
}

/// Avaliador dos limites: estado puro, testável sem máquina.
#[derive(Default)]
pub struct Limites {
    cpu_alta_desde: Option<i64>,
}

impl Limites {
    /// Devolve os alertas disparados por esta amostra.
    pub fn avaliar(&mut self, a: &Amostra) -> Vec<Alerta> {
        let mut v = Vec::new();
        if a.cpu > LIMITE_CPU {
            let desde = *self.cpu_alta_desde.get_or_insert(a.quando);
            if a.quando - desde >= (LIMITE_CPU_MIN as i64) * 60 {
                v.push(Alerta {
                    chave: "cpu",
                    texto: format!(
                        "Processador acima de {LIMITE_CPU:.0} % há mais de {LIMITE_CPU_MIN} min ({:.0} %). A máquina está no limite: reduza câmeras ou módulos, ou troque por câmeras com IA de borda.",
                        a.cpu
                    ),
                    quando: a.quando,
                });
            }
        } else {
            self.cpu_alta_desde = None;
        }
        if a.ram_pct > LIMITE_RAM {
            v.push(Alerta {
                chave: "ram",
                texto: format!(
                    "Memória acima de {LIMITE_RAM:.0} % ({:.0} % · {} de {} MB). Risco de travar.",
                    a.ram_pct, a.ram_mb, a.ram_total_mb
                ),
                quando: a.quando,
            });
        }
        if let Some(d) = a.disco_livre_mb {
            if d < LIMITE_DISCO_MB {
                v.push(Alerta {
                    chave: "disco",
                    texto: format!(
                        "Disco da pasta de dados com menos de 1 GB livre ({d} MB). Libere espaço."
                    ),
                    quando: a.quando,
                });
            }
        }
        if let Some(t) = a.temperatura {
            if t > LIMITE_TEMP {
                v.push(Alerta {
                    chave: "temperatura",
                    texto: format!(
                        "Temperatura em {t:.0} °C (limite {LIMITE_TEMP:.0} °C). Confira ventilação e poeira."
                    ),
                    quando: a.quando,
                });
            }
        }
        v
    }
}

/// Temperatura em °C pelos sensores do sistema. Linux: `/sys/class/thermal`
/// (maior zona, costuma ser a CPU). Windows/macOS: `None` (precisa de driver).
pub fn temperatura() -> Option<f32> {
    temperatura_em(Path::new("/sys/class/thermal"))
}

fn temperatura_em(base: &Path) -> Option<f32> {
    let mut max: Option<f32> = None;
    for e in std::fs::read_dir(base).ok()?.flatten() {
        if !e.file_name().to_string_lossy().starts_with("thermal_zone") {
            continue;
        }
        let Ok(s) = std::fs::read_to_string(e.path().join("temp")) else {
            continue;
        };
        let Ok(mili) = s.trim().parse::<f32>() else {
            continue;
        };
        let c = mili / 1000.0;
        if (0.0..=150.0).contains(&c) {
            max = Some(max.map_or(c, |m| m.max(c)));
        }
    }
    max
}

/// Espaço livre do disco onde fica a pasta de dados (MB).
fn disco_livre_mb(dir: &Path) -> Option<u64> {
    use sysinfo::{DiskRefreshKind, Disks};
    let disks =
        Disks::new_with_refreshed_list_specifics(DiskRefreshKind::nothing().with_storage()).ok()?;
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    // O ponto de montagem mais longo que é prefixo da pasta.
    disks
        .iter()
        .filter(|d| dir.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space() / 1_048_576)
}

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Banco
// ---------------------------------------------------------------------------

pub struct Log {
    conn: Mutex<Connection>,
}

impl Log {
    pub fn abrir(dir: &Path) -> rusqlite::Result<Self> {
        Self::init(Connection::open(dir.join("hardware.db"))?)
    }

    #[cfg(test)]
    pub fn memoria() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> rusqlite::Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS hardware_log (
                 quando        INTEGER PRIMARY KEY,
                 cpu           REAL NOT NULL,
                 ram_pct       REAL NOT NULL,
                 ram_mb        INTEGER NOT NULL,
                 ram_total_mb  INTEGER NOT NULL,
                 temperatura   REAL,
                 disco_livre_mb INTEGER,
                 uptime_s      INTEGER NOT NULL,
                 fps           TEXT NOT NULL DEFAULT '{}',
                 agregada      INTEGER NOT NULL DEFAULT 0
             );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn gravar(&self, a: &Amostra) -> rusqlite::Result<()> {
        self.lock().execute(
            "INSERT OR REPLACE INTO hardware_log(quando, cpu, ram_pct, ram_mb, ram_total_mb, temperatura, disco_livre_mb, uptime_s, fps, agregada)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                a.quando,
                a.cpu,
                a.ram_pct,
                a.ram_mb as i64,
                a.ram_total_mb as i64,
                a.temperatura,
                a.disco_livre_mb.map(|d| d as i64),
                a.uptime_s as i64,
                serde_json::to_string(&a.fps).unwrap_or_else(|_| "{}".into()),
                a.agregada as i64
            ],
        )?;
        Ok(())
    }

    /// Retenção de 30 dias e agregação por hora das amostras com mais de 48 h.
    pub fn arrumar(&self, agora: i64) -> rusqlite::Result<usize> {
        let c = self.lock();
        c.execute(
            "DELETE FROM hardware_log WHERE quando < ?1",
            params![agora - RETENCAO_S],
        )?;
        let limite = agora - DETALHE_S;
        let mut horas: Vec<i64> = {
            let mut st = c.prepare(
                "SELECT DISTINCT (quando / 3600) * 3600 FROM hardware_log WHERE agregada = 0 AND quando < ?1",
            )?;
            let v: rusqlite::Result<Vec<i64>> =
                st.query_map(params![limite], |r| r.get(0))?.collect();
            v?
        };
        horas.sort_unstable();
        for h in &horas {
            c.execute_batch("BEGIN")?;
            let r = (|| -> rusqlite::Result<()> {
                let (cpu, ram_pct, ram_mb, ram_total, temp, disco, uptime): (
                    f64,
                    f64,
                    f64,
                    f64,
                    Option<f64>,
                    Option<f64>,
                    f64,
                ) = c.query_row(
                    "SELECT AVG(cpu), AVG(ram_pct), AVG(ram_mb), MAX(ram_total_mb), MAX(temperatura), MIN(disco_livre_mb), MAX(uptime_s)
                     FROM hardware_log WHERE agregada = 0 AND quando >= ?1 AND quando < ?1 + 3600",
                    params![h],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                            r.get(6)?,
                        ))
                    },
                )?;
                c.execute(
                    "DELETE FROM hardware_log WHERE agregada = 0 AND quando >= ?1 AND quando < ?1 + 3600",
                    params![h],
                )?;
                c.execute(
                    "INSERT OR REPLACE INTO hardware_log(quando, cpu, ram_pct, ram_mb, ram_total_mb, temperatura, disco_livre_mb, uptime_s, fps, agregada)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, '{}', 1)",
                    params![
                        h,
                        cpu,
                        ram_pct,
                        ram_mb as i64,
                        ram_total as i64,
                        temp,
                        disco.map(|d| d as i64),
                        uptime as i64
                    ],
                )?;
                Ok(())
            })();
            match r {
                Ok(()) => c.execute_batch("COMMIT")?,
                Err(e) => {
                    let _ = c.execute_batch("ROLLBACK");
                    return Err(e);
                }
            }
        }
        Ok(horas.len())
    }

    /// Amostras desde um instante, da mais antiga para a mais nova.
    pub fn desde(&self, desde: i64) -> rusqlite::Result<Vec<Amostra>> {
        let c = self.lock();
        let mut st = c.prepare(
            "SELECT quando, cpu, ram_pct, ram_mb, ram_total_mb, temperatura, disco_livre_mb, uptime_s, fps, agregada
             FROM hardware_log WHERE quando >= ?1 ORDER BY quando",
        )?;
        let v = st
            .query_map(params![desde], |r| {
                let fps: String = r.get(8)?;
                Ok(Amostra {
                    quando: r.get(0)?,
                    cpu: r.get(1)?,
                    ram_pct: r.get(2)?,
                    ram_mb: r.get::<_, i64>(3)? as u64,
                    ram_total_mb: r.get::<_, i64>(4)? as u64,
                    temperatura: r.get(5)?,
                    disco_livre_mb: r.get::<_, Option<i64>>(6)?.map(|d| d as u64),
                    uptime_s: r.get::<_, i64>(7)? as u64,
                    fps: serde_json::from_str(&fps).unwrap_or_default(),
                    agregada: r.get::<_, i64>(9)? != 0,
                })
            })?
            .collect();
        v
    }

    pub fn total(&self) -> rusqlite::Result<i64> {
        self.lock()
            .query_row("SELECT COUNT(*) FROM hardware_log", [], |r| r.get(0))
    }
}

// ---------------------------------------------------------------------------
// Serviço
// ---------------------------------------------------------------------------

pub struct Monitor {
    pub log: Log,
    atual: Mutex<Option<Amostra>>,
    alertas: Mutex<Vec<Alerta>>,
    ultimo_envio: Mutex<HashMap<&'static str, Instant>>,
    limites: Mutex<Limites>,
    dir: PathBuf,
}

static MONITOR: OnceLock<Arc<Monitor>> = OnceLock::new();

pub fn monitor() -> Option<Arc<Monitor>> {
    MONITOR.get().cloned()
}

impl Monitor {
    pub fn atual(&self) -> Option<Amostra> {
        self.atual.lock().unwrap().clone()
    }

    /// Alertas ativos agora (últimos 10 por tipo, mais novo primeiro).
    pub fn alertas(&self) -> Vec<Alerta> {
        self.alertas.lock().unwrap().clone()
    }

    /// Uma medição completa (bloqueante por ~200 ms por causa da CPU).
    pub fn medir(&self, app: &App, sys: &mut sysinfo::System) -> Amostra {
        sys.refresh_cpu_usage();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let total = sys.total_memory();
        let usada = sys.used_memory();
        let fps: HashMap<i64, f32> = app
            .video
            .canais()
            .iter()
            .filter(|c| c.id > 0)
            .map(|c| (c.id, c.estado().fps))
            .collect();
        Amostra {
            quando: agora(),
            cpu: sys.global_cpu_usage(),
            ram_pct: if total > 0 {
                usada as f32 * 100.0 / total as f32
            } else {
                0.0
            },
            ram_mb: usada / 1_048_576,
            ram_total_mb: total / 1_048_576,
            temperatura: temperatura(),
            disco_livre_mb: disco_livre_mb(&self.dir),
            uptime_s: sysinfo::System::uptime().unwrap_or(0),
            fps,
            agregada: false,
        }
    }

    fn processar(&self, app: &App, a: Amostra) {
        let novos = self.limites.lock().unwrap().avaliar(&a);
        let agora_i = Instant::now();
        for al in novos {
            let mut ultimo = self.ultimo_envio.lock().unwrap();
            let manda = ultimo
                .get(al.chave)
                .is_none_or(|t| agora_i.duration_since(*t) >= ANTIRREPIQUE);
            if !manda {
                continue;
            }
            ultimo.insert(al.chave, agora_i);
            drop(ultimo);
            let _ = app
                .store
                .registrar_evento(0, None, "hardware", &al.texto, al.quando);
            if let Some(tg) = app.telegram.get() {
                tg.notificar(cf_notify::Alerta {
                    chave: format!("hardware:{}", al.chave),
                    texto: format!("Máquina: {}", al.texto),
                    foto: None,
                });
            }
            let mut lista = self.alertas.lock().unwrap();
            lista.insert(0, al);
            lista.truncate(20);
        }
        if let Err(e) = self.log.gravar(&a) {
            eprintln!("hardware_log: {e}");
        }
        *self.atual.lock().unwrap() = Some(a);
    }
}

/// Sobe a thread do monitor (uma vez). Falha silenciosa se o banco não abrir.
pub fn iniciar(app: Weak<App>) {
    let dir = cf_core::paths::data_dir();
    let log = match Log::abrir(&dir) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Monitor de hardware desligado: {e}");
            return;
        }
    };
    let m = Arc::new(Monitor {
        log,
        atual: Mutex::new(None),
        alertas: Mutex::new(Vec::new()),
        ultimo_envio: Mutex::new(HashMap::new()),
        limites: Mutex::new(Limites::default()),
        dir,
    });
    if MONITOR.set(m.clone()).is_err() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("hardware".into())
        .spawn(move || {
            let Ok(mut sys) = sysinfo::System::new() else {
                return;
            };
            let mut n: u64 = 0;
            loop {
                let Some(app) = app.upgrade() else { return };
                let a = m.medir(&app, &mut sys);
                m.processar(&app, a);
                drop(app);
                // Arruma o banco uma vez por hora.
                if n.is_multiple_of(60) {
                    let _ = m.log.arrumar(agora());
                }
                n += 1;
                std::thread::sleep(INTERVALO);
            }
        });
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

use crate::Shared;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn indisponivel() -> (StatusCode, Json<Value>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "erro": "monitor de hardware desligado" })),
    )
}

/// `GET /api/hardware`: amostra atual, limites e alertas recentes.
pub async fn get_atual(State(app): State<Shared>) -> ApiResult {
    let m = monitor().ok_or_else(indisponivel)?;
    let eventos: Vec<Value> = app
        .store
        .eventos_tipo("hardware", 20)
        .unwrap_or_default()
        .into_iter()
        .map(|e| json!({ "texto": e.detalhe, "quando": e.quando }))
        .collect();
    Ok(Json(json!({
        "atual": m.atual(),
        "alertas": m.alertas(),
        "eventos": eventos,
        "limites": {
            "cpu": LIMITE_CPU, "cpu_min": LIMITE_CPU_MIN, "ram": LIMITE_RAM,
            "disco_mb": LIMITE_DISCO_MB, "temperatura": LIMITE_TEMP,
        },
        "intervalo_s": INTERVALO.as_secs(),
        "amostras": m.log.total().unwrap_or(0),
        "sistema": std::env::consts::OS,
    })))
}

#[derive(Deserialize)]
pub struct HorasQuery {
    #[serde(default = "vinte_quatro")]
    horas: i64,
}
fn vinte_quatro() -> i64 {
    24
}

/// `GET /api/hardware/log?horas=24|168`: série para o gráfico.
pub async fn get_log(Query(q): Query<HorasQuery>) -> ApiResult {
    let m = monitor().ok_or_else(indisponivel)?;
    let horas = q.horas.clamp(1, 24 * 30);
    let desde = agora() - horas * 3600;
    let lista = tokio::task::spawn_blocking(move || m.log.desde(desde))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": e.to_string()})),
            )
        })?
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"erro": e.to_string()})),
            )
        })?;
    Ok(Json(json!({ "horas": horas, "amostras": lista })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amostra(quando: i64, cpu: f32) -> Amostra {
        Amostra {
            quando,
            cpu,
            ram_pct: 40.0,
            ram_mb: 1000,
            ram_total_mb: 4000,
            temperatura: Some(50.0),
            disco_livre_mb: Some(20_000),
            uptime_s: 10,
            fps: HashMap::from([(1, 5.0)]),
            agregada: false,
        }
    }

    #[test]
    fn cpu_so_depois_de_dez_minutos() {
        let mut l = Limites::default();
        assert!(l.avaliar(&amostra(0, 95.0)).is_empty());
        assert!(l.avaliar(&amostra(540, 95.0)).is_empty());
        let v = l.avaliar(&amostra(600, 95.0));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].chave, "cpu");
        // Caiu: zera a contagem.
        assert!(l.avaliar(&amostra(660, 10.0)).is_empty());
        assert!(l.avaliar(&amostra(720, 95.0)).is_empty());
    }

    #[test]
    fn ram_disco_temperatura() {
        let mut l = Limites::default();
        let mut a = amostra(0, 10.0);
        a.ram_pct = 95.0;
        a.disco_livre_mb = Some(500);
        a.temperatura = Some(90.0);
        let chaves: Vec<_> = l.avaliar(&a).into_iter().map(|x| x.chave).collect();
        assert_eq!(chaves, vec!["ram", "disco", "temperatura"]);
        a.temperatura = None;
        a.disco_livre_mb = None;
        a.ram_pct = 10.0;
        assert!(l.avaliar(&a).is_empty());
    }

    #[test]
    fn log_grava_le_e_agrega() {
        let log = Log::memoria().unwrap();
        let agora = 1_000_000_000i64;
        // 3 dias atrás, 4 amostras numa hora; e uma recente.
        let velha = agora - 3 * 86_400;
        for i in 0..4 {
            log.gravar(&amostra(velha + i * 60, 10.0 * (i + 1) as f32))
                .unwrap();
        }
        log.gravar(&amostra(agora - 60, 7.0)).unwrap();
        // E uma com 40 dias: cai pela retenção.
        log.gravar(&amostra(agora - 40 * 86_400, 1.0)).unwrap();
        assert_eq!(log.total().unwrap(), 6);
        assert_eq!(log.arrumar(agora).unwrap(), 1);
        let tudo = log.desde(0).unwrap();
        assert_eq!(tudo.len(), 2);
        assert!(tudo[0].agregada);
        assert!((tudo[0].cpu - 25.0).abs() < 0.01);
        assert_eq!(tudo[0].quando % 3600, 0);
        assert!(!tudo[1].agregada);
        assert_eq!(tudo[1].fps.get(&1), Some(&5.0));
        // Segunda passada não agrega de novo.
        assert_eq!(log.arrumar(agora).unwrap(), 0);
    }

    #[test]
    fn temperatura_sysfs_simulado() {
        let dir = std::env::temp_dir().join(format!("cf-termal-{}", std::process::id()));
        let z0 = dir.join("thermal_zone0");
        let z1 = dir.join("thermal_zone1");
        std::fs::create_dir_all(&z0).unwrap();
        std::fs::create_dir_all(&z1).unwrap();
        std::fs::write(z0.join("temp"), "45000\n").unwrap();
        std::fs::write(z1.join("temp"), "61500\n").unwrap();
        assert_eq!(temperatura_em(&dir), Some(61.5));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(temperatura_em(&dir), None);
    }
}
