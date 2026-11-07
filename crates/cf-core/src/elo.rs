//! Agente-Cam-Elo-IA: identidade criptográfica e pareamento (SDD-08).
//!
//! O agente tem um par de chaves Ed25519 próprio. Ele assina o desafio do site
//! a cada conexão e só obedece a navegadores que autorizou na tela local.

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
const B64URL: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;

/// Prefixo que o site espera na assinatura do desafio (cfia-api, hub.rs).
pub const PREFIXO_DESAFIO: &str = "cfia-agente-v1:";
/// Endereço padrão do site.
pub const URL_PADRAO: &str = "https://cfia.ysafe.com.br";

pub struct Identidade {
    chave: SigningKey,
}

impl Identidade {
    /// Gera uma identidade nova com o gerador aleatório do sistema.
    pub fn nova() -> Self {
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).expect("gerador aleatório do sistema indisponível");
        Self {
            chave: SigningKey::from_bytes(&seed),
        }
    }

    /// Restaura de base64 (32 bytes da semente).
    pub fn de_b64(s: &str) -> Option<Self> {
        let b: [u8; 32] = B64.decode(s).ok()?.try_into().ok()?;
        Some(Self {
            chave: SigningKey::from_bytes(&b),
        })
    }

    pub fn para_b64(&self) -> String {
        B64.encode(self.chave.to_bytes())
    }

    /// Chave pública em base64, como o site a guarda.
    pub fn chave_publica(&self) -> String {
        B64.encode(self.chave.verifying_key().to_bytes())
    }

    /// Assina o desafio do site.
    pub fn assinar_desafio(&self, nonce: &str) -> String {
        let msg = format!("{PREFIXO_DESAFIO}{nonce}");
        B64.encode(self.chave.sign(msg.as_bytes()).to_bytes())
    }

    /// Impressão digital mostrada no site e na tela local (mesma regra do cfia-api).
    pub fn impressao(&self) -> String {
        impressao(&self.chave_publica())
    }
}

/// `XXXX-XXXX-XXXX-XXXX`: primeiros 16 dígitos hex do SHA-256 da chave em base64.
pub fn impressao(chave_publica_b64: &str) -> String {
    let h: String = Sha256::digest(chave_publica_b64.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    h.as_bytes()[..16]
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("-")
        .to_uppercase()
}

/// Código de pareamento de uso único (128 bits) e o hash que vai para o site.
pub fn novo_codigo() -> (String, String) {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("gerador aleatório do sistema indisponível");
    let codigo = B64URL.encode(b);
    let hash = hash_codigo(&codigo);
    (codigo, hash)
}

/// SHA-256 hexadecimal do código (o site guarda só isto).
pub fn hash_codigo(codigo: &str) -> String {
    Sha256::digest(codigo.trim().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Link do QR code que o técnico lê com o celular.
pub fn link_pareamento(base: &str, codigo: &str, impressao: &str) -> String {
    format!(
        "{}/parear#c={codigo}&f={impressao}",
        base.trim_end_matches('/')
    )
}

/// Converte `https://site` em `wss://site/ws/agente`.
pub fn url_ws(base: &str) -> String {
    let b = base.trim_end_matches('/');
    let b = b
        .strip_prefix("https://")
        .map(|h| format!("wss://{h}"))
        .or_else(|| b.strip_prefix("http://").map(|h| format!("ws://{h}")))
        .unwrap_or_else(|| b.to_string());
    format!("{b}/ws/agente")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, VerifyingKey};

    #[test]
    fn assinatura_confere_com_o_site() {
        let id = Identidade::nova();
        let sig = id.assinar_desafio("abc");
        let pk: [u8; 32] = B64.decode(id.chave_publica()).unwrap().try_into().unwrap();
        let sig: [u8; 64] = B64.decode(sig).unwrap().try_into().unwrap();
        VerifyingKey::from_bytes(&pk)
            .unwrap()
            .verify_strict(
                format!("{PREFIXO_DESAFIO}abc").as_bytes(),
                &Signature::from_bytes(&sig),
            )
            .unwrap();
    }

    #[test]
    fn persiste_e_restaura() {
        let a = Identidade::nova();
        let b = Identidade::de_b64(&a.para_b64()).unwrap();
        assert_eq!(a.chave_publica(), b.chave_publica());
        assert!(Identidade::de_b64("curto").is_none());
    }

    #[test]
    fn impressao_igual_ao_site() {
        // Mesmo algoritmo do cfia-api (util::fingerprint).
        let f = impressao("chave");
        assert_eq!(f.len(), 19);
        assert_eq!(f, f.to_uppercase());
    }

    #[test]
    fn codigo_e_hash() {
        let (c, h) = novo_codigo();
        assert_eq!(c.len(), 22);
        assert_eq!(h.len(), 64);
        let (c2, _) = novo_codigo();
        assert_ne!(c, c2);
    }

    #[test]
    fn urls() {
        assert_eq!(
            url_ws("https://cfia.ysafe.com.br/"),
            "wss://cfia.ysafe.com.br/ws/agente"
        );
        assert_eq!(
            url_ws("http://localhost:8080"),
            "ws://localhost:8080/ws/agente"
        );
        assert_eq!(
            link_pareamento("https://x.com", "abc", "AAAA-BBBB-CCCC-DDDD"),
            "https://x.com/parear#c=abc&f=AAAA-BBBB-CCCC-DDDD"
        );
    }
}
