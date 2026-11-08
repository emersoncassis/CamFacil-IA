//! Nível médio (SDD-02): tudo que cerca o detector de objetos, sem a rede.
//!
//! Pré-processamento (letterbox RGB → tensor), leitura da saída do YOLO
//! (84×N), NMS, rastreamento simples por proximidade, contagem por linha,
//! permanência em zona e mapa de calor. O ONNX Runtime em si fica no binário
//! (`camfacil::ia`); aqui é só matemática testável.

use crate::visao::{cruzou, dentro, Forma, Intel, Ponto, Regra};
use serde::Serialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Lado da entrada do modelo (YOLOv8n/YOLO11n exportados em 640).
pub const LADO: usize = 640;

/// Classes do COCO que interessam (índice → nome em pt-BR).
pub const CLASSES: &[(usize, &str)] = &[
    (0, "pessoa"),
    (1, "bicicleta"),
    (2, "carro"),
    (3, "moto"),
    (5, "ônibus"),
    (7, "caminhão"),
];

/// Classes de um **modelo de estudo** (SDD-07) entram a partir deste índice,
/// para nunca se confundirem com as do COCO (`0` = pessoa). A classe `i` da
/// receita vira `CLASSE_ESTUDO + i`.
pub const CLASSE_ESTUDO: usize = 1000;

/// A trilha conta para zona, permanência e calor? Pessoa (COCO) ou qualquer
/// objeto de um modelo de estudo.
pub fn conta_em_zona(classe: usize) -> bool {
    classe == 0 || classe >= CLASSE_ESTUDO
}

/// A regra observa esta classe? Sem classes escolhidas vale o comportamento
/// de antes do SDD-09: linha conta qualquer uma, zona só pessoa/estudo.
pub fn regra_casa(r: &Regra, classe: usize) -> bool {
    let cs = &r.ajustes.classes;
    if cs.is_empty() {
        return r.forma == Forma::Linha || conta_em_zona(classe);
    }
    cs.contains(&classe)
}

pub fn nome_classe(c: usize) -> Option<&'static str> {
    CLASSES.iter().find(|(i, _)| *i == c).map(|(_, n)| *n)
}

/// Plural curto para o balão: "2 pessoas", "1 carro".
pub fn plural(c: usize, n: usize) -> String {
    let nome = nome_classe(c).unwrap_or("objeto");
    if n == 1 {
        format!("1 {nome}")
    } else {
        let p = match nome {
            "pessoa" => "pessoas",
            "bicicleta" => "bicicletas",
            "carro" => "carros",
            "moto" => "motos",
            "ônibus" => "ônibus",
            "caminhão" => "caminhões",
            _ => "objetos",
        };
        format!("{n} {p}")
    }
}

/// Imagem RGB intercalada (8 bits).
pub struct Rgb<'a> {
    pub w: usize,
    pub h: usize,
    pub px: &'a [u8],
}

/// Como a imagem foi encaixada no quadrado do modelo.
#[derive(Debug, Clone, Copy)]
pub struct Encaixe {
    pub escala: f32,
    pub dx: f32,
    pub dy: f32,
}

/// Letterbox: redimensiona mantendo proporção, centraliza em 640×640 cinza
/// (114) e devolve o tensor CHW normalizado 0–1. Vizinho mais próximo: barato
/// e suficiente para um detector nano.
pub fn letterbox(img: &Rgb<'_>) -> (Vec<f32>, Encaixe) {
    letterbox_lado(img, LADO)
}

/// Letterbox para um lado qualquer (modelo de estudo treinado em 320–1280).
pub fn letterbox_lado(img: &Rgb<'_>, lado: usize) -> (Vec<f32>, Encaixe) {
    let escala = (lado as f32 / img.w as f32).min(lado as f32 / img.h as f32);
    let nw = ((img.w as f32 * escala).round() as usize).clamp(1, lado);
    let nh = ((img.h as f32 * escala).round() as usize).clamp(1, lado);
    let dx = ((lado - nw) / 2) as f32;
    let dy = ((lado - nh) / 2) as f32;
    let mut t = vec![114.0 / 255.0; 3 * lado * lado];
    let plano = lado * lado;
    for y in 0..nh {
        let sy = ((y as f32 / escala) as usize).min(img.h - 1);
        let oy = y + dy as usize;
        for x in 0..nw {
            let sx = ((x as f32 / escala) as usize).min(img.w - 1);
            let i = (sy * img.w + sx) * 3;
            let o = oy * lado + x + dx as usize;
            t[o] = img.px[i] as f32 / 255.0;
            t[plano + o] = img.px[i + 1] as f32 / 255.0;
            t[2 * plano + o] = img.px[i + 2] as f32 / 255.0;
        }
    }
    (t, Encaixe { escala, dx, dy })
}

/// Detecção em coordenadas normalizadas (0–1) da imagem original.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Deteccao {
    pub classe: usize,
    pub conf: f32,
    pub cx: f32,
    pub cy: f32,
    pub w: f32,
    pub h: f32,
}

impl Deteccao {
    fn caixa(&self) -> (f32, f32, f32, f32) {
        (
            self.cx - self.w / 2.0,
            self.cy - self.h / 2.0,
            self.cx + self.w / 2.0,
            self.cy + self.h / 2.0,
        )
    }
    /// Ponto de apoio (pés): onde a pessoa está no chão.
    pub fn pe(&self) -> Ponto {
        Ponto {
            x: self.cx,
            y: (self.cy + self.h / 2.0).min(1.0),
        }
    }
}

pub fn iou(a: &Deteccao, b: &Deteccao) -> f32 {
    let (ax0, ay0, ax1, ay1) = a.caixa();
    let (bx0, by0, bx1, by1) = b.caixa();
    let iw = (ax1.min(bx1) - ax0.max(bx0)).max(0.0);
    let ih = (ay1.min(by1) - ay0.max(by0)).max(0.0);
    let inter = iw * ih;
    let uni = a.w * a.h + b.w * b.h - inter;
    if uni <= 0.0 {
        0.0
    } else {
        inter / uni
    }
}

/// Lê a saída `[1, 4+nc, N]` do YOLOv8/11 (sem NMS), filtra pelas classes de
/// interesse, aplica NMS e devolve em coordenadas normalizadas da imagem
/// original (`w_img`×`h_img`).
pub fn decodificar(
    saida: &[f32],
    shape: &[usize],
    enc: Encaixe,
    w_img: usize,
    h_img: usize,
    conf_min: f32,
) -> Vec<Deteccao> {
    decodificar_com(saida, shape, enc, w_img, h_img, conf_min, |c| {
        nome_classe(c).map(|_| c)
    })
}

/// Lê a saída de um **modelo de estudo** com `nc` classes próprias (YOLO do
/// Ultralytics, mesma saída `[1, 4+nc, N]`). A classe `i` volta como
/// `CLASSE_ESTUDO + i`.
pub fn decodificar_estudo(
    saida: &[f32],
    shape: &[usize],
    enc: Encaixe,
    w_img: usize,
    h_img: usize,
    conf_min: f32,
    nc: usize,
) -> Vec<Deteccao> {
    if shape.len() == 3 && shape[1] != 4 + nc {
        return Vec::new();
    }
    decodificar_com(saida, shape, enc, w_img, h_img, conf_min, |c| {
        (c < nc).then_some(CLASSE_ESTUDO + c)
    })
}

/// Núcleo da leitura: `mapear` diz se a classe interessa e qual índice usar.
fn decodificar_com(
    saida: &[f32],
    shape: &[usize],
    enc: Encaixe,
    w_img: usize,
    h_img: usize,
    conf_min: f32,
    mapear: impl Fn(usize) -> Option<usize>,
) -> Vec<Deteccao> {
    if shape.len() != 3 || shape[0] != 1 || shape[1] < 5 {
        return Vec::new();
    }
    let (canais, n) = (shape[1], shape[2]);
    let nc = canais - 4;
    if saida.len() < canais * n {
        return Vec::new();
    }
    let mut cand: Vec<Deteccao> = Vec::new();
    for i in 0..n {
        let mut melhor = (0usize, 0f32);
        for c in 0..nc {
            let s = saida[(4 + c) * n + i];
            if s > melhor.1 {
                melhor = (c, s);
            }
        }
        if melhor.1 < conf_min {
            continue;
        }
        let Some(classe) = mapear(melhor.0) else {
            continue;
        };
        let (cx, cy, w, h) = (saida[i], saida[n + i], saida[2 * n + i], saida[3 * n + i]);
        // Desfaz o letterbox e normaliza pela imagem original.
        let cx = ((cx - enc.dx) / enc.escala) / w_img as f32;
        let cy = ((cy - enc.dy) / enc.escala) / h_img as f32;
        let w = (w / enc.escala) / w_img as f32;
        let h = (h / enc.escala) / h_img as f32;
        if !(0.0..=1.0).contains(&cx) || !(0.0..=1.0).contains(&cy) {
            continue;
        }
        cand.push(Deteccao {
            classe,
            conf: melhor.1,
            cx,
            cy,
            w,
            h,
        });
    }
    nms(cand, 0.5)
}

/// Supressão de não máximos por classe.
pub fn nms(mut cand: Vec<Deteccao>, iou_max: f32) -> Vec<Deteccao> {
    cand.sort_by(|a, b| b.conf.total_cmp(&a.conf));
    let mut fica: Vec<Deteccao> = Vec::new();
    for d in cand {
        if fica
            .iter()
            .all(|f| f.classe != d.classe || iou(f, &d) < iou_max)
        {
            fica.push(d);
        }
    }
    fica
}

/// Contagem por classe, para o balão "2 pessoas · 1 carro".
pub fn contar(dets: &[Deteccao]) -> Vec<(usize, usize)> {
    let mut m: Vec<(usize, usize)> = Vec::new();
    for d in dets {
        match m.iter_mut().find(|(c, _)| *c == d.classe) {
            Some((_, n)) => *n += 1,
            None => m.push((d.classe, 1)),
        }
    }
    m.sort_by_key(|(c, _)| *c);
    m
}

// ---------------------------------------------------------------------------
// Rastreamento, contagem, permanência, calor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Trilha {
    pub id: u32,
    pub classe: usize,
    pub pos: Ponto,
    pub anterior: Ponto,
    /// Onde a trilha estava quando parou de andar (para a permanência).
    pub ancora: Ponto,
    pub parada_desde: Instant,
    pub visto_em: Instant,
    pub confirmacoes: u32,
}

/// Eventos do nível médio.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum EventoMedio {
    /// Alguém (pessoa) cruzou a linha da regra.
    LinhaCruzada {
        regra: i64,
        sentido: i8,
        classe: usize,
    },
    /// Zona passou a ter pessoa (ou objeto do modelo de estudo) / ficou vazia.
    ZonaOcupada {
        regra: i64,
        quantas: usize,
        /// Classe de quem ocupou (a primeira trilha confirmada na zona).
        classe: usize,
    },
    ZonaLivre {
        regra: i64,
    },
    /// Pessoa parada dentro da zona por mais de N segundos (uma vez por trilha).
    Permanencia {
        regra: i64,
        segundos: u64,
        /// Classe de quem ficou parado.
        classe: usize,
    },
    /// Densidade: a zona passou do limite de objetos ao mesmo tempo.
    Densidade {
        regra: i64,
        quantas: usize,
        classe: usize,
    },
}

/// Ajustes do nível médio.
#[derive(Debug, Clone)]
pub struct AjustesMedio {
    /// Distância máxima (normalizada) para casar uma detecção com a trilha.
    pub casar_max: f32,
    /// Quanto a trilha pode andar e ainda contar como "parada".
    pub parado_raio: f32,
    /// Quadros sem ver para esquecer a trilha.
    pub esquecer_apos: Duration,
    /// Tempo parado em zona para disparar permanência.
    pub permanencia: Duration,
}

impl Default for AjustesMedio {
    fn default() -> Self {
        Self {
            casar_max: 0.12,
            parado_raio: 0.03,
            esquecer_apos: Duration::from_secs(2),
            permanencia: Duration::from_secs(60),
        }
    }
}

/// Grade de calor acumulada (só para o técnico).
#[derive(Debug, Clone, Serialize)]
pub struct Calor {
    pub colunas: usize,
    pub linhas: usize,
    pub celulas: Vec<u32>,
    pub maximo: u32,
}

impl Calor {
    pub fn new(colunas: usize, linhas: usize) -> Self {
        Self {
            colunas,
            linhas,
            celulas: vec![0; colunas * linhas],
            maximo: 0,
        }
    }
    pub fn marcar(&mut self, p: Ponto) {
        let x = ((p.x * self.colunas as f32) as usize).min(self.colunas - 1);
        let y = ((p.y * self.linhas as f32) as usize).min(self.linhas - 1);
        let c = &mut self.celulas[y * self.colunas + x];
        *c = c.saturating_add(1);
        self.maximo = self.maximo.max(*c);
    }
    pub fn limpar(&mut self) {
        self.celulas.iter_mut().for_each(|c| *c = 0);
        self.maximo = 0;
    }
}

/// Estado do nível médio de uma câmera.
pub struct Rastreador {
    pub ajustes: AjustesMedio,
    trilhas: Vec<Trilha>,
    prox_id: u32,
    zonas: HashMap<i64, bool>,
    permanencias_avisadas: HashMap<(u32, i64), ()>,
    pub calor: Calor,
}

impl Default for Rastreador {
    fn default() -> Self {
        Self::new(AjustesMedio::default())
    }
}

impl Rastreador {
    pub fn new(ajustes: AjustesMedio) -> Self {
        Self {
            ajustes,
            trilhas: Vec::new(),
            prox_id: 1,
            zonas: HashMap::new(),
            permanencias_avisadas: HashMap::new(),
            calor: Calor::new(32, 18),
        }
    }

    pub fn trilhas(&self) -> &[Trilha] {
        &self.trilhas
    }

    pub fn zona_ocupada(&self, regra: i64) -> bool {
        self.zonas.get(&regra).copied().unwrap_or(false)
    }

    /// Processa as detecções de um quadro. `contagem`, `permanencia` e
    /// `calor` ligam cada módulo; `zonas` (pessoas) sempre atualiza.
    pub fn processar(
        &mut self,
        dets: &[Deteccao],
        regras: &[Regra],
        agora: Instant,
        contagem: bool,
        permanencia: bool,
        calor: bool,
    ) -> Vec<EventoMedio> {
        let mut ev = Vec::new();
        self.casar(dets, agora);
        let mut calor_por_regra = false;
        for r in regras {
            let intel = r.ajustes.intel;
            match r.forma {
                Forma::Linha if r.pontos.len() >= 2 => {
                    // Regra com inteligência própria roda sempre que o detector roda;
                    // só a antiga (legado) depende do módulo Contagem.
                    let vale = match intel {
                        Intel::Passagem => true,
                        Intel::Legado => contagem,
                        _ => false,
                    };
                    if !vale {
                        continue;
                    }
                    let (a, b) = (r.pontos[0], r.pontos[1]);
                    for t in self
                        .trilhas
                        .iter()
                        .filter(|t| t.confirmacoes >= 2 && regra_casa(r, t.classe))
                    {
                        if let Some(s) = cruzou(a, b, t.anterior, t.pos) {
                            ev.push(EventoMedio::LinhaCruzada {
                                regra: r.id,
                                sentido: s,
                                classe: t.classe,
                            });
                        }
                    }
                }
                Forma::Zona if r.pontos.len() >= 3 => {
                    let dentro_zona: Vec<&Trilha> = self
                        .trilhas
                        .iter()
                        .filter(|t| {
                            t.confirmacoes >= 2
                                && regra_casa(r, t.classe)
                                && dentro(t.pos, &r.pontos)
                        })
                        .collect();
                    match intel {
                        Intel::Legado | Intel::Presenca => {
                            let ocupada = !dentro_zona.is_empty();
                            let antes = self.zonas.insert(r.id, ocupada).unwrap_or(false);
                            if ocupada && !antes {
                                ev.push(EventoMedio::ZonaOcupada {
                                    regra: r.id,
                                    quantas: dentro_zona.len(),
                                    classe: dentro_zona[0].classe,
                                });
                            } else if !ocupada && antes {
                                ev.push(EventoMedio::ZonaLivre { regra: r.id });
                            }
                        }
                        Intel::Densidade => {
                            let limite = (r.ajustes.parametro as usize).max(2);
                            let alta = dentro_zona.len() >= limite;
                            let antes = self.zonas.insert(r.id, alta).unwrap_or(false);
                            if alta && !antes {
                                ev.push(EventoMedio::Densidade {
                                    regra: r.id,
                                    quantas: dentro_zona.len(),
                                    classe: dentro_zona[0].classe,
                                });
                            } else if !alta && antes {
                                ev.push(EventoMedio::ZonaLivre { regra: r.id });
                            }
                        }
                        Intel::Calor => calor_por_regra = true,
                        Intel::Passagem | Intel::Permanencia => {}
                    }
                    if (permanencia && intel == Intel::Legado) || intel == Intel::Permanencia {
                        let limite = if r.ajustes.parametro > 0 {
                            Duration::from_secs(r.ajustes.parametro as u64)
                        } else {
                            self.ajustes.permanencia
                        };
                        for t in dentro_zona {
                            let parado = agora.saturating_duration_since(t.parada_desde);
                            if parado >= limite
                                && !self.permanencias_avisadas.contains_key(&(t.id, r.id))
                            {
                                self.permanencias_avisadas.insert((t.id, r.id), ());
                                ev.push(EventoMedio::Permanencia {
                                    regra: r.id,
                                    segundos: parado.as_secs(),
                                    classe: t.classe,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if calor || calor_por_regra {
            if calor_por_regra {
                // Mapa de calor só do que passa nas zonas de calor, na classe pedida.
                for r in regras.iter().filter(|r| {
                    r.forma == Forma::Zona && r.ajustes.intel == Intel::Calor && r.pontos.len() >= 3
                }) {
                    for t in self
                        .trilhas
                        .iter()
                        .filter(|t| regra_casa(r, t.classe) && dentro(t.pos, &r.pontos))
                    {
                        self.calor.marcar(t.pos);
                    }
                }
            } else {
                for t in self.trilhas.iter().filter(|t| conta_em_zona(t.classe)) {
                    self.calor.marcar(t.pos);
                }
            }
        }
        // Esquece avisos de trilhas que já sumiram.
        let vivas: Vec<u32> = self.trilhas.iter().map(|t| t.id).collect();
        self.permanencias_avisadas
            .retain(|(id, _), _| vivas.contains(id));
        ev
    }

    /// Casa detecções com trilhas pelo pé mais próximo (mesma classe).
    fn casar(&mut self, dets: &[Deteccao], agora: Instant) {
        let mut usadas = vec![false; dets.len()];
        for t in &mut self.trilhas {
            let mut melhor: Option<(usize, f32)> = None;
            for (i, d) in dets.iter().enumerate() {
                if usadas[i] || d.classe != t.classe {
                    continue;
                }
                let p = d.pe();
                let dist = ((p.x - t.pos.x).powi(2) + (p.y - t.pos.y).powi(2)).sqrt();
                if dist < self.ajustes.casar_max && melhor.is_none_or(|(_, m)| dist < m) {
                    melhor = Some((i, dist));
                }
            }
            if let Some((i, _)) = melhor {
                usadas[i] = true;
                let p = dets[i].pe();
                t.anterior = t.pos;
                t.pos = p;
                t.visto_em = agora;
                t.confirmacoes += 1;
                let andou = ((p.x - t.ancora.x).powi(2) + (p.y - t.ancora.y).powi(2)).sqrt();
                if andou > self.ajustes.parado_raio {
                    t.ancora = p;
                    t.parada_desde = agora;
                }
            } else {
                t.anterior = t.pos;
            }
        }
        let esquecer = self.ajustes.esquecer_apos;
        self.trilhas
            .retain(|t| agora.saturating_duration_since(t.visto_em) < esquecer);
        for (i, d) in dets.iter().enumerate() {
            if !usadas[i] {
                let p = d.pe();
                self.trilhas.push(Trilha {
                    id: self.prox_id,
                    classe: d.classe,
                    pos: p,
                    anterior: p,
                    ancora: p,
                    parada_desde: agora,
                    visto_em: agora,
                    confirmacoes: 1,
                });
                self.prox_id = self.prox_id.wrapping_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pessoa(cx: f32, cy: f32) -> Deteccao {
        Deteccao {
            classe: 0,
            conf: 0.9,
            cx,
            cy,
            w: 0.1,
            h: 0.3,
        }
    }

    #[test]
    fn letterbox_centraliza_e_normaliza() {
        // Imagem 4×2 vermelha: escala 160, 640×320 centrado em y=160.
        let px: Vec<u8> = [255u8, 0, 0].repeat(8);
        let (t, e) = letterbox(&Rgb {
            w: 4,
            h: 2,
            px: &px,
        });
        assert_eq!(t.len(), 3 * LADO * LADO);
        assert_eq!(e.escala, 160.0);
        assert_eq!((e.dx, e.dy), (0.0, 160.0));
        let plano = LADO * LADO;
        // Canto superior: cinza de borda. Centro: vermelho.
        assert!((t[0] - 114.0 / 255.0).abs() < 1e-6);
        let c = 320 * LADO + 320;
        assert_eq!(t[c], 1.0);
        assert_eq!(t[plano + c], 0.0);
    }

    #[test]
    fn decodifica_saida_do_yolo_com_nms() {
        // 84 canais × 3 caixas: duas pessoas quase iguais e um gato (classe 15).
        let n = 3;
        let mut s = vec![0f32; 84 * n];
        let caixa = |s: &mut Vec<f32>,
                     i: usize,
                     cx: f32,
                     cy: f32,
                     w: f32,
                     h: f32,
                     cls: usize,
                     conf: f32| {
            s[i] = cx;
            s[n + i] = cy;
            s[2 * n + i] = w;
            s[3 * n + i] = h;
            s[(4 + cls) * n + i] = conf;
        };
        caixa(&mut s, 0, 320.0, 320.0, 100.0, 200.0, 0, 0.9);
        caixa(&mut s, 1, 325.0, 322.0, 100.0, 200.0, 0, 0.6);
        caixa(&mut s, 2, 100.0, 100.0, 50.0, 50.0, 15, 0.95);
        let enc = Encaixe {
            escala: 1.0,
            dx: 0.0,
            dy: 140.0,
        };
        let d = decodificar(&s, &[1, 84, n], enc, 640, 360, 0.4);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].classe, 0);
        assert!((d[0].cx - 0.5).abs() < 1e-3);
        assert!((d[0].cy - 0.5).abs() < 1e-3);
        assert_eq!(contar(&d), vec![(0, 1)]);
        assert_eq!(plural(0, 2), "2 pessoas");
        assert_eq!(plural(2, 1), "1 carro");
    }

    #[test]
    fn decodifica_modelo_de_estudo_com_classes_proprias() {
        // Modelo de estudo com 2 classes: saída [1, 6, N].
        let n = 2;
        let mut s = vec![0f32; 6 * n];
        // caixa 0: classe 1 (0.8); caixa 1: classe 0 abaixo da confiança.
        s[0] = 320.0;
        s[n] = 320.0;
        s[2 * n] = 64.0;
        s[3 * n] = 64.0;
        s[5 * n] = 0.8;
        s[1] = 100.0;
        s[n + 1] = 100.0;
        s[2 * n + 1] = 10.0;
        s[3 * n + 1] = 10.0;
        s[4 * n + 1] = 0.1;
        let enc = Encaixe {
            escala: 1.0,
            dx: 0.0,
            dy: 0.0,
        };
        let d = decodificar_estudo(&s, &[1, 6, n], enc, 640, 640, 0.35, 2);
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].classe, CLASSE_ESTUDO + 1);
        // Lida como COCO, a classe 1 viraria "bicicleta": por isso o deslocamento.
        assert_eq!(
            decodificar(&s, &[1, 6, n], enc, 640, 640, 0.35)[0].classe,
            1
        );
        // Número de classes diferente do modelo: não inventa detecções.
        assert!(decodificar_estudo(&s, &[1, 6, n], enc, 640, 640, 0.35, 3).is_empty());
        // Saída truncada não derruba o programa.
        assert!(decodificar_estudo(&s[..5], &[1, 6, n], enc, 640, 640, 0.35, 2).is_empty());
        assert!(conta_em_zona(CLASSE_ESTUDO) && conta_em_zona(0) && !conta_em_zona(2));
        let (t, e) = letterbox_lado(
            &Rgb {
                w: 4,
                h: 2,
                px: &[200; 24],
            },
            320,
        );
        assert_eq!(t.len(), 3 * 320 * 320);
        assert!((e.escala - 80.0).abs() < 1e-3);
    }

    #[test]
    fn objeto_de_estudo_ocupa_zona() {
        let mut r = Rastreador::default();
        let regras = vec![Regra {
            id: 9,
            nome: "Doca".into(),
            forma: Forma::Zona,
            ajustes: Default::default(),
            pontos: vec![
                Ponto { x: 0.0, y: 0.0 },
                Ponto { x: 1.0, y: 0.0 },
                Ponto { x: 1.0, y: 1.0 },
                Ponto { x: 0.0, y: 1.0 },
            ],
        }];
        let t0 = Instant::now();
        let mut todos = Vec::new();
        for i in 0..3 {
            let mut d = pessoa(0.5, 0.4);
            d.classe = CLASSE_ESTUDO + 2;
            todos.extend(r.processar(
                &[d],
                &regras,
                t0 + Duration::from_millis(300 * i),
                true,
                false,
                false,
            ));
        }
        assert!(
            todos.contains(&EventoMedio::ZonaOcupada {
                regra: 9,
                quantas: 1,
                classe: CLASSE_ESTUDO + 2
            }),
            "{todos:?}"
        );
    }

    #[test]
    fn conta_passagem_e_ocupa_zona() {
        let mut r = Rastreador::default();
        let regras = vec![
            Regra {
                id: 1,
                nome: "Porta".into(),
                forma: Forma::Linha,
                ajustes: Default::default(),
                pontos: vec![Ponto { x: 0.5, y: 0.0 }, Ponto { x: 0.5, y: 1.0 }],
            },
            Regra {
                id: 2,
                nome: "Balcão".into(),
                forma: Forma::Zona,
                ajustes: Default::default(),
                pontos: vec![
                    Ponto { x: 0.7, y: 0.0 },
                    Ponto { x: 1.0, y: 0.0 },
                    Ponto { x: 1.0, y: 1.0 },
                    Ponto { x: 0.7, y: 1.0 },
                ],
            },
        ];
        let t0 = Instant::now();
        let mut todos = Vec::new();
        for i in 0..10 {
            let x = 0.1 + i as f32 * 0.08;
            let ag = t0 + Duration::from_millis(400 * i);
            todos.extend(r.processar(&[pessoa(x, 0.5)], &regras, ag, true, true, true));
        }
        assert!(
            todos.iter().any(|e| matches!(
                e,
                EventoMedio::LinhaCruzada {
                    regra: 1,
                    classe: 0,
                    ..
                }
            )),
            "{todos:?}"
        );
        assert!(
            todos.iter().any(|e| matches!(
                e,
                EventoMedio::ZonaOcupada {
                    regra: 2,
                    quantas: 1,
                    classe: 0
                }
            )),
            "{todos:?}"
        );
        assert!(r.zona_ocupada(2));
        assert!(r.calor.maximo >= 1);
        // Sumiu: zona libera depois do tempo de esquecer.
        let depois = r.processar(&[], &regras, t0 + Duration::from_secs(30), true, true, true);
        assert!(
            depois.contains(&EventoMedio::ZonaLivre { regra: 2 }),
            "{depois:?}"
        );
    }

    #[test]
    fn permanencia_dispara_uma_vez() {
        let mut r = Rastreador::new(AjustesMedio {
            permanencia: Duration::from_secs(5),
            ..Default::default()
        });
        let zona = vec![Regra {
            id: 9,
            nome: "Caixa".into(),
            forma: Forma::Zona,
            ajustes: Default::default(),
            pontos: vec![
                Ponto { x: 0.0, y: 0.0 },
                Ponto { x: 1.0, y: 0.0 },
                Ponto { x: 1.0, y: 1.0 },
                Ponto { x: 0.0, y: 1.0 },
            ],
        }];
        let t0 = Instant::now();
        let mut ev = Vec::new();
        for i in 0..20 {
            // Pessoa quase parada (oscila 0,5 % da tela).
            let x = 0.5 + if i % 2 == 0 { 0.005 } else { -0.005 };
            ev.extend(r.processar(
                &[pessoa(x, 0.5)],
                &zona,
                t0 + Duration::from_millis(500 * i),
                false,
                true,
                false,
            ));
        }
        let perms: Vec<_> = ev
            .iter()
            .filter(|e| matches!(e, EventoMedio::Permanencia { regra: 9, .. }))
            .collect();
        assert_eq!(perms.len(), 1, "{ev:?}");
        // Pessoa que anda não dispara.
        let mut r2 = Rastreador::new(AjustesMedio {
            permanencia: Duration::from_secs(5),
            ..Default::default()
        });
        let mut ev2 = Vec::new();
        for i in 0..20 {
            ev2.extend(r2.processar(
                &[pessoa(0.1 + 0.04 * i as f32, 0.5)],
                &zona,
                t0 + Duration::from_millis(500 * i),
                false,
                true,
                false,
            ));
        }
        assert!(
            !ev2.iter()
                .any(|e| matches!(e, EventoMedio::Permanencia { .. })),
            "{ev2:?}"
        );
    }

    fn zona_cheia(id: i64, intel: Intel, classes: Vec<usize>, parametro: u32) -> Vec<Regra> {
        vec![Regra {
            id,
            nome: "Z".into(),
            forma: Forma::Zona,
            ajustes: crate::visao::AjustesRegra {
                intel,
                classes,
                parametro,
                ..Default::default()
            },
            pontos: vec![
                Ponto { x: 0.0, y: 0.0 },
                Ponto { x: 1.0, y: 0.0 },
                Ponto { x: 1.0, y: 1.0 },
                Ponto { x: 0.0, y: 1.0 },
            ],
        }]
    }

    fn carro(cx: f32, cy: f32) -> Deteccao {
        Deteccao {
            classe: 2,
            ..pessoa(cx, cy)
        }
    }

    #[test]
    fn linha_so_de_carro_ignora_pessoa() {
        let linha = vec![Regra {
            id: 3,
            nome: "Portão".into(),
            forma: Forma::Linha,
            pontos: vec![Ponto { x: 0.5, y: 0.0 }, Ponto { x: 0.5, y: 1.0 }],
            ajustes: crate::visao::AjustesRegra {
                intel: Intel::Passagem,
                classes: vec![2],
                ..Default::default()
            },
        }];
        let mut r = Rastreador::default();
        let t0 = Instant::now();
        let mut ev = Vec::new();
        for i in 0..12u64 {
            let x = 0.1 + 0.07 * i as f32;
            ev.extend(r.processar(
                &[pessoa(x, 0.5), carro(x, 0.6)],
                &linha,
                t0 + Duration::from_millis(200 * i),
                true,
                false,
                false,
            ));
        }
        let cruzamentos: Vec<_> = ev
            .iter()
            .filter_map(|e| match e {
                EventoMedio::LinhaCruzada { classe, .. } => Some(*classe),
                _ => None,
            })
            .collect();
        assert_eq!(cruzamentos, vec![2], "{ev:?}");
    }

    #[test]
    fn densidade_dispara_com_o_limite_e_so_da_classe() {
        let regras = zona_cheia(5, Intel::Densidade, vec![2], 3);
        let mut r = Rastreador::default();
        let t0 = Instant::now();
        let mut ev = Vec::new();
        for i in 0..4u64 {
            let dets = [
                carro(0.2, 0.5),
                carro(0.5, 0.5),
                pessoa(0.8, 0.5),
                pessoa(0.7, 0.2),
                pessoa(0.6, 0.3),
            ];
            ev.extend(r.processar(
                &dets,
                &regras,
                t0 + Duration::from_millis(300 * i),
                false,
                false,
                false,
            ));
        }
        assert!(
            !ev.iter()
                .any(|e| matches!(e, EventoMedio::Densidade { .. })),
            "2 carros < 3: {ev:?}"
        );
        for i in 4..8u64 {
            let dets = [carro(0.2, 0.5), carro(0.5, 0.5), carro(0.8, 0.5)];
            ev.extend(r.processar(
                &dets,
                &regras,
                t0 + Duration::from_millis(300 * i),
                false,
                false,
                false,
            ));
        }
        let altas: Vec<_> = ev
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    EventoMedio::Densidade {
                        quantas: 3,
                        classe: 2,
                        ..
                    }
                )
            })
            .collect();
        assert_eq!(altas.len(), 1, "{ev:?}");
    }

    #[test]
    fn permanencia_so_do_carro_e_sem_aviso_de_entrada() {
        let regras = zona_cheia(6, Intel::Permanencia, vec![2], 5);
        let mut r = Rastreador::default();
        let t0 = Instant::now();
        let mut ev = Vec::new();
        for i in 0..20u64 {
            ev.extend(r.processar(
                &[carro(0.5, 0.5), pessoa(0.2, 0.5)],
                &regras,
                t0 + Duration::from_millis(500 * i),
                false,
                true,
                false,
            ));
        }
        let perms: Vec<_> = ev
            .iter()
            .filter(|e| matches!(e, EventoMedio::Permanencia { classe: 2, .. }))
            .collect();
        assert_eq!(perms.len(), 1, "{ev:?}");
        assert!(
            !ev.iter().any(|e| matches!(
                e,
                EventoMedio::ZonaOcupada { .. } | EventoMedio::Permanencia { classe: 0, .. }
            )),
            "uma inteligência por regra: {ev:?}"
        );
    }

    #[test]
    fn calor_so_conta_a_classe_na_zona() {
        let regras = zona_cheia(7, Intel::Calor, vec![0], 0);
        let mut r = Rastreador::default();
        let t0 = Instant::now();
        for i in 0..4u64 {
            r.processar(
                &[pessoa(0.3, 0.5), carro(0.8, 0.5)],
                &regras,
                t0 + Duration::from_millis(200 * i),
                false,
                false,
                true,
            );
        }
        let total: u32 = r.calor.celulas.iter().sum();
        assert!(total >= 2, "pessoa marcou o calor: {total}");
        let x_carro = (0.8 * r.calor.colunas as f32) as usize;
        let y = (0.5 * r.calor.linhas as f32) as usize;
        let na_celula_do_carro = r.calor.celulas[y * r.calor.colunas + x_carro];
        assert_eq!(na_celula_do_carro, 0, "carro não conta no calor de pessoa");
    }
}
