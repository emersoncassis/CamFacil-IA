//! Catálogo das inteligências de uma regra (SDD-09): o que cada uma faz,
//! quanto pesa, de qual módulo depende e quais classes aceita.
//!
//! Uma regra tem **uma** inteligência. Quem escolhe é o técnico, na hora de
//! criar; a tela só deixa marcar o que está disponível (módulo ligado).

use crate::visao::{Forma, Intel};
use serde::Serialize;

/// Peso para a máquina: leve roda em qualquer PC velho; pesado pede mais.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Peso {
    Leve,
    Medio,
    Pesado,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Inteligencia {
    pub id: &'static str,
    pub forma: &'static str,
    pub peso: Peso,
    /// Módulo que precisa estar ligado (id de `video::MODULOS`).
    pub modulo: &'static str,
    /// Parâmetro numérico: (unidade, mínimo, padrão, máximo); `None` = não tem.
    pub parametro: Option<(&'static str, u32, u32, u32)>,
}

pub const INTELIGENCIAS: &[Inteligencia] = &[
    Inteligencia {
        id: "passagem",
        forma: "linha",
        peso: Peso::Leve,
        modulo: "contagem",
        parametro: None,
    },
    Inteligencia {
        id: "presenca",
        forma: "zona",
        peso: Peso::Leve,
        modulo: "pessoas",
        parametro: None,
    },
    Inteligencia {
        id: "densidade",
        forma: "zona",
        peso: Peso::Medio,
        modulo: "pessoas",
        parametro: Some(("quantos", 2, 3, 50)),
    },
    Inteligencia {
        id: "permanencia",
        forma: "zona",
        peso: Peso::Medio,
        modulo: "permanencia",
        parametro: Some(("segundos", 5, 60, 3600)),
    },
    Inteligencia {
        id: "calor",
        forma: "zona",
        peso: Peso::Pesado,
        modulo: "calor",
        parametro: None,
    },
];

/// Classes que o técnico pode escolher no modelo padrão (COCO): índice e id
/// curto usado na tela e nos textos.
pub const CLASSES_ESCOLHA: &[(usize, &str)] = &[
    (0, "pessoa"),
    (2, "carro"),
    (3, "moto"),
    (1, "bicicleta"),
    (5, "onibus"),
    (7, "caminhao"),
];

pub fn achar(id: &str) -> Option<&'static Inteligencia> {
    INTELIGENCIAS.iter().find(|i| i.id == id)
}

/// Sons e cores aceitos (ids; a tela traduz e desenha).
pub const SONS: &[&str] = &["bipe", "sino", "alarme", "gota", "silencio"];
pub const CORES: &[&str] = &["verde", "ambar", "vermelho", "azul", "branco", "magenta"];

/// Por que a combinação não vale (mensagem para o técnico), ou `None`.
pub fn validar(
    forma: &Forma,
    intel: Intel,
    classes: &[usize],
    parametro: u32,
    cor: &str,
    som: &str,
    estudo: bool,
) -> Option<String> {
    if intel != Intel::Legado {
        let id = match intel {
            Intel::Passagem => "passagem",
            Intel::Presenca => "presenca",
            Intel::Permanencia => "permanencia",
            Intel::Densidade => "densidade",
            Intel::Calor => "calor",
            Intel::Legado => unreachable!(),
        };
        let i = achar(id)?;
        let forma_txt = if *forma == Forma::Linha {
            "linha"
        } else {
            "zona"
        };
        if i.forma != forma_txt {
            return Some(format!(
                "essa inteligência é de {}, não de {forma_txt}",
                i.forma
            ));
        }
        if let Some((_, min, _, max)) = i.parametro {
            if parametro != 0 && !(min..=max).contains(&parametro) {
                return Some(format!("use um valor de {min} a {max}"));
            }
        }
    }
    let valida = |c: &usize| {
        CLASSES_ESCOLHA.iter().any(|(i, _)| i == c)
            || (estudo && *c >= crate::deteccao::CLASSE_ESTUDO)
    };
    if classes.len() > 12 || classes.iter().any(|c| !valida(c)) {
        return Some("classe desconhecida".into());
    }
    if !cor.is_empty() && !CORES.contains(&cor) {
        return Some("cor desconhecida".into());
    }
    if !som.is_empty() && !SONS.contains(&som) {
        return Some("som desconhecido".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linha_so_aceita_passagem() {
        assert!(validar(&Forma::Linha, Intel::Densidade, &[], 0, "", "", false).is_some());
        assert!(validar(&Forma::Zona, Intel::Passagem, &[], 0, "", "", false).is_some());
        assert!(validar(
            &Forma::Linha,
            Intel::Passagem,
            &[2, 3],
            0,
            "ambar",
            "sino",
            false
        )
        .is_none());
    }

    #[test]
    fn limites_e_classes() {
        assert!(validar(&Forma::Zona, Intel::Densidade, &[0], 1, "", "", false).is_some());
        assert!(validar(&Forma::Zona, Intel::Densidade, &[0], 4, "", "", false).is_none());
        assert!(validar(&Forma::Zona, Intel::Presenca, &[42], 0, "", "", false).is_some());
        assert!(validar(&Forma::Zona, Intel::Presenca, &[1002], 0, "", "", false).is_some());
        assert!(validar(&Forma::Zona, Intel::Presenca, &[1002], 0, "", "", true).is_none());
        assert!(validar(&Forma::Zona, Intel::Presenca, &[0], 0, "rosa", "", false).is_some());
    }
}
