//! Utilidades: aleatoriedade, hashes e codificação.

use base64::Engine;
use sha2::{Digest, Sha256};

pub const B64URL: base64::engine::GeneralPurpose = base64::engine::general_purpose::URL_SAFE_NO_PAD;
pub const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

/// `n` bytes aleatórios do sistema operacional, em base64url.
pub fn token(n: usize) -> String {
    let mut buf = vec![0u8; n];
    getrandom::fill(&mut buf).expect("gerador aleatório do sistema indisponível");
    B64URL.encode(buf)
}

/// SHA-256 em hexadecimal.
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// SHA-256 em base64url (desafio PKCE).
pub fn sha256_b64url(data: &[u8]) -> String {
    B64URL.encode(Sha256::digest(data))
}

/// Impressão digital curta de uma chave pública, para mostrar ao técnico.
pub fn fingerprint(chave_publica: &str) -> String {
    let h = sha256_hex(chave_publica.as_bytes());
    h.as_bytes()[..16]
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("-")
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_distintos_e_tamanho() {
        let a = token(32);
        let b = token(32);
        assert_ne!(a, b);
        assert_eq!(a.len(), 43);
    }

    #[test]
    fn pkce_rfc7636() {
        // Exemplo do apêndice B da RFC 7636.
        assert_eq!(
            sha256_b64url(b"dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn impressao_digital() {
        let f = fingerprint("chave");
        assert_eq!(f.len(), 19);
        assert_eq!(f.matches('-').count(), 3);
    }
}
