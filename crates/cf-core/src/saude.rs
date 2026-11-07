//! Monitor de saúde da câmera (SDD-02, nível leve; SDD-05, diagnóstico).
//!
//! Classifica cada quadro (ok, escura, coberta, desfocada) e, com a queda do
//! sinal, "sem sinal". Só anuncia mudança de estado depois de alguns segundos
//! na mesma situação (histerese): uma pessoa passando na frente da lente ou
//! uma nuvem não viram evento. Sem spam: um evento por mudança.

use crate::visao::{saude, Quadro};
use serde::Serialize;
use std::time::{Duration, Instant};

/// Situação da imagem de uma câmera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Situacao {
    Ok,
    Escura,
    Coberta,
    Desfocada,
    SemSinal,
}

impl Situacao {
    /// Chave estável usada no banco, na API e nas traduções (`saude.<chave>`).
    pub fn chave(self) -> &'static str {
        match self {
            Situacao::Ok => "ok",
            Situacao::Escura => "escura",
            Situacao::Coberta => "coberta",
            Situacao::Desfocada => "desfocada",
            Situacao::SemSinal => "sem_sinal",
        }
    }

    pub fn problema(self) -> bool {
        self != Situacao::Ok
    }
}

/// Limiares para cena de loja, em quadros cinza pequenos (~160 px de largura).
pub const ESCURA_MEDIA: f32 = 18.0;
pub const COBERTA_DESVIO: f32 = 6.0;
pub const DESFOCADA_NITIDEZ: f32 = 12.0;

/// Classifica um quadro pela imagem.
pub fn classificar(q: &Quadro) -> Situacao {
    let s = saude(q);
    if s.media < ESCURA_MEDIA {
        Situacao::Escura
    } else if s.desvio < COBERTA_DESVIO {
        Situacao::Coberta
    } else if s.nitidez < DESFOCADA_NITIDEZ {
        Situacao::Desfocada
    } else {
        Situacao::Ok
    }
}

/// Mudança confirmada de estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mudanca {
    pub de: Situacao,
    pub para: Situacao,
}

/// Máquina de estados com histerese por tempo.
pub struct Monitor {
    atual: Situacao,
    desde: Instant,
    candidata: Option<(Situacao, Instant)>,
    ultimo_problema: Option<Instant>,
    histerese: Duration,
}

/// Histerese padrão: a nova situação precisa durar isto para valer.
pub const HISTERESE: Duration = Duration::from_secs(4);

impl Default for Monitor {
    fn default() -> Self {
        Self::new(HISTERESE, Instant::now())
    }
}

impl Monitor {
    pub fn new(histerese: Duration, agora: Instant) -> Self {
        Self {
            atual: Situacao::Ok,
            desde: agora,
            candidata: None,
            ultimo_problema: None,
            histerese,
        }
    }

    pub fn atual(&self) -> Situacao {
        self.atual
    }

    /// Há quanto tempo está na situação atual.
    pub fn ha_quanto(&self, agora: Instant) -> Duration {
        agora.saturating_duration_since(self.desde)
    }

    /// Tempo desde o começo do último problema (ou `None` se nunca houve).
    pub fn desde_ultimo_problema(&self, agora: Instant) -> Option<Duration> {
        self.ultimo_problema
            .map(|t| agora.saturating_duration_since(t))
    }

    /// Alimenta uma observação. Devolve a mudança só quando a nova situação
    /// se sustentou pela histerese inteira.
    pub fn observar(&mut self, s: Situacao, agora: Instant) -> Option<Mudanca> {
        if s == self.atual {
            self.candidata = None;
            return None;
        }
        match self.candidata {
            Some((c, inicio)) if c == s => {
                if agora.saturating_duration_since(inicio) < self.histerese {
                    return None;
                }
                let de = self.atual;
                self.atual = s;
                self.desde = inicio;
                self.candidata = None;
                if s.problema() {
                    self.ultimo_problema = Some(inicio);
                }
                Some(Mudanca { de, para: s })
            }
            _ => {
                self.candidata = Some((s, agora));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn so_muda_depois_da_histerese() {
        let t0 = Instant::now();
        let mut m = Monitor::new(seg(4), t0);
        assert_eq!(m.atual(), Situacao::Ok);
        // Escura por 3 s: ainda nada.
        for i in 0..=3 {
            assert_eq!(m.observar(Situacao::Escura, t0 + seg(i)), None);
        }
        // Aos 4 s confirma, uma vez só.
        assert_eq!(
            m.observar(Situacao::Escura, t0 + seg(4)),
            Some(Mudanca {
                de: Situacao::Ok,
                para: Situacao::Escura
            })
        );
        assert_eq!(m.observar(Situacao::Escura, t0 + seg(5)), None);
        assert_eq!(m.atual(), Situacao::Escura);
        assert_eq!(m.desde_ultimo_problema(t0 + seg(10)), Some(seg(10)));
    }

    #[test]
    fn piscada_curta_nao_gera_evento() {
        let t0 = Instant::now();
        let mut m = Monitor::new(seg(4), t0);
        assert_eq!(m.observar(Situacao::Coberta, t0 + seg(1)), None);
        assert_eq!(m.observar(Situacao::Coberta, t0 + seg(2)), None);
        // Voltou ao normal antes da histerese: candidata descartada.
        assert_eq!(m.observar(Situacao::Ok, t0 + seg(3)), None);
        assert_eq!(m.observar(Situacao::Coberta, t0 + seg(4)), None);
        assert_eq!(m.observar(Situacao::Coberta, t0 + seg(7)), None);
        assert_eq!(m.atual(), Situacao::Ok);
        assert_eq!(m.desde_ultimo_problema(t0 + seg(7)), None);
    }

    #[test]
    fn troca_de_candidata_reinicia_contagem() {
        let t0 = Instant::now();
        let mut m = Monitor::new(seg(4), t0);
        m.observar(Situacao::Escura, t0);
        m.observar(Situacao::Escura, t0 + seg(3));
        // Mudou para outro problema: a contagem começa de novo.
        assert_eq!(m.observar(Situacao::SemSinal, t0 + seg(4)), None);
        assert_eq!(m.observar(Situacao::SemSinal, t0 + seg(7)), None);
        assert_eq!(
            m.observar(Situacao::SemSinal, t0 + seg(8)).map(|c| c.para),
            Some(Situacao::SemSinal)
        );
        // Volta ao normal também respeita a histerese.
        assert_eq!(m.observar(Situacao::Ok, t0 + seg(9)), None);
        assert_eq!(
            m.observar(Situacao::Ok, t0 + seg(13)),
            Some(Mudanca {
                de: Situacao::SemSinal,
                para: Situacao::Ok
            })
        );
        assert_eq!(m.ha_quanto(t0 + seg(20)), seg(11));
    }

    #[test]
    fn classifica_pela_imagem() {
        let (w, h) = (160, 90);
        let escura = Quadro {
            w,
            h,
            px: vec![5; w * h],
        };
        let coberta = Quadro {
            w,
            h,
            px: vec![120; w * h],
        };
        let mut px = vec![0u8; w * h];
        for y in 0..h {
            for x in 0..w {
                px[y * w + x] = 40 + ((x * 7 + y * 13) % 23) as u8;
            }
        }
        let normal = Quadro { w, h, px };
        assert_eq!(classificar(&escura), Situacao::Escura);
        assert_eq!(classificar(&coberta), Situacao::Coberta);
        assert_eq!(classificar(&normal), Situacao::Ok);
        assert_eq!(Situacao::SemSinal.chave(), "sem_sinal");
    }
}
