//! Cofre de segredos: cifra tokens antes de gravar nas configurações (regra 7).
//!
//! ChaCha20-Poly1305 com uma chave de 32 bytes gerada na primeira vez e
//! guardada em `data_dir/segredo.key` com permissão só do dono (Unix). Quem
//! copiar só o banco não lê o token; quem copiar os dois tem a máquina toda.

use base64::prelude::*;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use std::path::{Path, PathBuf};

const PREFIXO: &str = "enc:";
const ARQUIVO: &str = "segredo.key";

pub struct Cofre {
    chave: Key,
}

impl Cofre {
    /// Abre (ou cria) a chave na pasta de dados do CamFácil IA.
    pub fn abrir() -> Result<Self, String> {
        Self::abrir_em(&cf_core::paths::data_dir())
    }

    pub fn abrir_em(dir: &Path) -> Result<Self, String> {
        let caminho: PathBuf = dir.join(ARQUIVO);
        let bytes = match std::fs::read(&caminho) {
            Ok(b) if b.len() == 32 => b,
            _ => {
                let chave = ChaCha20Poly1305::generate_key(&mut OsRng);
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                escrever_restrito(&caminho, chave.as_slice())?;
                chave.to_vec()
            }
        };
        Ok(Self {
            chave: *Key::from_slice(&bytes),
        })
    }

    /// Devolve `enc:<base64(nonce || cifra)>`.
    pub fn cifrar(&self, texto: &str) -> String {
        let c = ChaCha20Poly1305::new(&self.chave);
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        let mut saida = nonce.to_vec();
        // `encrypt` só falha por tamanho absurdo; token nunca chega perto.
        saida.extend(c.encrypt(&nonce, texto.as_bytes()).unwrap_or_default());
        format!("{PREFIXO}{}", BASE64_STANDARD.encode(saida))
    }

    pub fn decifrar(&self, guardado: &str) -> Option<String> {
        let b64 = guardado.strip_prefix(PREFIXO)?;
        let bytes = BASE64_STANDARD.decode(b64).ok()?;
        if bytes.len() < 12 {
            return None;
        }
        let (nonce, ct) = bytes.split_at(12);
        let c = ChaCha20Poly1305::new(&self.chave);
        let claro = c.decrypt(Nonce::from_slice(nonce), ct).ok()?;
        String::from_utf8(claro).ok()
    }
}

/// `n` bytes aleatórios do sistema (sal, nonce).
pub fn aleatorio(n: usize) -> Vec<u8> {
    use chacha20poly1305::aead::rand_core::RngCore;
    let mut v = vec![0u8; n];
    OsRng.fill_bytes(&mut v);
    v
}

/// Cifra bytes com uma chave dada (backup protegido por senha). Saída:
/// `nonce (12) || cifra+tag`.
pub fn selar(chave: &[u8; 32], dados: &[u8]) -> Vec<u8> {
    let c = ChaCha20Poly1305::new(Key::from_slice(chave));
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let mut saida = nonce.to_vec();
    saida.extend(c.encrypt(&nonce, dados).unwrap_or_default());
    saida
}

/// Inverso de [`selar`]; `None` se a chave estiver errada ou o arquivo mexido.
pub fn abrir_selado(chave: &[u8; 32], selado: &[u8]) -> Option<Vec<u8>> {
    if selado.len() < 12 + 16 {
        return None;
    }
    let (nonce, ct) = selado.split_at(12);
    ChaCha20Poly1305::new(Key::from_slice(chave))
        .decrypt(Nonce::from_slice(nonce), ct)
        .ok()
}

fn escrever_restrito(caminho: &Path, bytes: &[u8]) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(caminho)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())
    }
    #[cfg(not(unix))]
    {
        // Windows: %PROGRAMDATA%\CamFacil já é restrito a administradores por
        // padrão; não há modo POSIX para apertar mais sem a API de ACL.
        std::fs::write(caminho, bytes).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cifra_e_decifra_com_a_mesma_chave() {
        let dir = std::env::temp_dir().join(format!("cf-cofre-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let c = Cofre::abrir_em(&dir).unwrap();
        let g = c.cifrar("123456:ABC-token");
        assert!(g.starts_with("enc:"));
        assert!(!g.contains("ABC-token"));
        assert_eq!(c.decifrar(&g).as_deref(), Some("123456:ABC-token"));
        // Reabrir lê a mesma chave do disco.
        let c2 = Cofre::abrir_em(&dir).unwrap();
        assert_eq!(c2.decifrar(&g).as_deref(), Some("123456:ABC-token"));
        assert_eq!(c2.decifrar("enc:lixo"), None);
        assert_eq!(c2.decifrar("texto puro"), None);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let m = std::fs::metadata(dir.join(ARQUIVO))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(m & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
