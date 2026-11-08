//! Visão computacional leve (nível "leve" do SDD-02): movimento inteligente e
//! cerca virtual (zonas e linhas), sem rede neural. A saúde da câmera usa as
//! métricas de `saude()` daqui, com a máquina de estados em `crate::saude`.
//!
//! Tudo trabalha em quadros cinza pequenos (ex.: 160×90). Coordenadas de regras
//! são normalizadas (0–1), independentes da resolução da câmera.

use serde::{Deserialize, Serialize};

/// Quadro em tons de cinza.
#[derive(Clone)]
pub struct Quadro {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

impl Quadro {
    /// Reduz por média em blocos até caber em `max_w` de largura.
    pub fn reduzir(w: usize, h: usize, px: &[u8], max_w: usize) -> Quadro {
        let f = w.div_ceil(max_w).max(1);
        let (nw, nh) = (w / f, h / f);
        let mut out = vec![0u8; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let mut s = 0u32;
                for dy in 0..f {
                    let row = (y * f + dy) * w;
                    for dx in 0..f {
                        s += px[row + x * f + dx] as u32;
                    }
                }
                out[y * nw + x] = (s / (f * f) as u32) as u8;
            }
        }
        Quadro {
            w: nw,
            h: nh,
            px: out,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ponto {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Forma {
    Zona,
    Linha,
}

/// O que a regra faz (uma inteligência por regra, SDD-09).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Intel {
    /// Regra criada antes do SDD-09: linha conta passagem; zona avisa quem
    /// entra e, com o módulo ligado, também a permanência. Só pessoa (zona).
    #[default]
    Legado,
    /// Linha: conta quem cruza, com sentido.
    Passagem,
    /// Zona: avisa quando alguém entra e quando sai (cerca).
    Presenca,
    /// Zona: avisa quando alguém fica parado por N segundos.
    Permanencia,
    /// Zona: avisa quando há N ou mais dentro ao mesmo tempo.
    Densidade,
    /// Zona: acumula o mapa de calor só do que passa nela.
    Calor,
}

impl Intel {
    pub fn de_texto(s: &str) -> Option<Self> {
        Some(match s {
            "passagem" => Self::Passagem,
            "presenca" => Self::Presenca,
            "permanencia" => Self::Permanencia,
            "densidade" => Self::Densidade,
            "calor" => Self::Calor,
            "" | "legado" => Self::Legado,
            _ => return None,
        })
    }
}

fn verdade() -> bool {
    true
}

impl Default for AjustesRegra {
    fn default() -> Self {
        Self {
            ativa: true,
            intel: Intel::default(),
            classes: Vec::new(),
            parametro: 0,
            cor: String::new(),
            som: String::new(),
        }
    }
}

/// Ajustes de uma regra além do desenho: inteligência, classes, cor e som.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AjustesRegra {
    /// Regra ligada. Desligada fica salva, mas não detecta nem alerta.
    #[serde(default = "verdade")]
    pub ativa: bool,
    #[serde(default)]
    pub intel: Intel,
    /// Classes que a regra observa (índices do COCO ou de um modelo de
    /// estudo). Vazio = o padrão da inteligência legada.
    #[serde(default)]
    pub classes: Vec<usize>,
    /// Densidade: quantos ao mesmo tempo. Permanência: segundos (0 = padrão
    /// global). Passagem/presença/calor: não usa.
    #[serde(default)]
    pub parametro: u32,
    /// Cor da linha/zona no overlay (nome do token: verde, ambar…).
    #[serde(default)]
    pub cor: String,
    /// Som do alerta (bipe, sino…); vazio = padrão.
    #[serde(default)]
    pub som: String,
}

/// Regra desenhada pelo técnico.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regra {
    pub id: i64,
    pub nome: String,
    pub forma: Forma,
    pub pontos: Vec<Ponto>,
    #[serde(default)]
    pub ajustes: AjustesRegra,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Evento {
    Movimento,
    ZonaOcupada { regra: i64 },
    ZonaLivre { regra: i64 },
    LinhaCruzada { regra: i64, sentido: i8 },
}

#[derive(Debug, Clone, Copy)]
pub struct Mancha {
    pub cx: f32,
    pub cy: f32,
    pub area: usize,
}

/// Parâmetros ajustáveis (valores para cena de loja a 5 quadros/s).
#[derive(Debug, Clone)]
pub struct Ajustes {
    /// Diferença mínima de brilho (0–255) para um pixel contar como movimento.
    pub limiar: u8,
    /// Área mínima de uma mancha, em fração do quadro (filtra folha, chuva, ruído).
    pub area_min: f32,
    /// Quadros seguidos com movimento para confirmar (filtra piscadas).
    pub persistencia: u32,
    /// Fração da zona com movimento para considerá-la ocupada.
    pub ocupacao: f32,
    /// Quadros sem movimento para liberar a zona.
    pub liberar_apos: u32,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            limiar: 24,
            area_min: 0.004,
            persistencia: 2,
            ocupacao: 0.03,
            liberar_apos: 10,
        }
    }
}

struct Trilha {
    id: u32,
    x: f32,
    y: f32,
    visto: u32,
}

#[derive(Default, Clone)]
struct EstadoZona {
    ocupada: bool,
    acima: u32,
    abaixo: u32,
}

/// Analisador de uma câmera.
pub struct Analisador {
    pub ajustes: Ajustes,
    fundo: Vec<u16>,
    anterior: Option<Quadro>,
    trilhas: Vec<Trilha>,
    prox_id: u32,
    zonas: std::collections::HashMap<i64, EstadoZona>,
    movimento_seguido: u32,
}

impl Default for Analisador {
    fn default() -> Self {
        Self::new(Ajustes::default())
    }
}

impl Analisador {
    pub fn new(ajustes: Ajustes) -> Self {
        Self {
            ajustes,
            fundo: Vec::new(),
            anterior: None,
            trilhas: Vec::new(),
            prox_id: 1,
            zonas: Default::default(),
            movimento_seguido: 0,
        }
    }

    /// Centros das trilhas confirmadas, normalizados 0..1 (para a sugestão).
    pub fn centros(&self) -> Vec<(u32, f32, f32)> {
        let Some(q) = &self.anterior else {
            return Vec::new();
        };
        let (w, h) = (q.w.max(1) as f32, q.h.max(1) as f32);
        self.trilhas
            .iter()
            .filter(|t| t.visto >= 2)
            .map(|t| (t.id, t.x / w, t.y / h))
            .collect()
    }

    /// Estado atual de uma zona (para o balão).
    pub fn zona_ocupada(&self, regra: i64) -> bool {
        self.zonas.get(&regra).is_some_and(|z| z.ocupada)
    }

    /// Processa um quadro e devolve os eventos novos.
    pub fn processar(&mut self, q: &Quadro, regras: &[Regra], movimento: bool) -> Vec<Evento> {
        let mut ev = Vec::new();
        let n = q.w * q.h;
        if self.fundo.len() != n {
            self.fundo = q.px.iter().map(|&p| (p as u16) << 4).collect();
            self.anterior = Some(q.clone());
            return ev;
        }

        // Máscara de movimento contra o fundo (média móvel lenta).
        let lim = self.ajustes.limiar as i32;
        let mut mask = vec![false; n];
        for ((m, f), &p) in mask.iter_mut().zip(self.fundo.iter_mut()).zip(&q.px) {
            let d = (p as i32 - (*f >> 4) as i32).abs();
            *m = d > lim;
            // Atualiza o fundo: rápido onde está parado, devagar onde mexe.
            let alvo = (p as u16) << 4;
            let passo = if *m { 64 } else { 8 };
            *f = ((*f as u32 * (passo - 1) + alvo as u32) / passo) as u16;
        }
        let mask = abrir(&mask, q.w, q.h);

        let area_min = ((self.ajustes.area_min * n as f32) as usize).max(3);
        let manchas = manchas(&mask, q.w, q.h, area_min);

        // Movimento inteligente: só confirma após alguns quadros seguidos.
        if manchas.is_empty() {
            self.movimento_seguido = 0;
        } else {
            self.movimento_seguido += 1;
            if movimento && self.movimento_seguido == self.ajustes.persistencia {
                ev.push(Evento::Movimento);
            }
        }

        // Rastreamento simples pelo centro mais próximo.
        let antigas: Vec<(u32, f32, f32)> = self.trilhas.iter().map(|t| (t.id, t.x, t.y)).collect();
        self.atualizar_trilhas(&manchas);

        for r in regras {
            match r.forma {
                Forma::Zona if r.pontos.len() >= 3 => {
                    let (dentro, total) = fracao_na_zona(&mask, q.w, q.h, &r.pontos);
                    let ocupada_agora =
                        total > 0 && dentro as f32 / total as f32 >= self.ajustes.ocupacao;
                    let z = self.zonas.entry(r.id).or_default();
                    if ocupada_agora {
                        z.acima += 1;
                        z.abaixo = 0;
                        if !z.ocupada && z.acima >= self.ajustes.persistencia {
                            z.ocupada = true;
                            ev.push(Evento::ZonaOcupada { regra: r.id });
                        }
                    } else {
                        z.abaixo += 1;
                        z.acima = 0;
                        if z.ocupada && z.abaixo >= self.ajustes.liberar_apos {
                            z.ocupada = false;
                            ev.push(Evento::ZonaLivre { regra: r.id });
                        }
                    }
                }
                Forma::Linha if r.pontos.len() >= 2 => {
                    let (a, b) = (r.pontos[0], r.pontos[1]);
                    for t in &self.trilhas {
                        if t.visto < 2 {
                            continue;
                        }
                        if let Some(&(_, ox, oy)) = antigas.iter().find(|(id, _, _)| *id == t.id) {
                            let p0 = Ponto {
                                x: ox / q.w as f32,
                                y: oy / q.h as f32,
                            };
                            let p1 = Ponto {
                                x: t.x / q.w as f32,
                                y: t.y / q.h as f32,
                            };
                            if let Some(s) = cruzou(a, b, p0, p1) {
                                ev.push(Evento::LinhaCruzada {
                                    regra: r.id,
                                    sentido: s,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        self.anterior = Some(q.clone());
        ev
    }

    fn atualizar_trilhas(&mut self, manchas: &[Mancha]) {
        let mut usadas = vec![false; manchas.len()];
        let max_d2 = 20.0f32 * 20.0;
        for t in &mut self.trilhas {
            let mut melhor: Option<(usize, f32)> = None;
            for (i, m) in manchas.iter().enumerate() {
                if usadas[i] {
                    continue;
                }
                let d2 = (m.cx - t.x).powi(2) + (m.cy - t.y).powi(2);
                if d2 < max_d2 && melhor.is_none_or(|(_, b)| d2 < b) {
                    melhor = Some((i, d2));
                }
            }
            if let Some((i, _)) = melhor {
                usadas[i] = true;
                t.x = manchas[i].cx;
                t.y = manchas[i].cy;
                t.visto += 1;
            } else {
                t.visto = 0;
            }
        }
        self.trilhas.retain(|t| t.visto > 0);
        for (i, m) in manchas.iter().enumerate() {
            if !usadas[i] {
                self.trilhas.push(Trilha {
                    id: self.prox_id,
                    x: m.cx,
                    y: m.cy,
                    visto: 1,
                });
                self.prox_id = self.prox_id.wrapping_add(1);
            }
        }
    }
}

/// Erosão seguida de dilatação 3×3 (remove pontos isolados).
fn abrir(m: &[bool], w: usize, h: usize) -> Vec<bool> {
    let viz = |m: &[bool], x: usize, y: usize, todos: bool| {
        let mut r = todos;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                let v = nx >= 0
                    && ny >= 0
                    && (nx as usize) < w
                    && (ny as usize) < h
                    && m[ny as usize * w + nx as usize];
                if todos {
                    r &= v;
                } else {
                    r |= v;
                }
            }
        }
        r
    };
    let mut e = vec![false; m.len()];
    for y in 0..h {
        for x in 0..w {
            e[y * w + x] = m[y * w + x] && viz(m, x, y, true);
        }
    }
    let mut d = vec![false; m.len()];
    for y in 0..h {
        for x in 0..w {
            d[y * w + x] = viz(&e, x, y, false);
        }
    }
    d
}

/// Componentes conectados (4-vizinhança) com área mínima.
pub fn manchas(mask: &[bool], w: usize, h: usize, area_min: usize) -> Vec<Mancha> {
    let mut visto = vec![false; mask.len()];
    let mut out = Vec::new();
    let mut pilha = Vec::new();
    for i in 0..mask.len() {
        if !mask[i] || visto[i] {
            continue;
        }
        let (mut sx, mut sy, mut n) = (0usize, 0usize, 0usize);
        visto[i] = true;
        pilha.push(i);
        while let Some(j) = pilha.pop() {
            let (x, y) = (j % w, j / w);
            sx += x;
            sy += y;
            n += 1;
            let mut tenta = |k: usize| {
                if mask[k] && !visto[k] {
                    visto[k] = true;
                    pilha.push(k);
                }
            };
            if x > 0 {
                tenta(j - 1);
            }
            if x + 1 < w {
                tenta(j + 1);
            }
            if y > 0 {
                tenta(j - w);
            }
            if y + 1 < h {
                tenta(j + w);
            }
        }
        if n >= area_min {
            out.push(Mancha {
                cx: sx as f32 / n as f32,
                cy: sy as f32 / n as f32,
                area: n,
            });
        }
    }
    out
}

/// Ponto dentro de polígono (coordenadas normalizadas).
pub fn dentro(p: Ponto, poli: &[Ponto]) -> bool {
    let mut c = false;
    let n = poli.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poli[i], poli[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            c = !c;
        }
        j = i;
    }
    c
}

fn fracao_na_zona(mask: &[bool], w: usize, h: usize, poli: &[Ponto]) -> (usize, usize) {
    let (mut minx, mut miny, mut maxx, mut maxy) = (1f32, 1f32, 0f32, 0f32);
    for p in poli {
        minx = minx.min(p.x);
        miny = miny.min(p.y);
        maxx = maxx.max(p.x);
        maxy = maxy.max(p.y);
    }
    let x0 = (minx.clamp(0.0, 1.0) * w as f32) as usize;
    let x1 = ((maxx.clamp(0.0, 1.0) * w as f32) as usize).min(w);
    let y0 = (miny.clamp(0.0, 1.0) * h as f32) as usize;
    let y1 = ((maxy.clamp(0.0, 1.0) * h as f32) as usize).min(h);
    let (mut dentro_mov, mut total) = (0, 0);
    for y in y0..y1 {
        for x in x0..x1 {
            let p = Ponto {
                x: (x as f32 + 0.5) / w as f32,
                y: (y as f32 + 0.5) / h as f32,
            };
            if dentro(p, poli) {
                total += 1;
                if mask[y * w + x] {
                    dentro_mov += 1;
                }
            }
        }
    }
    (dentro_mov, total)
}

fn lado(a: Ponto, b: Ponto, p: Ponto) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

/// Se o deslocamento p0→p1 atravessou o segmento a–b, devolve o sentido (+1/−1).
pub fn cruzou(a: Ponto, b: Ponto, p0: Ponto, p1: Ponto) -> Option<i8> {
    let (s0, s1) = (lado(a, b, p0), lado(a, b, p1));
    if s0 == 0.0 || s1 == 0.0 || (s0 > 0.0) == (s1 > 0.0) {
        return None;
    }
    // O cruzamento precisa cair dentro do segmento (não na reta infinita).
    let (t0, t1) = (lado(p0, p1, a), lado(p0, p1, b));
    if (t0 > 0.0) == (t1 > 0.0) {
        return None;
    }
    Some(if s0 < 0.0 { 1 } else { -1 })
}

pub struct Saude {
    pub media: f32,
    pub desvio: f32,
    pub nitidez: f32,
}

/// Brilho médio, contraste (desvio padrão) e nitidez (variância do Laplaciano).
pub fn saude(q: &Quadro) -> Saude {
    let n = (q.w * q.h) as f32;
    let media = q.px.iter().map(|&p| p as f32).sum::<f32>() / n;
    let desvio = (q
        .px
        .iter()
        .map(|&p| (p as f32 - media).powi(2))
        .sum::<f32>()
        / n)
        .sqrt();
    let mut lap = Vec::with_capacity(q.w * q.h);
    for y in 1..q.h.saturating_sub(1) {
        for x in 1..q.w.saturating_sub(1) {
            let c = q.px[y * q.w + x] as f32;
            let l = q.px[y * q.w + x - 1] as f32
                + q.px[y * q.w + x + 1] as f32
                + q.px[(y - 1) * q.w + x] as f32
                + q.px[(y + 1) * q.w + x] as f32
                - 4.0 * c;
            lap.push(l);
        }
    }
    let lm = lap.iter().sum::<f32>() / lap.len().max(1) as f32;
    let nitidez = lap.iter().map(|l| (l - lm).powi(2)).sum::<f32>() / lap.len().max(1) as f32;
    Saude {
        media,
        desvio,
        nitidez: nitidez.sqrt(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 160;
    const H: usize = 90;

    /// Cena com textura e uma "pessoa" (retângulo claro) em x.
    fn cena(px_pessoa: Option<usize>) -> Quadro {
        let mut px = vec![0u8; W * H];
        for y in 0..H {
            for x in 0..W {
                px[y * W + x] = 40 + ((x * 7 + y * 13) % 23) as u8;
            }
        }
        if let Some(cx) = px_pessoa {
            for y in 30..75 {
                for x in cx.saturating_sub(6)..(cx + 6).min(W) {
                    px[y * W + x] = 210;
                }
            }
        }
        Quadro { w: W, h: H, px }
    }

    fn p(x: f32, y: f32) -> Ponto {
        Ponto { x, y }
    }

    #[test]
    fn reduz_por_media() {
        let q = Quadro::reduzir(4, 2, &[0, 10, 20, 30, 0, 10, 20, 30], 2);
        assert_eq!((q.w, q.h), (2, 1));
        assert_eq!(q.px, vec![5, 25]);
    }

    #[test]
    fn poligono_e_cruzamento() {
        let sq = [p(0.2, 0.2), p(0.8, 0.2), p(0.8, 0.8), p(0.2, 0.8)];
        assert!(dentro(p(0.5, 0.5), &sq));
        assert!(!dentro(p(0.9, 0.5), &sq));
        let (a, b) = (p(0.5, 0.1), p(0.5, 0.9));
        assert_eq!(cruzou(a, b, p(0.4, 0.5), p(0.6, 0.5)), Some(-1));
        assert_eq!(cruzou(a, b, p(0.6, 0.5), p(0.4, 0.5)), Some(1));
        assert_eq!(
            cruzou(a, b, p(0.4, 0.95), p(0.6, 0.95)),
            None,
            "fora do segmento"
        );
        assert_eq!(cruzou(a, b, p(0.1, 0.5), p(0.3, 0.5)), None);
    }

    #[test]
    fn cena_parada_nao_gera_evento() {
        let mut a = Analisador::default();
        for _ in 0..30 {
            assert!(a.processar(&cena(None), &[], true).is_empty());
        }
    }

    #[test]
    fn pessoa_atravessa_linha_e_ocupa_zona() {
        let mut a = Analisador::default();
        let regras = vec![
            Regra {
                id: 1,
                nome: "Porta".into(),
                forma: Forma::Linha,
                ajustes: Default::default(),
                pontos: vec![p(0.5, 0.1), p(0.5, 0.95)],
            },
            Regra {
                id: 2,
                nome: "Balcão".into(),
                forma: Forma::Zona,
                ajustes: Default::default(),
                pontos: vec![p(0.7, 0.2), p(0.95, 0.2), p(0.95, 0.9), p(0.7, 0.9)],
            },
        ];
        for _ in 0..5 {
            a.processar(&cena(None), &regras, true);
        }
        let mut todos = Vec::new();
        for x in (10..140).step_by(5) {
            todos.extend(a.processar(&cena(Some(x)), &regras, true));
        }
        assert!(todos.contains(&Evento::Movimento));
        assert!(
            todos
                .iter()
                .any(|e| matches!(e, Evento::LinhaCruzada { regra: 1, .. })),
            "{todos:?}"
        );
        assert!(
            todos.contains(&Evento::ZonaOcupada { regra: 2 }),
            "{todos:?}"
        );
        assert!(a.zona_ocupada(2));
        // Pessoa sai de cena: a zona libera depois de alguns quadros.
        let mut depois = Vec::new();
        for _ in 0..20 {
            depois.extend(a.processar(&cena(None), &regras, true));
        }
        assert!(
            depois.contains(&Evento::ZonaLivre { regra: 2 }),
            "{depois:?}"
        );
    }

    #[test]
    fn ruido_pequeno_e_ignorado() {
        let mut a = Analisador::default();
        for _ in 0..5 {
            a.processar(&cena(None), &[], true);
        }
        // Um pixel piscando (folha, chuva, ruído) não vira movimento.
        for i in 0..10 {
            let mut q = cena(None);
            q.px[(40 + i) * W + 80] = 255;
            assert!(a.processar(&q, &[], true).is_empty());
        }
    }

    #[test]
    fn saude_detecta_tapada_e_escura() {
        let escura = Quadro {
            w: W,
            h: H,
            px: vec![5; W * H],
        };
        let tapada = Quadro {
            w: W,
            h: H,
            px: vec![120; W * H],
        };
        assert!(saude(&escura).media < 18.0);
        assert!(saude(&tapada).desvio < 6.0);
        assert!(saude(&cena(None)).nitidez > 12.0);
    }
}
