//! Núcleo do CamFácil IA: tudo que não depende de HTTP.
//!
//! Cada módulo é pequeno e testável sem rede. Ver `docs/sdd/SDD-01-nucleo.md`.

pub mod bench;
pub mod onvif;
pub mod paths;
pub mod ports;
pub mod rtsp;
pub mod store;
pub mod system;

/// Versão do binário, exibida na dashboard.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Versão vigente do texto de isenção (`docs/legal/ISENCAO.md`).
pub const DISCLAIMER_VERSION: &str = "1.0";
