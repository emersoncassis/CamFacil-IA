//! "IA, avalie esta regra": observa a câmera por um tempo e diz se a regra
//! criada está bem posicionada e bem ajustada, em frases curtas.
//!
//! Usa as mesmas trilhas (id, x, y, classe) do "sugira para mim". Para linha:
//! quantos passaram, quantos cruzaram, quem passou perto sem cruzar. Para
//! zona: quantos entraram, o máximo ao mesmo tempo, quanto tempo cada um
//! ficou. Compara com o ajuste da regra (limite de densidade, tempo de
//! permanência) e sugere o número certo.

use crate::deteccao::{plural, regra_casa};
use crate::visao::{cruzou, dentro, Forma, Intel, Ponto, Regra};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Distância (da imagem) até a linha que conta como "passou perto".
const PERTO: f32 = 0.06;
/// Tempo padrão de permanência quando a regra não definiu o seu.
const PERMANENCIA_PADRAO: u32 = 60;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Achado {
    /// `ok`, `atencao` ou `problema`.
    pub tom: &'static str,
    pub texto: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Avaliacao {
    pub regra: String,
    pub achados: Vec<Achado>,
}

pub struct Avaliador {
    regra: Regra,
    ultimo: HashMap<u32, Ponto>,
    classe: HashMap<u32, usize>,
    /// Quem se moveu (da classe da regra).
    moveu: HashSet<u32>,
    /// Quem cruzou a linha / entrou na zona.
    acertou: HashSet<u32>,
    /// Linha: quem chegou perto sem precisar cruzar.
    perto: HashSet<u32>,
    /// Zona: quem já estava dentro no quadro anterior.
    dentro_antes: HashSet<u32>,
    /// Segundos dentro da zona, por trilha.
    ficou: HashMap<u32, f32>,
    max_juntos: usize,
    /// Classes que a regra ignora mas apareceram na linha/zona.
    ignoradas: HashMap<usize, HashSet<u32>>,
    pub quadros: u64,
}

fn dist_segmento(p: Ponto, a: Ponto, b: Ponto) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let t = if dx == 0.0 && dy == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0)
    };
    ((p.x - (a.x + t * dx)).powi(2) + (p.y - (a.y + t * dy)).powi(2)).sqrt()
}

impl Avaliador {
    pub fn new(regra: Regra) -> Self {
        Self {
            regra,
            ultimo: HashMap::new(),
            classe: HashMap::new(),
            moveu: HashSet::new(),
            acertou: HashSet::new(),
            perto: HashSet::new(),
            dentro_antes: HashSet::new(),
            ficou: HashMap::new(),
            max_juntos: 0,
            ignoradas: HashMap::new(),
            quadros: 0,
        }
    }

    /// `eq` = quantos quadros de 1/5 s esta chamada vale.
    pub fn alimentar(&mut self, trilhas: &[(u32, f32, f32, usize)], eq: u32) {
        self.quadros += 1;
        let seg = eq as f32 * 0.2;
        let r = self.regra.clone();
        let mut dentro_agora: HashSet<u32> = HashSet::new();
        for &(id, x, y, classe) in trilhas {
            let p = Ponto { x, y };
            self.classe.insert(id, classe);
            let casa = regra_casa(&r, classe);
            let anterior = self.ultimo.get(&id).copied();
            let andou =
                anterior.is_some_and(|o| ((x - o.x).powi(2) + (y - o.y).powi(2)).sqrt() > 0.004);
            match r.forma {
                Forma::Linha if r.pontos.len() >= 2 => {
                    let (a, b) = (r.pontos[0], r.pontos[1]);
                    let perto = dist_segmento(p, a, b) < PERTO;
                    if casa {
                        if andou {
                            self.moveu.insert(id);
                        }
                        if perto {
                            self.perto.insert(id);
                        }
                        if anterior.is_some_and(|o| cruzou(a, b, o, p).is_some()) {
                            self.acertou.insert(id);
                        }
                    } else if perto && andou {
                        self.ignoradas.entry(classe).or_default().insert(id);
                    }
                }
                Forma::Zona if r.pontos.len() >= 3 => {
                    let dentro_zona = dentro(p, &r.pontos);
                    if casa {
                        if andou {
                            self.moveu.insert(id);
                        }
                        if dentro_zona {
                            dentro_agora.insert(id);
                            *self.ficou.entry(id).or_default() += seg;
                            if !self.dentro_antes.contains(&id) && anterior.is_some() {
                                self.acertou.insert(id);
                            }
                        }
                    } else if dentro_zona {
                        self.ignoradas.entry(classe).or_default().insert(id);
                    }
                }
                _ => {}
            }
            self.ultimo.insert(id, p);
        }
        self.max_juntos = self.max_juntos.max(dentro_agora.len());
        self.dentro_antes = dentro_agora;
    }

    pub fn resultado(&self) -> Avaliacao {
        let r = &self.regra;
        let mut a: Vec<Achado> = Vec::new();
        let mut diz = |tom: &'static str, texto: String| a.push(Achado { tom, texto });
        let intel = r.ajustes.intel;
        let quem = |ids: &HashSet<u32>| -> String {
            let mut por: HashMap<usize, usize> = HashMap::new();
            for id in ids {
                if let Some(c) = self.classe.get(id) {
                    *por.entry(*c).or_default() += 1;
                }
            }
            let mut v: Vec<_> = por.into_iter().collect();
            v.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
            v.iter()
                .map(|(c, n)| plural(*c, *n))
                .collect::<Vec<_>>()
                .join(" e ")
        };
        if self.quadros < 10 {
            diz(
                "atencao",
                "Pouco tempo observando. Deixe rodar mais um pouco.".into(),
            );
        } else if r.forma == Forma::Linha {
            let vistos = self.moveu.len().max(self.perto.len());
            let cruzaram = self.acertou.len();
            if vistos == 0 {
                diz(
                    "atencao",
                    "Ninguém do que a regra olha passou neste tempo.".into(),
                );
            } else if cruzaram == 0 {
                diz(
                    "problema",
                    format!(
                        "Passaram {vistos}, mas nenhum cruzou a linha. Ela está fora do caminho."
                    ),
                );
            } else if cruzaram * 10 < vistos * 4 && vistos >= 3 {
                diz(
                    "atencao",
                    format!("Só {cruzaram} de {vistos} cruzaram a linha. Ela pega pouco do fluxo."),
                );
            } else {
                diz(
                    "ok",
                    format!("{} cruzaram a linha. Bem posicionada.", quem(&self.acertou)),
                );
            }
            let _ = intel;
        } else {
            let entraram = self.acertou.len();
            let longos: Vec<f32> = self.ficou.values().copied().collect();
            let maior = longos.iter().copied().fold(0.0f32, f32::max);
            if entraram == 0 && self.ficou.is_empty() {
                if self.moveu.is_empty() {
                    diz(
                        "atencao",
                        "Nada do que a regra olha entrou na área neste tempo.".into(),
                    );
                } else {
                    diz(
                        "problema",
                        "Passou gente, mas ninguém entrou na área. Ela está fora do movimento."
                            .into(),
                    );
                }
            } else {
                match intel {
                    Intel::Densidade => {
                        let lim = (r.ajustes.parametro as usize).max(2);
                        if self.max_juntos >= lim {
                            diz(
                                "ok",
                                format!(
                                    "O limite de {lim} foi atingido (máximo {} juntos).",
                                    self.max_juntos
                                ),
                            );
                        } else {
                            diz(
                                "atencao",
                                format!(
                                    "No máximo {} ao mesmo tempo; o limite de {lim} não foi atingido. Para ver o alerta, baixe para {}.",
                                    self.max_juntos,
                                    self.max_juntos.max(2)
                                ),
                            );
                        }
                    }
                    Intel::Permanencia => {
                        let p = if r.ajustes.parametro > 0 {
                            r.ajustes.parametro
                        } else {
                            PERMANENCIA_PADRAO
                        };
                        let passaram = longos.iter().filter(|s| **s >= p as f32).count();
                        if passaram > 0 {
                            diz(
                                "ok",
                                format!("{passaram} ficaram mais de {p} s. A regra dispararia."),
                            );
                        } else {
                            let sug = ((maior * 0.6).round() as u32).max(5);
                            diz(
                                "atencao",
                                format!("Ninguém ficou {p} s (o maior foi {} s). Para ver o alerta, use ~{sug} s.", maior.round() as u32),
                            );
                        }
                        let todos = longos.iter().filter(|s| **s >= p as f32).count();
                        if longos.len() >= 4 && todos == longos.len() {
                            diz("atencao", format!("Todos ficaram mais de {p} s. Se isso é normal aqui, aumente o tempo."));
                        }
                    }
                    _ => {
                        diz(
                            "ok",
                            format!(
                                "{} entraram na área.",
                                if entraram > 0 {
                                    quem(&self.acertou)
                                } else {
                                    "Já havia gente dentro".into()
                                }
                            ),
                        );
                    }
                }
            }
        }
        // Quem a regra ignora e apareceu onde ela olha.
        let mut ign: Vec<(usize, usize)> =
            self.ignoradas.iter().map(|(c, v)| (*c, v.len())).collect();
        ign.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
        if !ign.is_empty() {
            let t = ign
                .iter()
                .map(|(c, n)| plural(*c, *n))
                .collect::<Vec<_>>()
                .join(" e ");
            diz(
                "atencao",
                format!(
                    "Também apareceram {t}, que a regra ignora. Marque em Quem se quiser incluir."
                ),
            );
        }
        Avaliacao {
            regra: r.nome.clone(),
            achados: a,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::visao::AjustesRegra;

    fn linha(classes: Vec<usize>) -> Regra {
        Regra {
            id: 1,
            nome: "Portão".into(),
            forma: Forma::Linha,
            pontos: vec![Ponto { x: 0.5, y: 0.1 }, Ponto { x: 0.5, y: 0.9 }],
            ajustes: AjustesRegra {
                intel: Intel::Passagem,
                classes,
                ..Default::default()
            },
        }
    }

    fn zona(intel: Intel, classes: Vec<usize>, parametro: u32) -> Regra {
        Regra {
            id: 2,
            nome: "Vaga".into(),
            forma: Forma::Zona,
            pontos: vec![
                Ponto { x: 0.6, y: 0.2 },
                Ponto { x: 0.9, y: 0.2 },
                Ponto { x: 0.9, y: 0.8 },
                Ponto { x: 0.6, y: 0.8 },
            ],
            ajustes: AjustesRegra {
                intel,
                classes,
                parametro,
                ..Default::default()
            },
        }
    }

    fn passar(a: &mut Avaliador, id: u32, y: f32, classe: usize) {
        for i in 0..30 {
            a.alimentar(&[(id, 0.052 + i as f32 * 0.03, y, classe)], 1);
        }
        a.alimentar(&[], 1);
    }

    #[test]
    fn linha_bem_posicionada_e_classe_ignorada() {
        let mut a = Avaliador::new(linha(vec![0]));
        for p in 0..3 {
            passar(&mut a, p + 1, 0.5, 0);
        }
        passar(&mut a, 9, 0.5, 2);
        let r = a.resultado();
        assert_eq!(r.achados[0].tom, "ok", "{r:?}");
        assert!(r.achados[0].texto.contains("3 pessoas"), "{r:?}");
        assert!(
            r.achados.iter().any(|x| x.texto.contains("1 carro")),
            "{r:?}"
        );
    }

    #[test]
    fn linha_fora_do_caminho_e_problema() {
        let mut a = Avaliador::new(linha(vec![0]));
        // Gente andando na parte de baixo, mas a linha só vai até y=0.9 e eles
        // passam pelo lado esquerdo sem chegar a x=0.5.
        for p in 0..3u32 {
            for i in 0..30 {
                a.alimentar(&[(p + 1, 0.05 + i as f32 * 0.005, 0.5, 0)], 1);
            }
        }
        let r = a.resultado();
        assert_eq!(r.achados[0].tom, "problema", "{r:?}");
    }

    #[test]
    fn permanencia_longa_demais_sugere_tempo() {
        let mut a = Avaliador::new(zona(Intel::Permanencia, vec![2], 120));
        for i in 0..60 {
            a.alimentar(&[(1, 0.7, 0.5, 2)], 2);
            let _ = i;
        }
        let r = a.resultado();
        let t = &r.achados[0];
        assert_eq!(t.tom, "atencao", "{r:?}");
        assert!(t.texto.contains("use ~"), "{t:?}");
    }

    #[test]
    fn densidade_atingida() {
        let mut a = Avaliador::new(zona(Intel::Densidade, vec![2], 2));
        for _ in 0..20 {
            a.alimentar(&[(1, 0.65, 0.4, 2), (2, 0.8, 0.6, 2)], 2);
        }
        let r = a.resultado();
        assert_eq!(r.achados[0].tom, "ok", "{r:?}");
    }

    #[test]
    fn zona_fora_do_movimento() {
        let mut a = Avaliador::new(zona(Intel::Presenca, vec![0], 0));
        for p in 0..3u32 {
            for i in 0..30 {
                a.alimentar(&[(p + 1, 0.05 + i as f32 * 0.005, 0.5, 0)], 1);
            }
        }
        let r = a.resultado();
        assert_eq!(r.achados[0].tom, "problema", "{r:?}");
    }
}
