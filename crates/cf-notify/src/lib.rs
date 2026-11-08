//! Notificações do CamFácil IA (SDD-04): Telegram pela Bot API oficial,
//! WhatsApp pela Evolution API (gateway HTTP), webhook genérico, fila offline
//! com limite, antirrepique e cofre de segredos.

pub mod cofre;
pub mod comandos;
pub mod servico;
pub mod telegram;
pub mod tls;
pub mod webhook;
pub mod whatsapp;

pub use servico::{Alerta, Anfitriao, Telegram};
pub use webhook::Webhook;
pub use whatsapp::WhatsApp;
