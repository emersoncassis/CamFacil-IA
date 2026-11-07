//! Notificações do CamFácil IA (SDD-04): Telegram pela Bot API oficial,
//! fila offline com limite, antirrepique e cofre de segredos.

pub mod cofre;
pub mod servico;
pub mod telegram;

pub use servico::{Alerta, Anfitriao, Telegram};
