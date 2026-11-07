//! Nível médio (SDD-02): tudo que cerca o detector de objetos, sem a rede.
//!
//! Pré-processamento (letterbox RGB → tensor), leitura da saída do YOLO
//! (84×N), NMS, rastreamento simples por proximidade, contagem por linha,
//! permanência em zona e mapa de calor. O ONNX Runtime em si fica no binário
//! (`camfacil::ia`); aqui é só matemática testável.

use crate::visao::{cruzou, dentro, Forma, Ponto, Regra};
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
    let escala = (LADO as f32 / img.w as f32).min(LADO as f32 / img.h as f32);
    let nw = ((img.w as f32 * escala).round() as usize).clamp(1, LADO);
    let nh = ((img.h as f32 * escala).round() as usize).clamp(1, LADO);
    let dx = ((LADO - nw) / 2) as f32;
    let dy = ((LADO - nh) / 2) as f32;
    let mut t = vec![114.0 / 255.0; 3 * LADO * LADO];
    let plano = LADO * LADO;
    for y in 0..nh {
        let sy = ((y as f32 / escala) as usize).min(img.h - 1);
        let oy = y + dy as usize;
        for x in 0..nw {
            let sx = ((x as f32 / escala) as usize).min(img.w - 1);
            let i = (sy * img.w + sx) * 3;
            let o = oy * LADO + x + dx as usize;
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
    if shape.len() != 3 || shape[0] != 1 || shape[1] < 5 {
        return Vec::new();
    }
    let (canais, n) = (shape[1], shape[2]);
    let nc = canais - 4;
    let mut cand: Vec<Deteccao> = Vec::new();
    for i in 0..n {
        let mut melhor = (0usize, 0f32);
        for c in 0..nc {
            let s = saida[(4 + c) * n + i];
            if s > melhor.1 {
                melhor = (c, s);
            }
        }
        if melhor.1 < conf_min || nome_classe(melhor.0).is_none() {
            continue;
        }
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
            classe: melhor.0,
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
    /// Zona passou a ter pessoa / ficou vazia.
    ZonaOcupada {
        regra: i64,
        quantas: usize,
    },
    ZonaLivre {
        regra: i64,
    },
    /// Pessoa parada dentro da zona por mais de N segundos (uma vez por trilha).
    Permanencia {
        regra: i64,
        segundos: u64,
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
        let pessoas: Vec<&Trilha> = self.trilhas.iter().filter(|t| t.classe == 0).collect();

        for r in regras {
            match r.forma {
                Forma::Linha if contagem && r.pontos.len() >= 2 => {
                    let (a, b) = (r.pontos[0], r.pontos[1]);
                    for t in self.trilhas.iter().filter(|t| t.confirmacoes >= 2) {
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
                    let dentro_zona: Vec<&&Trilha> = pessoas
                        .iter()
                        .filter(|t| t.confirmacoes >= 2 && dentro(t.pos, &r.pontos))
                        .collect();
                    let ocupada = !dentro_zona.is_empty();
                    let antes = self.zonas.insert(r.id, ocupada).unwrap_or(false);
                    if ocupada && !antes {
                        ev.push(EventoMedio::ZonaOcupada {
                            regra: r.id,
                            quantas: dentro_zona.len(),
                        });
                    } else if !ocupada && antes {
                        ev.push(EventoMedio::ZonaLivre { regra: r.id });
                    }
                    if permanencia {
                        for t in dentro_zona {
                            let parado = agora.saturating_duration_since(t.parada_desde);
                            if parado >= self.ajustes.permanencia
                                && !self.permanencias_avisadas.contains_key(&(t.id, r.id))
                            {
                                self.permanencias_avisadas.insert((t.id, r.id), ());
                                ev.push(EventoMedio::Permanencia {
                                    regra: r.id,
                                    segundos: parado.as_secs(),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if calor {
            for t in self.trilhas.iter().filter(|t| t.classe == 0) {
                self.calor.marcar(t.pos);
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
    fn conta_passagem_e_ocupa_zona() {
        let mut r = Rastreador::default();
        let regras = vec![
            Regra {
                id: 1,
                nome: "Porta".into(),
                forma: Forma::Linha,
                pontos: vec![Ponto { x: 0.5, y: 0.0 }, Ponto { x: 0.5, y: 1.0 }],
            },
            Regra {
                id: 2,
                nome: "Balcão".into(),
                forma: Forma::Zona,
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
                    quantas: 1
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
}
