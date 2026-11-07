//! Benchmark da máquina: decide quais níveis de IA podem ser ligados.
//!
//! Fase 1: estimativa por desempenho de ponto flutuante (o que a inferência usa).
//! Fase 2: substituir pela inferência real de um modelo de referência em ONNX
//! (SDD-02, "Níveis e benchmark"). Os limites abaixo são provisórios.

use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Leve,
    Medio,
    Pesado,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Leve => "leve",
            Level::Medio => "medio",
            Level::Pesado => "pesado",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "leve" => Some(Level::Leve),
            "medio" => Some(Level::Medio),
            "pesado" => Some(Level::Pesado),
            _ => None,
        }
    }
}

/// Custo aproximado de uma inferência de um detector "nano" em 320×320.
const NANO_GFLOP_PER_FRAME: f64 = 2.2;
/// Fração do pico medido que um runtime de inferência real costuma aproveitar.
const RUNTIME_EFFICIENCY: f64 = 0.5;
/// Quadros por segundo por câmera para considerar o nível usável.
const MIN_FPS_PER_CAMERA: f64 = 3.0;

#[derive(Debug, Clone, Serialize)]
pub struct BenchResult {
    pub gflops: f64,
    pub threads: usize,
    /// Quadros por segundo estimados de um detector nano (todas as threads).
    pub fps_estimado: f64,
    /// Câmeras estimadas com detecção de pessoas a 3 fps.
    pub cameras_medio: u32,
    pub nivel: Level,
    pub duracao_ms: u128,
}

/// Classifica o nível a partir dos quadros por segundo estimados.
pub fn classify(fps: f64) -> Level {
    if fps >= MIN_FPS_PER_CAMERA * 4.0 {
        Level::Pesado
    } else if fps >= MIN_FPS_PER_CAMERA {
        Level::Medio
    } else {
        Level::Leve
    }
}

/// Kernel parecido com o miolo de uma convolução: multiplica e acumula em blocos
/// que cabem no cache. Retorna o número de operações de ponto flutuante feitas.
fn kernel(seconds: Duration) -> (u64, f32) {
    const N: usize = 4096;
    let a: Vec<f32> = (0..N).map(|i| (i % 13) as f32 * 0.01).collect();
    let b: Vec<f32> = (0..N).map(|i| (i % 7) as f32 * 0.02).collect();
    let mut acc = [0f32; 8];
    let mut flops = 0u64;
    let start = Instant::now();
    while start.elapsed() < seconds {
        for _ in 0..64 {
            let (ca8, _) = a.as_chunks::<8>();
            let (cb8, _) = b.as_chunks::<8>();
            for (ca, cb) in ca8.iter().zip(cb8) {
                // Multiplicação e soma separadas: `mul_add` vira chamada de software
                // lenta em binários genéricos (sem FMA), o que falsearia o resultado.
                for k in 0..8 {
                    acc[k] += ca[k] * cb[k];
                }
            }
            flops += (N as u64) * 2;
        }
        // Evita que o compilador elimine o trabalho.
        acc = std::hint::black_box(acc);
    }
    (flops, acc.iter().sum())
}

/// Roda o benchmark em todas as threads por `seconds`. É bloqueante: chame em
/// `spawn_blocking`.
pub fn run(seconds: Duration) -> BenchResult {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let start = Instant::now();
    let handles: Vec<_> = (0..threads)
        .map(|_| std::thread::spawn(move || kernel(seconds)))
        .collect();
    let mut flops = 0u64;
    for h in handles {
        if let Ok((f, sink)) = h.join() {
            flops += f;
            std::hint::black_box(sink);
        }
    }
    let elapsed = start.elapsed();
    let gflops = flops as f64 / elapsed.as_secs_f64() / 1e9;
    let fps = gflops * RUNTIME_EFFICIENCY / NANO_GFLOP_PER_FRAME;
    BenchResult {
        gflops: (gflops * 10.0).round() / 10.0,
        threads,
        fps_estimado: (fps * 10.0).round() / 10.0,
        cameras_medio: (fps / MIN_FPS_PER_CAMERA).floor().max(0.0) as u32,
        nivel: classify(fps),
        duracao_ms: elapsed.as_millis(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classificacao() {
        assert_eq!(classify(0.5), Level::Leve);
        assert_eq!(classify(3.0), Level::Medio);
        assert_eq!(classify(11.9), Level::Medio);
        assert_eq!(classify(12.0), Level::Pesado);
    }

    #[test]
    fn roda_rapido() {
        let r = run(Duration::from_millis(150));
        assert!(r.gflops > 0.0);
        assert!(r.threads >= 1);
        assert!(r.duracao_ms < 2000);
    }

    #[test]
    fn nivel_texto() {
        for l in [Level::Leve, Level::Medio, Level::Pesado] {
            assert_eq!(Level::parse(l.as_str()), Some(l));
        }
    }
}
