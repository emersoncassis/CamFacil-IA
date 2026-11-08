//! Núcleo do CamFácil IA: tudo que não depende de HTTP.
//!
//! Cada módulo é pequeno e testável sem rede. Ver `docs/sdd/SDD-01-nucleo.md`.

pub mod bench;
pub mod conectar;
pub mod deteccao;
pub mod elo;
pub mod notebook;
pub mod onvif;
pub mod paths;
pub mod ports;
pub mod receita;
pub mod rede;
pub mod rtsp;
pub mod saude;
pub mod store;
pub mod sugestao;
pub mod system;
pub mod unifilar;
pub mod visao;

/// Versão do binário, exibida na dashboard.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Versão vigente do texto de isenção (`docs/legal/ISENCAO.md`).
pub const DISCLAIMER_VERSION: &str = "1.0";
