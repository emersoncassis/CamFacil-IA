//! Armazenamento local em SQLite: configurações e resultados.
//!
//! Tudo fica na máquina do cliente (offline-first). O arquivo entra no backup.

use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

pub struct Store {
    conn: Mutex<Connection>,
}

pub type Result<T> = std::result::Result<T, rusqlite::Error>;

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::init(Connection::open(path)?)
    }

    pub fn memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             CREATE TABLE IF NOT EXISTS settings (
                 key   TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS dispositivos_autorizados (
                 chave_publica TEXT PRIMARY KEY,
                 tecnico_email TEXT NOT NULL,
                 adicionado_em INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS acceptances (
                 id         INTEGER PRIMARY KEY,
                 document   TEXT NOT NULL,
                 version    TEXT NOT NULL,
                 accepted_at INTEGER NOT NULL
             );",
        )?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self, key: &str) -> Result<Option<String>> {
        self.lock()
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn set(&self, key: &str, value: &str) -> Result<()> {
        self.lock().execute(
            "INSERT INTO settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Registra o aceite de um termo (data, hora e versão), exigido pela LGPD do projeto.
    pub fn accept(&self, document: &str, version: &str, at_unix: i64) -> Result<()> {
        self.lock().execute(
            "INSERT INTO acceptances(document, version, accepted_at) VALUES(?1, ?2, ?3)",
            params![document, version, at_unix],
        )?;
        Ok(())
    }

    /// Diz se a versão indicada do termo já foi aceita.
    pub fn accepted(&self, document: &str, version: &str) -> Result<bool> {
        let n: i64 = self.lock().query_row(
            "SELECT COUNT(*) FROM acceptances WHERE document = ?1 AND version = ?2",
            params![document, version],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }
}

impl Store {
    /// Autoriza um navegador do técnico (feito só após confirmação na tela local).
    pub fn autorizar_dispositivo(&self, chave: &str, email: &str, at_unix: i64) -> Result<()> {
        self.lock().execute(
            "INSERT INTO dispositivos_autorizados(chave_publica, tecnico_email, adicionado_em)
             VALUES (?1, ?2, ?3) ON CONFLICT(chave_publica) DO UPDATE SET tecnico_email = excluded.tecnico_email",
            params![chave, email, at_unix],
        )?;
        Ok(())
    }

    pub fn revogar_dispositivo(&self, chave: &str) -> Result<()> {
        self.lock().execute(
            "DELETE FROM dispositivos_autorizados WHERE chave_publica = ?1",
            params![chave],
        )?;
        Ok(())
    }

    pub fn dispositivo_autorizado(&self, chave: &str) -> Result<bool> {
        let n: i64 = self.lock().query_row(
            "SELECT COUNT(*) FROM dispositivos_autorizados WHERE chave_publica = ?1",
            params![chave],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Desparear: esquece todos os navegadores autorizados.
    pub fn limpar_dispositivos(&self) -> Result<()> {
        self.lock()
            .execute("DELETE FROM dispositivos_autorizados", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grava_e_le_configuracao() {
        let s = Store::memory().unwrap();
        assert_eq!(s.get("port").unwrap(), None);
        s.set("port", "7070").unwrap();
        s.set("port", "7071").unwrap();
        assert_eq!(s.get("port").unwrap().as_deref(), Some("7071"));
    }

    #[test]
    fn dispositivos_autorizados() {
        let s = Store::memory().unwrap();
        assert!(!s.dispositivo_autorizado("k").unwrap());
        s.autorizar_dispositivo("k", "a@b", 1).unwrap();
        assert!(s.dispositivo_autorizado("k").unwrap());
        s.revogar_dispositivo("k").unwrap();
        assert!(!s.dispositivo_autorizado("k").unwrap());
        s.autorizar_dispositivo("k", "a@b", 1).unwrap();
        s.limpar_dispositivos().unwrap();
        assert!(!s.dispositivo_autorizado("k").unwrap());
    }

    #[test]
    fn aceite_por_versao() {
        let s = Store::memory().unwrap();
        assert!(!s.accepted("isencao", "1.0").unwrap());
        s.accept("isencao", "1.0", 1_700_000_000).unwrap();
        assert!(s.accepted("isencao", "1.0").unwrap());
        assert!(!s.accepted("isencao", "2.0").unwrap());
    }
}
