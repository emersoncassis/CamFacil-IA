//! Comandos de chat compartilhados pelos canais (WhatsApp e, no futuro, outros):
//! `status`, `foto`, `resumo`, `liga`, `desliga`, `ajuda`. Aceita com ou sem a
//! barra inicial, porque no WhatsApp ninguém digita `/`.

use crate::servico::{escolher_camera, hora_local, separar, Anfitriao};
use cf_core::store::Store;

/// O que responder ao contato.
pub enum Resposta {
    Texto(String),
    Foto {
        jpeg: Vec<u8>,
        legenda: String,
    },
    /// Mensagem comum (não é comando): não responder nada.
    Nada,
}

pub const AJUDA: &str = "Comandos:\nstatus — câmeras, saúde e módulos\nfoto <nome ou nº> — foto da câmera agora\nresumo — alertas de hoje\nliga <módulo> · desliga <módulo>\najuda — esta lista";

fn agora_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Interpreta `texto` vindo de um contato **já autorizado** e monta a resposta.
/// `canal` e `remetente` só entram no log do evento `comando`.
pub fn responder(
    anfitriao: &dyn Anfitriao,
    store: &Store,
    texto: &str,
    canal: &str,
    remetente: &str,
) -> Resposta {
    let (cmd, arg) = separar(texto);
    let cmd = cmd.trim_start_matches('/').to_string();
    let r = match cmd.as_str() {
        "start" | "ajuda" | "help" | "oi" | "ola" | "olá" | "menu" => AJUDA.to_string(),
        "status" => anfitriao.status(),
        "resumo" => anfitriao.resumo(),
        "liga" | "desliga" => {
            let ligar = cmd == "liga";
            if arg.is_empty() {
                "Diga qual: liga movimento · cerca · saude · pessoas · contagem · permanencia · calor"
                    .into()
            } else {
                let r = anfitriao.modulo(&arg, ligar);
                let _ = store.registrar_evento(
                    0,
                    None,
                    "comando",
                    &format!("/{cmd} {arg} via {canal} ({remetente})"),
                    agora_unix(),
                );
                r.unwrap_or_else(|e| e)
            }
        }
        "foto" => {
            let cams = anfitriao.cameras();
            match escolher_camera(&cams, &arg) {
                Ok((id, nome)) => match anfitriao.foto(id) {
                    Some(jpeg) => {
                        return Resposta::Foto {
                            jpeg,
                            legenda: format!("{nome} · {}", hora_local(store)),
                        }
                    }
                    None => format!("{nome}: ainda sem imagem (câmera fora do ar?)."),
                },
                Err(msg) => msg,
            }
        }
        _ if texto.trim_start().starts_with('/') => format!("Não conheço {cmd}.\n\n{AJUDA}"),
        _ => return Resposta::Nada,
    };
    Resposta::Texto(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Falso;
    impl Anfitriao for Falso {
        fn status(&self) -> String {
            "tudo ok".into()
        }
        fn resumo(&self) -> String {
            "resumo".into()
        }
        fn cameras(&self) -> Vec<(i64, String)> {
            vec![(1, "Loja".into())]
        }
        fn foto(&self, _: i64) -> Option<Vec<u8>> {
            Some(vec![1, 2, 3])
        }
        fn modulo(&self, nome: &str, ligado: bool) -> Result<String, String> {
            Ok(format!("{nome}:{ligado}"))
        }
    }

    #[test]
    fn com_e_sem_barra() {
        let s = Store::memory().unwrap();
        let txt = |r: Resposta| match r {
            Resposta::Texto(t) => t,
            Resposta::Foto { .. } => "<foto>".into(),
            Resposta::Nada => "<nada>".into(),
        };
        assert_eq!(txt(responder(&Falso, &s, "status", "wa", "x")), "tudo ok");
        assert_eq!(txt(responder(&Falso, &s, "/Status", "wa", "x")), "tudo ok");
        assert_eq!(txt(responder(&Falso, &s, "foto", "wa", "x")), "<foto>");
        assert_eq!(txt(responder(&Falso, &s, "bom dia", "wa", "x")), "<nada>");
        assert!(txt(responder(&Falso, &s, "/xyz", "wa", "x")).starts_with("Não conheço"));
        assert_eq!(
            txt(responder(&Falso, &s, "liga cerca", "wa", "5511")),
            "cerca:true"
        );
        assert_eq!(s.eventos(5).unwrap()[0].tipo, "comando");
    }
}
