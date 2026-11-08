//! Teste de estresse (PRD §5.1, SDD-01 `bench`): quantas câmeras esta
//! máquina aguenta de verdade.
//!
//! Sobe câmeras de demonstração (640 px, 5 quadros/s, a mesma cena da demo)
//! com uma cerca virtual (zona + linha) em degraus de 1, 2, 3, 4, 6, 8…
//! Cada degrau aquece 4 s e mede 8 s: CPU e memória da máquina toda e os
//! quadros/s de cada câmera de teste. Para no primeiro degrau que passar do
//! limite (CPU média > 85 %, alguma câmera abaixo de 4 q/s, memória > 90 %,
//! temperatura > 85 °C) e o último degrau aprovado é o resultado. As câmeras
//! reais continuam ligadas: o teste mede o que cabe *além* delas.
//!
//! As câmeras de teste não gravam evento, não notificam e não aparecem na TV.

use crate::{App, Shared};
use axum::{extract::State, http::StatusCode, Json};
use cf_core::visao::{Forma, Ponto, Regra};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const CHAVE: &str = "estresse_resultado";
const DEGRAUS: &[usize] = &[1, 2, 3, 4, 6, 8, 10, 12, 16, 20, 24, 32];
const AQUECER: Duration = Duration::from_secs(4);
const MEDIR: Duration = Duration::from_secs(8);
pub const LIMITE_CPU: f32 = 85.0;
/// Acima disso antes de começar, o teste recusa (outro programa pesando).
const CPU_BASE_MAX: f32 = 60.0;
pub const LIMITE_RAM: f32 = 90.0;
pub const LIMITE_TEMP: f32 = 85.0;
/// A câmera de teste entrega 5 q/s; abaixo de 4 a máquina já está perdendo quadro.
pub const FPS_MIN: f32 = 4.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Degrau {
    pub cameras: usize,
    pub cpu: f32,
    pub ram_pct: f32,
    pub fps_min: f32,
    pub temperatura: Option<f32>,
    /// `None` = aprovado; senão, o motivo da reprovação.
    pub falha: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resultado {
    pub quando: i64,
    pub cameras_reais: usize,
    /// CPU média antes de subir as câmeras de teste.
    #[serde(default)]
    pub cpu_base: f32,
    /// Câmeras de teste que a máquina aguentou além das reais.
    pub aprovadas: usize,
    /// Com folga de 25 % para o dia a dia.
    pub recomendadas: usize,
    /// Chegou ao último degrau sem falhar (o limite real pode ser maior).
    pub teto: bool,
    pub degraus: Vec<Degrau>,
    pub veredito: String,
}

#[derive(Default, Serialize, Clone)]
struct Andamento {
    rodando: bool,
    cameras: usize,
    degraus: Vec<Degrau>,
    erro: Option<String>,
}

static ANDAMENTO: Mutex<Option<Andamento>> = Mutex::new(None);
static CANCELAR: AtomicBool = AtomicBool::new(false);

fn andamento() -> Andamento {
    ANDAMENTO.lock().unwrap().clone().unwrap_or_default()
}

fn atualizar(f: impl FnOnce(&mut Andamento)) {
    let mut g = ANDAMENTO.lock().unwrap();
    f(g.get_or_insert_with(Andamento::default));
}

/// Avalia um degrau medido; devolve o motivo da falha, se houver.
pub fn avaliar(cpu: f32, ram: f32, fps_min: f32, temp: Option<f32>) -> Option<String> {
    if cpu > LIMITE_CPU {
        Some(format!(
            "processador em {cpu:.0} %, acima de {LIMITE_CPU:.0} %"
        ))
    } else if fps_min < FPS_MIN {
        Some(format!(
            "câmera caiu para {fps_min:.1} quadros/s, abaixo de {FPS_MIN:.0}"
        ))
    } else if ram > LIMITE_RAM {
        Some(format!("memória em {ram:.0} %, acima de {LIMITE_RAM:.0} %"))
    } else {
        temp.filter(|t| *t > LIMITE_TEMP)
            .map(|t| format!("temperatura em {t:.0} °C, acima de {LIMITE_TEMP:.0} °C"))
    }
}

/// Resume os degraus em resultado e frase para o técnico e o relatório.
pub fn concluir(
    degraus: Vec<Degrau>,
    cameras_reais: usize,
    cpu_base: f32,
    quando: i64,
) -> Resultado {
    let aprovadas = degraus
        .iter()
        .take_while(|d| d.falha.is_none())
        .map(|d| d.cameras)
        .last()
        .unwrap_or(0);
    let teto = !degraus.is_empty() && degraus.iter().all(|d| d.falha.is_none());
    let recomendadas = aprovadas * 3 / 4;
    let alem = if cameras_reais > 0 {
        format!(" além das {cameras_reais} câmeras já ligadas")
    } else {
        String::new()
    };
    let veredito = match (aprovadas, degraus.iter().find_map(|d| d.falha.clone())) {
        (0, Some(m)) => format!(
            "esta máquina não aguentou nem 1 câmera de teste com cerca virtual{alem} ({m}). Use câmeras com IA de borda ou um computador mais forte."
        ),
        (n, Some(m)) => format!(
            "aguentou {n} câmera(s) com cerca virtual{alem}; com {} passou do limite ({m}). Recomendado no dia a dia: até {} para ter folga.",
            degraus.iter().find(|d| d.falha.is_some()).map(|d| d.cameras).unwrap_or(n + 1),
            recomendadas.max(1)
        ),
        (n, None) => format!(
            "aguentou {n} câmera(s) com cerca virtual{alem} sem chegar ao limite (fim do teste). Recomendado no dia a dia: até {}.",
            recomendadas.max(1)
        ),
    };
    Resultado {
        quando,
        cameras_reais,
        cpu_base,
        aprovadas,
        recomendadas: if aprovadas == 0 {
            0
        } else {
            recomendadas.max(1)
        },
        teto,
        degraus,
        veredito,
    }
}

fn regras_teste() -> Vec<Regra> {
    let p = |x: f32, y: f32| Ponto { x, y };
    vec![
        Regra {
            id: 1,
            nome: "Zona de teste".into(),
            forma: Forma::Zona,
            pontos: vec![p(0.3, 0.2), p(0.7, 0.2), p(0.7, 0.9), p(0.3, 0.9)],
        },
        Regra {
            id: 2,
            nome: "Linha de teste".into(),
            forma: Forma::Linha,
            pontos: vec![p(0.5, 0.1), p(0.5, 0.95)],
        },
    ]
}

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Espera em fatias de 250 ms para o cancelamento responder rápido.
fn esperar(d: Duration) -> bool {
    let fim = std::time::Instant::now() + d;
    while std::time::Instant::now() < fim {
        if CANCELAR.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    true
}

/// CPU da máquina toda numa janela de 500 ms.
fn medir_cpu(sys: &mut sysinfo::System) -> f32 {
    sys.refresh_cpu_usage();
    std::thread::sleep(Duration::from_millis(500));
    sys.refresh_cpu_usage();
    sys.global_cpu_usage()
}

fn rodar(app: &App, maximo: usize) -> Result<Resultado, String> {
    if crate::video::achar_ffmpeg().is_none() {
        return Err("ffmpeg não encontrado; instale na tela Câmeras antes do teste".into());
    }
    let cameras_reais = app.video.canais().len();
    let mut sys = sysinfo::System::new().map_err(|e| e.to_string())?;
    // Linha de base: se a máquina já está ocupada, o teste não diz nada.
    let mut base = 0.0f32;
    for _ in 0..3 {
        if CANCELAR.load(Ordering::Relaxed) {
            return Err("teste cancelado".into());
        }
        base += medir_cpu(&mut sys);
        std::thread::sleep(Duration::from_millis(500));
    }
    let base = base / 3.0;
    if base > CPU_BASE_MAX {
        return Err(format!(
            "o processador já está em {base:.0} % antes do teste; feche outros programas e tente de novo"
        ));
    }
    let mut canais = Vec::new();
    let mut degraus: Vec<Degrau> = Vec::new();
    for &n in DEGRAUS.iter().filter(|n| **n <= maximo) {
        atualizar(|a| a.cameras = n);
        while canais.len() < n {
            canais.push(app.video.iniciar_ensaio(regras_teste()));
        }
        if !esperar(AQUECER) {
            return Err("teste cancelado".into());
        }
        let (mut cpu, mut ram, mut k) = (0.0f32, 0.0f32, 0u32);
        let fim = std::time::Instant::now() + MEDIR;
        while std::time::Instant::now() < fim {
            if CANCELAR.load(Ordering::Relaxed) {
                return Err("teste cancelado".into());
            }
            cpu += medir_cpu(&mut sys);
            sys.refresh_memory();
            let total = sys.total_memory().max(1);
            ram += sys.used_memory() as f32 * 100.0 / total as f32;
            k += 1;
            std::thread::sleep(Duration::from_millis(500));
        }
        let k = k.max(1) as f32;
        let (cpu, ram) = (cpu / k, ram / k);
        let estados: Vec<_> = canais.iter().map(|c| c.estado()).collect();
        if let Some(e) = estados.iter().find(|e| !e.online && e.erro.is_some()) {
            return Err(format!(
                "a câmera de teste não abriu: {}",
                e.erro.clone().unwrap_or_default()
            ));
        }
        let fps_min = estados.iter().map(|e| e.fps).fold(f32::MAX, f32::min);
        let temperatura = crate::hardware::temperatura();
        let falha = avaliar(cpu, ram, fps_min, temperatura);
        let parar = falha.is_some();
        let d = Degrau {
            cameras: n,
            cpu,
            ram_pct: ram,
            fps_min,
            temperatura,
            falha,
        };
        atualizar(|a| a.degraus.push(d.clone()));
        degraus.push(d);
        if parar {
            break;
        }
    }
    Ok(concluir(degraus, cameras_reais, base, agora()))
}

// ---------------------------------------------------------------------------
// API
// ---------------------------------------------------------------------------

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn resultado_salvo(app: &App) -> Value {
    app.store
        .get(CHAVE)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}

/// `GET /api/estresse`: andamento do teste e último resultado.
pub async fn get_estado(State(app): State<Shared>) -> Json<Value> {
    Json(json!({
        "andamento": andamento(),
        "resultado": resultado_salvo(&app),
        "limites": { "cpu": LIMITE_CPU, "ram": LIMITE_RAM, "temperatura": LIMITE_TEMP, "fps_min": FPS_MIN },
    }))
}

#[derive(Deserialize, Default)]
pub struct Pedido {
    #[serde(default)]
    maximo: Option<usize>,
}

/// `POST /api/estresse`: começa o teste em segundo plano.
pub async fn post_iniciar(State(app): State<Shared>, body: Option<Json<Pedido>>) -> ApiResult {
    let maximo = body
        .and_then(|b| b.0.maximo)
        .unwrap_or(16)
        .clamp(1, *DEGRAUS.last().unwrap());
    {
        let mut g = ANDAMENTO.lock().unwrap();
        if g.as_ref().is_some_and(|a| a.rodando) {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({ "erro": "o teste já está rodando" })),
            ));
        }
        *g = Some(Andamento {
            rodando: true,
            ..Default::default()
        });
    }
    CANCELAR.store(false, Ordering::Relaxed);
    let app2: Arc<App> = app.clone();
    std::thread::Builder::new()
        .name("estresse".into())
        .spawn(move || {
            let r = rodar(&app2, maximo);
            app2.video.parar_ensaios();
            match r {
                Ok(res) => {
                    if let Ok(s) = serde_json::to_string(&res) {
                        let _ = app2.store.set(CHAVE, &s);
                    }
                    atualizar(|a| a.rodando = false);
                }
                Err(e) => atualizar(|a| {
                    a.rodando = false;
                    a.erro = Some(e);
                }),
            }
        })
        .map_err(|e| {
            atualizar(|a| a.rodando = false);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "erro": e.to_string() })),
            )
        })?;
    Ok(Json(json!({ "iniciado": true, "maximo": maximo })))
}

/// `DELETE /api/estresse`: cancela (as câmeras de teste param em até 1 s).
pub async fn delete_cancelar() -> Json<Value> {
    CANCELAR.store(true, Ordering::Relaxed);
    Json(json!({ "cancelando": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(n: usize, falha: Option<&str>) -> Degrau {
        Degrau {
            cameras: n,
            cpu: 50.0,
            ram_pct: 40.0,
            fps_min: 5.0,
            temperatura: None,
            falha: falha.map(String::from),
        }
    }

    #[test]
    fn limites() {
        assert!(avaliar(50.0, 50.0, 5.0, Some(60.0)).is_none());
        assert!(avaliar(90.0, 50.0, 5.0, None)
            .unwrap()
            .contains("processador"));
        assert!(avaliar(50.0, 50.0, 3.0, None).unwrap().contains("quadros"));
        assert!(avaliar(50.0, 95.0, 5.0, None).unwrap().contains("memória"));
        assert!(avaliar(50.0, 50.0, 5.0, Some(90.0))
            .unwrap()
            .contains("temperatura"));
    }

    #[test]
    fn conclusao() {
        let r = concluir(
            vec![
                d(1, None),
                d(2, None),
                d(3, None),
                d(4, None),
                d(6, Some("cpu")),
            ],
            2,
            10.0,
            0,
        );
        assert_eq!(r.aprovadas, 4);
        assert_eq!(r.recomendadas, 3);
        assert!(!r.teto);
        assert!(r.veredito.contains("aguentou 4"));
        assert!(r.veredito.contains("com 6"));
        assert!(r.veredito.contains("além das 2"));

        let r = concluir(vec![d(1, Some("cpu"))], 0, 0.0, 0);
        assert_eq!(r.aprovadas, 0);
        assert_eq!(r.recomendadas, 0);
        assert!(r.veredito.contains("nem 1"));

        let r = concluir(vec![d(1, None), d(2, None)], 0, 0.0, 0);
        assert!(r.teto);
        assert_eq!(r.recomendadas, 1);
    }
}
