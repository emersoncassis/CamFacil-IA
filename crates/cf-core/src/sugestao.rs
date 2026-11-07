//! "IA, sugira para mim": observa a câmera por um tempo, aprende o fluxo de
//! pessoas e propõe regras já desenhadas. O técnico só aceita.
//!
//! Sem rede neural: usa as trilhas (centros) que o analisador já calcula.
//! - Passagem: direção dominante do movimento (eixo principal dos vetores) →
//!   linha perpendicular a ela, no meio do corredor, longe das bordas.
//! - Permanência: células onde alguém ficou quase parado por bastante tempo →
//!   zona em volta desse ponto.
//!
//! Custo: uma grade 24×14 de contadores e um ponto por trilha. Desprezível.

use crate::visao::{Forma, Ponto};
use serde::Serialize;
use std::collections::HashMap;

const GX: usize = 24;
const GY: usize = 14;
/// Margem das bordas: linha e zona nunca encostam na borda da imagem.
const MARGEM: f32 = 0.12;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Sugestao {
    pub forma: Forma,
    pub nome: String,
    pub pontos: Vec<Ponto>,
    /// Por que a IA sugeriu (mostrado ao técnico).
    pub motivo: String,
    /// 0 a 1.
    pub confianca: f32,
}

#[derive(Default)]
pub struct Observador {
    /// Última posição de cada trilha (normalizada 0..1).
    ultimo: HashMap<u32, (f32, f32)>,
    /// Quadros parados seguidos por trilha.
    parado: HashMap<u32, u32>,
    passagem: Vec<u32>,
    permanencia: Vec<u32>,
    // Momentos de segunda ordem dos vetores de movimento.
    sxx: f64,
    syy: f64,
    sxy: f64,
    /// Centros em movimento (para posicionar a linha).
    pontos: Vec<(f32, f32)>,
    trilhas: std::collections::HashSet<u32>,
    pub quadros: u64,
}

impl Observador {
    pub fn new() -> Self {
        Self {
            passagem: vec![0; GX * GY],
            permanencia: vec![0; GX * GY],
            ..Default::default()
        }
    }

    /// Alimenta com as trilhas do quadro atual: (id, x, y) normalizados.
    pub fn alimentar(&mut self, trilhas: &[(u32, f32, f32)]) {
        self.quadros += 1;
        for &(id, x, y) in trilhas {
            let c = celula(x, y);
            if let Some(&(ox, oy)) = self.ultimo.get(&id) {
                let (dx, dy) = (x - ox, y - oy);
                let passo = (dx * dx + dy * dy).sqrt();
                if passo > 0.004 {
                    self.passagem[c] += 1;
                    self.sxx += (dx * dx) as f64;
                    self.syy += (dy * dy) as f64;
                    self.sxy += (dx * dy) as f64;
                    if self.pontos.len() < 20_000 {
                        self.pontos.push((x, y));
                    }
                    self.parado.insert(id, 0);
                    self.trilhas.insert(id);
                } else {
                    let p = self.parado.entry(id).or_default();
                    *p += 1;
                    // Parado por mais de ~2 s (a 5 q/s) conta como permanência.
                    if *p > 10 {
                        self.permanencia[c] += 1;
                    }
                }
            }
            self.ultimo.insert(id, (x, y));
        }
        let vivos: std::collections::HashSet<u32> = trilhas.iter().map(|t| t.0).collect();
        self.ultimo.retain(|k, _| vivos.contains(k));
        self.parado.retain(|k, _| vivos.contains(k));
    }

    /// Quantas pessoas (trilhas que se moveram) já foram vistas.
    pub fn pessoas(&self) -> usize {
        self.trilhas.len()
    }

    pub fn sugerir(&self) -> Vec<Sugestao> {
        let mut out = Vec::new();
        if let Some(s) = self.linha() {
            out.push(s);
        }
        if let Some(s) = self.zona() {
            out.push(s);
        }
        out
    }

    fn linha(&self) -> Option<Sugestao> {
        if self.pontos.len() < 15 || self.trilhas.len() < 2 {
            return None;
        }
        // Direção dominante: eixo principal dos vetores de movimento.
        let ang = 0.5 * (2.0 * self.sxy).atan2(self.sxx - self.syy);
        let (ux, uy) = (ang.cos() as f32, ang.sin() as f32); // direção do fluxo
        let (px, py) = (-uy, ux); // perpendicular: a linha
        let n = self.pontos.len() as f32;
        let cx = self.pontos.iter().map(|p| p.0).sum::<f32>() / n;
        let cy = self.pontos.iter().map(|p| p.1).sum::<f32>() / n;
        let cx = cx.clamp(MARGEM + 0.05, 1.0 - MARGEM - 0.05);
        let cy = cy.clamp(MARGEM + 0.05, 1.0 - MARGEM - 0.05);
        // Largura do corredor: espalhamento dos pontos na direção da linha.
        let mut lado: Vec<f32> = self
            .pontos
            .iter()
            .map(|p| (p.0 - cx) * px + (p.1 - cy) * py)
            .collect();
        lado.sort_by(|a, b| a.total_cmp(b));
        let q = |f: f32| lado[((lado.len() - 1) as f32 * f) as usize];
        let (a, b) = (q(0.05) - 0.06, q(0.95) + 0.06);
        // Pelo menos 30% da imagem: a linha precisa pegar o corpo inteiro.
        let (meio, metade) = ((a + b) / 2.0, ((b - a) / 2.0).max(0.15));
        let (a, b) = (meio - metade, meio + metade);
        let ponto = |t: f32| Ponto {
            x: (cx + px * t).clamp(MARGEM, 1.0 - MARGEM),
            y: (cy + py * t).clamp(MARGEM, 1.0 - MARGEM),
        };
        let (p0, p1) = (ponto(a), ponto(b));
        if ((p1.x - p0.x).powi(2) + (p1.y - p0.y).powi(2)).sqrt() < 0.12 {
            return None;
        }
        // Concentração: quanto do movimento segue a direção dominante.
        let traco = (self.sxx + self.syy).max(1e-9);
        let disc = ((self.sxx - self.syy).powi(2) + 4.0 * self.sxy.powi(2)).sqrt();
        let foco = (disc / traco) as f32;
        let horizontal = ux.abs() > uy.abs();
        Some(Sugestao {
            forma: Forma::Linha,
            nome: "Passagem".into(),
            pontos: vec![p0, p1],
            motivo: format!(
                "{} pessoas passaram, a maioria {}. A linha fica atravessando esse caminho, longe das bordas, para contar quem passa.",
                self.trilhas.len(),
                if horizontal { "de um lado para o outro" } else { "indo e vindo (para perto e para longe)" }
            ),
            confianca: (foco * (self.trilhas.len() as f32 / 10.0).min(1.0)).clamp(0.0, 1.0),
        })
    }

    fn zona(&self) -> Option<Sugestao> {
        let max = *self.permanencia.iter().max()?;
        if max < 15 {
            return None;
        }
        // Células com permanência relevante, ligadas à mais forte.
        let corte = (max as f32 * 0.25).max(5.0) as u32;
        let inicio = self.permanencia.iter().position(|&v| v == max)?;
        let mut fila = vec![inicio];
        let mut visto = vec![false; GX * GY];
        visto[inicio] = true;
        let (mut x0, mut y0, mut x1, mut y1) = (GX, GY, 0, 0);
        let mut soma = 0u32;
        while let Some(c) = fila.pop() {
            let (x, y) = (c % GX, c / GX);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            soma += self.permanencia[c];
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= GX as i32 || ny >= GY as i32 {
                    continue;
                }
                let n = ny as usize * GX + nx as usize;
                if !visto[n] && self.permanencia[n] >= corte {
                    visto[n] = true;
                    fila.push(n);
                }
            }
        }
        // Folga de uma célula em volta, sem encostar na borda.
        let fx = |i: usize| (i as f32 / GX as f32).clamp(MARGEM * 0.5, 1.0 - MARGEM * 0.5);
        let fy = |i: usize| (i as f32 / GY as f32).clamp(MARGEM * 0.5, 1.0 - MARGEM * 0.5);
        let (ax, ay) = (fx(x0.saturating_sub(1)), fy(y0.saturating_sub(1)));
        let (bx, by) = (fx((x1 + 2).min(GX)), fy((y1 + 2).min(GY)));
        // Zona que pega mais de 40% da imagem não é "um lugar onde se para".
        if (bx - ax) * (by - ay) > 0.4 {
            return None;
        }
        let segundos = soma / 5;
        Some(Sugestao {
            forma: Forma::Zona,
            nome: "Atendimento".into(),
            pontos: vec![
                Ponto { x: ax, y: ay },
                Ponto { x: bx, y: ay },
                Ponto { x: bx, y: by },
                Ponto { x: ax, y: by },
            ],
            motivo: format!(
                "Pessoas ficaram paradas aqui por cerca de {segundos} s no total (balcão, caixa ou porta de espera). A zona avisa quando tem alguém nesse ponto."
            ),
            confianca: (soma as f32 / 300.0).clamp(0.2, 1.0),
        })
    }
}

fn celula(x: f32, y: f32) -> usize {
    let cx = ((x.clamp(0.0, 0.999)) * GX as f32) as usize;
    let cy = ((y.clamp(0.0, 0.999)) * GY as f32) as usize;
    cy * GX + cx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fluxo_horizontal_vira_linha_vertical_longe_da_borda() {
        let mut o = Observador::new();
        // 5 pessoas atravessando da esquerda para a direita, na altura 0,6.
        for p in 0..5u32 {
            for i in 0..40 {
                let x = 0.02 + i as f32 * 0.024;
                o.alimentar(&[(p + 1, x, 0.6 + p as f32 * 0.02)]);
            }
            o.alimentar(&[]);
        }
        let s = o.sugerir();
        let l = s.iter().find(|s| s.forma == Forma::Linha).expect("linha");
        let (a, b) = (l.pontos[0], l.pontos[1]);
        assert!((a.x - b.x).abs() < 0.05, "linha deve ser quase vertical");
        for p in [a, b] {
            assert!(p.x >= MARGEM && p.x <= 1.0 - MARGEM);
            assert!(p.y >= MARGEM && p.y <= 1.0 - MARGEM);
        }
    }

    #[test]
    fn quem_para_vira_zona() {
        let mut o = Observador::new();
        for _ in 0..80 {
            o.alimentar(&[(7, 0.7, 0.5)]);
        }
        let s = o.sugerir();
        let z = s.iter().find(|s| s.forma == Forma::Zona).expect("zona");
        assert_eq!(z.pontos.len(), 4);
        assert!(z.pontos[0].x < 0.7 && z.pontos[2].x > 0.7);
        assert!(z.pontos[0].y < 0.5 && z.pontos[2].y > 0.5);
    }

    #[test]
    fn sem_gente_nao_sugere() {
        let mut o = Observador::new();
        for _ in 0..100 {
            o.alimentar(&[]);
        }
        assert!(o.sugerir().is_empty());
    }
}
