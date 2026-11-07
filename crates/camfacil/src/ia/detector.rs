//! Thread do detector (nível médio): uma sessão ONNX compartilhada por todas as
//! câmeras, 2–3 inferências por segundo no total, no quadro já reduzido do
//! stream secundário. Só roda com algum módulo médio ligado e com runtime e
//! modelo baixados; parada, libera o modelo da RAM depois de um minuto.

use super::Ia;
use crate::video::{modulos, Video};
use cf_core::deteccao::{self, Rgb, LADO};
use cf_core::store::Store;
use ort::session::Session;
use ort::value::Tensor;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/// Intervalo mínimo entre inferências (≈3 q/s no total).
const INTERVALO: Duration = Duration::from_millis(340);
/// Confiança mínima de uma detecção.
const CONF_MIN: f32 = 0.35;
/// Sem módulo médio ligado por este tempo, descarrega o modelo.
const DESCARREGAR_APOS: Duration = Duration::from_secs(60);

static AMBIENTE: OnceLock<Result<ort::environment::Environment, String>> = OnceLock::new();

pub fn iniciar(ia: Arc<Ia>, video: Arc<Video>, store: Arc<Store>) {
    let _ = std::thread::Builder::new()
        .name("ia-detector".into())
        .spawn(move || laco(ia, video, store));
}

fn carregar(ia: &Ia) -> Result<Session, String> {
    let lib = ia.lib_ort().ok_or("ONNX Runtime ainda não baixado")?;
    let modelo = ia.modelo().ok_or("modelo ainda não baixado")?;
    let env = AMBIENTE
        .get_or_init(|| {
            ort::init_from(&lib)
                .map_err(|e| format!("não carregou o ONNX Runtime ({}): {e}", lib.display()))?
                .with_name("camfacil")
                .build()
                .map_err(|e| format!("ONNX Runtime: {e}"))
        })
        .as_ref()
        .map_err(|e| e.clone())?;
    let nucleos = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    let erro = |e: &dyn std::fmt::Display| format!("não abriu o modelo {}: {e}", modelo.display());
    Session::builder(env)
        .map_err(|e| erro(&e))?
        .with_intra_threads(nucleos.clamp(1, 2))
        .map_err(|e| erro(&e))?
        .with_inter_threads(1)
        .map_err(|e| erro(&e))?
        .commit_from_file(&modelo)
        .map_err(|e| erro(&e))
}

fn decodificar_rgb(jpeg: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    use zune_core::bytestream::ZCursor;
    use zune_core::colorspace::ColorSpace;
    use zune_core::options::DecoderOptions;
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut d = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(jpeg), opts);
    let px = d.decode().ok()?;
    let (w, h) = d.dimensions()?;
    (px.len() == w * h * 3).then_some((w, h, px))
}

fn inferir(sessao: &mut Session, jpeg: &[u8]) -> Result<Vec<deteccao::Deteccao>, String> {
    let (w, h, px) = decodificar_rgb(jpeg).ok_or("quadro JPEG inválido")?;
    let (tensor, enc) = deteccao::letterbox(&Rgb { w, h, px: &px });
    let entrada =
        Tensor::from_array(([1usize, 3, LADO, LADO], tensor)).map_err(|e| e.to_string())?;
    let saidas = sessao
        .run(ort::inputs![entrada])
        .map_err(|e| format!("inferência: {e}"))?;
    let (shape, dados) = saidas[0]
        .try_extract_tensor::<f32>()
        .map_err(|e| e.to_string())?;
    let shape: Vec<usize> = shape.iter().map(|&d| d.max(0) as usize).collect();
    Ok(deteccao::decodificar(dados, &shape, enc, w, h, CONF_MIN))
}

fn laco(ia: Arc<Ia>, video: Arc<Video>, store: Arc<Store>) {
    let mut sessao: Option<Session> = None;
    let mut ultimos: HashMap<i64, Arc<Vec<u8>>> = HashMap::new();
    let mut ocioso_desde = Instant::now();
    loop {
        let m = modulos(&store);
        if !m.qualquer_medio() || !ia.pronto() {
            if sessao.is_some() && ocioso_desde.elapsed() > DESCARREGAR_APOS {
                sessao = None;
                ia.detector.lock().unwrap().carregado = false;
                for c in video.canais() {
                    let mut md = c.medio.lock().unwrap();
                    md.deteccoes.clear();
                    md.quando = None;
                }
            }
            std::thread::sleep(Duration::from_secs(1));
            continue;
        }
        ocioso_desde = Instant::now();
        if sessao.is_none() {
            match carregar(&ia) {
                Ok(s) => {
                    sessao = Some(s);
                    let mut d = ia.detector.lock().unwrap();
                    d.carregado = true;
                    d.erro = None;
                }
                Err(e) => {
                    ia.detector.lock().unwrap().erro = Some(e);
                    std::thread::sleep(Duration::from_secs(10));
                    continue;
                }
            }
        }
        let Some(s) = sessao.as_mut() else { continue };
        // Permanência configurável (segundos) sem reiniciar.
        let perm = store
            .get("permanencia_s")
            .ok()
            .flatten()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60)
            .clamp(5, 3600);

        let mut trabalhou = false;
        for canal in video.canais() {
            let Some(jpeg) = canal.ultimo.lock().unwrap().clone() else {
                continue;
            };
            if ultimos
                .get(&canal.id)
                .is_some_and(|u| Arc::ptr_eq(u, &jpeg))
            {
                continue;
            }
            ultimos.insert(canal.id, jpeg.clone());
            trabalhou = true;
            let t0 = Instant::now();
            match inferir(s, &jpeg) {
                Ok(dets) => {
                    let agora = Instant::now();
                    let regras = canal.regras();
                    let eventos = {
                        let mut md = canal.medio.lock().unwrap();
                        md.rastreador.ajustes.permanencia = Duration::from_secs(perm);
                        let ev = md.rastreador.processar(
                            &dets,
                            &regras,
                            agora,
                            m.contagem,
                            m.permanencia,
                            m.calor,
                        );
                        md.deteccoes = dets;
                        md.quando = Some(agora);
                        ev
                    };
                    canal.aplicar_medio(&store, eventos);
                    let mut d = ia.detector.lock().unwrap();
                    d.inferencias += 1;
                    d.ultimo_ms = t0.elapsed().as_millis() as u64;
                    d.erro = None;
                }
                Err(e) => {
                    ia.detector.lock().unwrap().erro = Some(e);
                }
            }
            let gasto = t0.elapsed();
            if gasto < INTERVALO {
                std::thread::sleep(INTERVALO - gasto);
            }
        }
        ultimos.retain(|id, _| video.canal(*id).is_some());
        if !trabalhou {
            std::thread::sleep(Duration::from_millis(60));
        }
    }
}
