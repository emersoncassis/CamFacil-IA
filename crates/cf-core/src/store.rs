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
            "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
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
             CREATE TABLE IF NOT EXISTS cameras (
                 id     INTEGER PRIMARY KEY,
                 nome   TEXT NOT NULL,
                 fonte  TEXT NOT NULL,
                 url    TEXT NOT NULL DEFAULT '',
                 ativa  INTEGER NOT NULL DEFAULT 1
             );
             CREATE TABLE IF NOT EXISTS regras (
                 id         INTEGER PRIMARY KEY,
                 camera_id  INTEGER NOT NULL REFERENCES cameras(id) ON DELETE CASCADE,
                 nome       TEXT NOT NULL,
                 forma      TEXT NOT NULL,
                 pontos     TEXT NOT NULL,
                 rotulo_on  TEXT NOT NULL DEFAULT '',
                 rotulo_off TEXT NOT NULL DEFAULT ''
             );
             CREATE TABLE IF NOT EXISTS eventos (
                 id        INTEGER PRIMARY KEY,
                 camera_id INTEGER NOT NULL,
                 regra_id  INTEGER,
                 tipo      TEXT NOT NULL,
                 detalhe   TEXT NOT NULL DEFAULT '',
                 quando    INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS eventos_quando ON eventos(quando DESC);
             CREATE TABLE IF NOT EXISTS selos (
                 modulo   TEXT PRIMARY KEY,
                 ganho_em INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS telegram_chats (
                 chat_id    INTEGER PRIMARY KEY,
                 nome       TEXT NOT NULL DEFAULT '',
                 autorizado INTEGER NOT NULL DEFAULT 0,
                 visto_em   INTEGER NOT NULL
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

// ---------------- Câmeras, regras, eventos e selos ----------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct Camera {
    pub id: i64,
    pub nome: String,
    /// `rtsp`, `demo`, `arquivo` ou `webcam`.
    pub fonte: String,
    pub url: String,
    pub ativa: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RegraSalva {
    pub id: i64,
    pub camera_id: i64,
    pub nome: String,
    pub forma: String,
    pub pontos: Vec<crate::visao::Ponto>,
    pub rotulo_on: String,
    pub rotulo_off: String,
}

impl RegraSalva {
    pub fn para_visao(&self) -> Option<crate::visao::Regra> {
        let forma = match self.forma.as_str() {
            "zona" => crate::visao::Forma::Zona,
            "linha" => crate::visao::Forma::Linha,
            _ => return None,
        };
        Some(crate::visao::Regra {
            id: self.id,
            nome: self.nome.clone(),
            forma,
            pontos: self.pontos.clone(),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EventoSalvo {
    pub id: i64,
    pub camera_id: i64,
    pub regra_id: Option<i64>,
    pub tipo: String,
    pub detalhe: String,
    pub quando: i64,
}

impl Store {
    pub fn cameras(&self) -> Result<Vec<Camera>> {
        let c = self.lock();
        let mut st = c.prepare("SELECT id, nome, fonte, url, ativa FROM cameras ORDER BY id")?;
        let v = st
            .query_map([], |r| {
                Ok(Camera {
                    id: r.get(0)?,
                    nome: r.get(1)?,
                    fonte: r.get(2)?,
                    url: r.get(3)?,
                    ativa: r.get::<_, i64>(4)? != 0,
                })
            })?
            .collect();
        v
    }

    pub fn camera(&self, id: i64) -> Result<Option<Camera>> {
        Ok(self.cameras()?.into_iter().find(|c| c.id == id))
    }

    pub fn criar_camera(&self, nome: &str, fonte: &str, url: &str) -> Result<i64> {
        let c = self.lock();
        c.execute(
            "INSERT INTO cameras(nome, fonte, url) VALUES (?1, ?2, ?3)",
            params![nome, fonte, url],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn remover_camera(&self, id: i64) -> Result<bool> {
        let c = self.lock();
        c.execute("DELETE FROM regras WHERE camera_id = ?1", params![id])?;
        Ok(c.execute("DELETE FROM cameras WHERE id = ?1", params![id])? > 0)
    }

    pub fn regras(&self, camera_id: i64) -> Result<Vec<RegraSalva>> {
        let c = self.lock();
        let mut st = c.prepare(
            "SELECT id, camera_id, nome, forma, pontos, rotulo_on, rotulo_off FROM regras WHERE camera_id = ?1 ORDER BY id",
        )?;
        let v = st
            .query_map(params![camera_id], |r| {
                let pontos: String = r.get(4)?;
                Ok(RegraSalva {
                    id: r.get(0)?,
                    camera_id: r.get(1)?,
                    nome: r.get(2)?,
                    forma: r.get(3)?,
                    pontos: serde_json::from_str(&pontos).unwrap_or_default(),
                    rotulo_on: r.get(5)?,
                    rotulo_off: r.get(6)?,
                })
            })?
            .collect();
        v
    }

    #[allow(clippy::too_many_arguments)]
    pub fn criar_regra(
        &self,
        camera_id: i64,
        nome: &str,
        forma: &str,
        pontos: &[crate::visao::Ponto],
        rotulo_on: &str,
        rotulo_off: &str,
    ) -> Result<i64> {
        let c = self.lock();
        c.execute(
            "INSERT INTO regras(camera_id, nome, forma, pontos, rotulo_on, rotulo_off) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                camera_id,
                nome,
                forma,
                serde_json::to_string(pontos).unwrap_or_default(),
                rotulo_on,
                rotulo_off
            ],
        )?;
        Ok(c.last_insert_rowid())
    }

    pub fn editar_regra(
        &self,
        id: i64,
        nome: &str,
        rotulo_on: &str,
        rotulo_off: &str,
    ) -> Result<bool> {
        Ok(self.lock().execute(
            "UPDATE regras SET nome = ?2, rotulo_on = ?3, rotulo_off = ?4 WHERE id = ?1",
            params![id, nome, rotulo_on, rotulo_off],
        )? > 0)
    }

    pub fn remover_regra(&self, id: i64) -> Result<bool> {
        Ok(self
            .lock()
            .execute("DELETE FROM regras WHERE id = ?1", params![id])?
            > 0)
    }

    pub fn registrar_evento(
        &self,
        camera_id: i64,
        regra_id: Option<i64>,
        tipo: &str,
        detalhe: &str,
        quando: i64,
    ) -> Result<()> {
        let c = self.lock();
        c.execute(
            "INSERT INTO eventos(camera_id, regra_id, tipo, detalhe, quando) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![camera_id, regra_id, tipo, detalhe, quando],
        )?;
        // Retenção: guarda só os 5.000 eventos mais recentes.
        c.execute(
            "DELETE FROM eventos WHERE id <= (SELECT id FROM eventos ORDER BY id DESC LIMIT 1 OFFSET 5000)",
            [],
        )?;
        Ok(())
    }

    pub fn eventos(&self, limite: i64) -> Result<Vec<EventoSalvo>> {
        let c = self.lock();
        let mut st = c.prepare(
            "SELECT id, camera_id, regra_id, tipo, detalhe, quando FROM eventos ORDER BY id DESC LIMIT ?1",
        )?;
        let v = st
            .query_map(params![limite], |r| {
                Ok(EventoSalvo {
                    id: r.get(0)?,
                    camera_id: r.get(1)?,
                    regra_id: r.get(2)?,
                    tipo: r.get(3)?,
                    detalhe: r.get(4)?,
                    quando: r.get(5)?,
                })
            })?
            .collect();
        v
    }

    /// Últimos eventos de um tipo (ex.: `saude`), do mais novo para o mais velho.
    pub fn eventos_tipo(&self, tipo: &str, limite: i64) -> Result<Vec<EventoSalvo>> {
        let c = self.lock();
        let mut st = c.prepare(
            "SELECT id, camera_id, regra_id, tipo, detalhe, quando FROM eventos WHERE tipo = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let v = st
            .query_map(params![tipo, limite], |r| {
                Ok(EventoSalvo {
                    id: r.get(0)?,
                    camera_id: r.get(1)?,
                    regra_id: r.get(2)?,
                    tipo: r.get(3)?,
                    detalhe: r.get(4)?,
                    quando: r.get(5)?,
                })
            })?
            .collect();
        v
    }

    pub fn ganhar_selo(&self, modulo: &str, quando: i64) -> Result<bool> {
        Ok(self.lock().execute(
            "INSERT OR IGNORE INTO selos(modulo, ganho_em) VALUES (?1, ?2)",
            params![modulo, quando],
        )? > 0)
    }

    pub fn selos(&self) -> Result<Vec<(String, i64)>> {
        let c = self.lock();
        let mut st = c.prepare("SELECT modulo, ganho_em FROM selos ORDER BY ganho_em")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect();
        v
    }
}

// ---------------- Telegram: contatos ----------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChatTelegram {
    pub chat_id: i64,
    pub nome: String,
    pub autorizado: bool,
    pub visto_em: i64,
}

impl Store {
    /// Alguém falou com o bot: registra (ou atualiza nome e data) sem autorizar.
    pub fn telegram_chat_visto(&self, chat_id: i64, nome: &str, quando: i64) -> Result<()> {
        self.lock().execute(
            "INSERT INTO telegram_chats(chat_id, nome, visto_em) VALUES (?1, ?2, ?3)
             ON CONFLICT(chat_id) DO UPDATE SET nome = excluded.nome, visto_em = excluded.visto_em",
            params![chat_id, nome, quando],
        )?;
        Ok(())
    }

    pub fn telegram_chats(&self) -> Result<Vec<ChatTelegram>> {
        let c = self.lock();
        let mut st = c.prepare(
            "SELECT chat_id, nome, autorizado, visto_em FROM telegram_chats ORDER BY autorizado DESC, visto_em DESC",
        )?;
        let v = st
            .query_map([], |r| {
                Ok(ChatTelegram {
                    chat_id: r.get(0)?,
                    nome: r.get(1)?,
                    autorizado: r.get::<_, i64>(2)? != 0,
                    visto_em: r.get(3)?,
                })
            })?
            .collect();
        v
    }

    pub fn telegram_autorizados(&self) -> Result<Vec<i64>> {
        Ok(self
            .telegram_chats()?
            .into_iter()
            .filter(|c| c.autorizado)
            .map(|c| c.chat_id)
            .collect())
    }

    pub fn telegram_autorizar(&self, chat_id: i64, autorizado: bool) -> Result<bool> {
        Ok(self.lock().execute(
            "UPDATE telegram_chats SET autorizado = ?2 WHERE chat_id = ?1",
            params![chat_id, autorizado as i64],
        )? > 0)
    }

    pub fn telegram_remover(&self, chat_id: i64) -> Result<bool> {
        Ok(self.lock().execute(
            "DELETE FROM telegram_chats WHERE chat_id = ?1",
            params![chat_id],
        )? > 0)
    }

    /// Eventos desde um instante (para o /resumo do dia).
    pub fn eventos_desde(&self, desde: i64) -> Result<Vec<EventoSalvo>> {
        let c = self.lock();
        let mut st = c.prepare(
            "SELECT id, camera_id, regra_id, tipo, detalhe, quando FROM eventos WHERE quando >= ?1 ORDER BY id DESC LIMIT 5000",
        )?;
        let v = st
            .query_map(params![desde], |r| {
                Ok(EventoSalvo {
                    id: r.get(0)?,
                    camera_id: r.get(1)?,
                    regra_id: r.get(2)?,
                    tipo: r.get(3)?,
                    detalhe: r.get(4)?,
                    quando: r.get(5)?,
                })
            })?
            .collect();
        v
    }
}

// ---------------- Backup da configuração ----------------

/// Configuração exportável: tudo o que o técnico montou, sem histórico de
/// eventos e sem o que é desta máquina (identidade, porta, aparelhos
/// pareados). Os valores vão como estão no banco; quem chama trata segredos.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ConfigBackup {
    pub settings: Vec<(String, String)>,
    pub cameras: Vec<CameraBackup>,
    pub regras: Vec<RegraBackup>,
    pub telegram: Vec<TelegramBackup>,
    pub selos: Vec<(String, i64)>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CameraBackup {
    pub id: i64,
    pub nome: String,
    pub fonte: String,
    pub url: String,
    pub ativa: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegraBackup {
    pub id: i64,
    pub camera_id: i64,
    pub nome: String,
    pub forma: String,
    /// JSON dos pontos, como gravado.
    pub pontos: String,
    pub rotulo_on: String,
    pub rotulo_off: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TelegramBackup {
    pub chat_id: i64,
    pub nome: String,
    pub autorizado: bool,
}

/// Chaves de configuração que são desta máquina e não viajam no backup.
pub fn chave_local(k: &str) -> bool {
    k == "port" || k.starts_with("elo_") || k.starts_with("bench_") || k == "estresse_resultado"
}

impl Store {
    pub fn exportar_config(&self) -> Result<ConfigBackup> {
        let c = self.lock();
        let mut b = ConfigBackup::default();
        let mut st = c.prepare("SELECT key, value FROM settings ORDER BY key")?;
        b.settings = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<Vec<(String, String)>>>()?
            .into_iter()
            .filter(|(k, _)| !chave_local(k))
            .collect();
        let mut st = c.prepare("SELECT id, nome, fonte, url, ativa FROM cameras ORDER BY id")?;
        b.cameras = st
            .query_map([], |r| {
                Ok(CameraBackup {
                    id: r.get(0)?,
                    nome: r.get(1)?,
                    fonte: r.get(2)?,
                    url: r.get(3)?,
                    ativa: r.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<Result<_>>()?;
        let mut st = c.prepare(
            "SELECT id, camera_id, nome, forma, pontos, rotulo_on, rotulo_off FROM regras ORDER BY id",
        )?;
        b.regras = st
            .query_map([], |r| {
                Ok(RegraBackup {
                    id: r.get(0)?,
                    camera_id: r.get(1)?,
                    nome: r.get(2)?,
                    forma: r.get(3)?,
                    pontos: r.get(4)?,
                    rotulo_on: r.get(5)?,
                    rotulo_off: r.get(6)?,
                })
            })?
            .collect::<Result<_>>()?;
        let mut st =
            c.prepare("SELECT chat_id, nome, autorizado FROM telegram_chats ORDER BY chat_id")?;
        b.telegram = st
            .query_map([], |r| {
                Ok(TelegramBackup {
                    chat_id: r.get(0)?,
                    nome: r.get(1)?,
                    autorizado: r.get::<_, i64>(2)? != 0,
                })
            })?
            .collect::<Result<_>>()?;
        let mut st = c.prepare("SELECT modulo, ganho_em FROM selos ORDER BY modulo")?;
        b.selos = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_>>()?;
        Ok(b)
    }

    /// Substitui a configuração pela do backup, numa transação só (ou entra
    /// tudo ou nada). Mantém eventos, aceites e o que é desta máquina; eventos
    /// de câmeras que deixaram de existir ficam como histórico.
    pub fn importar_config(&self, b: &ConfigBackup, quando: i64) -> Result<()> {
        let mut c = self.lock();
        let tx = c.transaction()?;
        let locais: Vec<(String, String)> = {
            let mut st = tx.prepare("SELECT key, value FROM settings")?;
            let v = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<Result<Vec<(String, String)>>>()?;
            v.into_iter().filter(|(k, _)| chave_local(k)).collect()
        };
        tx.execute_batch(
            "DELETE FROM regras; DELETE FROM cameras; DELETE FROM settings;
             DELETE FROM telegram_chats; DELETE FROM selos;",
        )?;
        for (k, v) in locais
            .iter()
            .chain(b.settings.iter().filter(|(k, _)| !chave_local(k)))
        {
            tx.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![k, v],
            )?;
        }
        for k in &b.cameras {
            tx.execute(
                "INSERT INTO cameras (id, nome, fonte, url, ativa) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![k.id, k.nome, k.fonte, k.url, k.ativa as i64],
            )?;
        }
        for r in &b.regras {
            tx.execute(
                "INSERT INTO regras (id, camera_id, nome, forma, pontos, rotulo_on, rotulo_off)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    r.id,
                    r.camera_id,
                    r.nome,
                    r.forma,
                    r.pontos,
                    r.rotulo_on,
                    r.rotulo_off
                ],
            )?;
        }
        for t in &b.telegram {
            tx.execute(
                "INSERT INTO telegram_chats (chat_id, nome, autorizado, visto_em) VALUES (?1, ?2, ?3, ?4)",
                params![t.chat_id, t.nome, t.autorizado as i64, quando],
            )?;
        }
        for (m, q) in &b.selos {
            tx.execute(
                "INSERT INTO selos (modulo, ganho_em) VALUES (?1, ?2)",
                params![m, q],
            )?;
        }
        tx.commit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contatos_do_telegram() {
        let s = Store::memory().unwrap();
        s.telegram_chat_visto(10, "Zé", 1).unwrap();
        s.telegram_chat_visto(10, "Zé Técnico", 2).unwrap();
        assert!(s.telegram_autorizados().unwrap().is_empty());
        assert!(s.telegram_autorizar(10, true).unwrap());
        assert_eq!(s.telegram_autorizados().unwrap(), vec![10]);
        assert_eq!(s.telegram_chats().unwrap()[0].nome, "Zé Técnico");
        assert!(!s.telegram_autorizar(99, true).unwrap());
        assert!(s.telegram_remover(10).unwrap());
        assert!(s.telegram_chats().unwrap().is_empty());
    }

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
    fn cameras_regras_eventos() {
        use crate::visao::Ponto;
        let s = Store::memory().unwrap();
        let cam = s.criar_camera("Portão", "demo", "").unwrap();
        let pts = [Ponto { x: 0.1, y: 0.2 }, Ponto { x: 0.5, y: 0.6 }];
        let r = s.criar_regra(cam, "Porta", "linha", &pts, "", "").unwrap();
        let rs = s.regras(cam).unwrap();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].pontos, pts.to_vec());
        assert!(rs[0].para_visao().is_some());
        assert!(s.editar_regra(r, "Porta da frente", "", "").unwrap());
        s.registrar_evento(cam, Some(r), "linha_cruzada", "", 10)
            .unwrap();
        assert_eq!(s.eventos(10).unwrap()[0].tipo, "linha_cruzada");
        s.registrar_evento(cam, None, "saude", "escura", 11)
            .unwrap();
        let sa = s.eventos_tipo("saude", 10).unwrap();
        assert_eq!(sa.len(), 1);
        assert_eq!(sa[0].detalhe, "escura");
        assert!(s.ganhar_selo("cerca", 1).unwrap());
        assert!(!s.ganhar_selo("cerca", 2).unwrap());
        assert!(s.remover_camera(cam).unwrap());
        assert!(s.regras(cam).unwrap().is_empty());
    }

    #[test]
    fn aceite_por_versao() {
        let s = Store::memory().unwrap();
        assert!(!s.accepted("isencao", "1.0").unwrap());
        s.accept("isencao", "1.0", 1_700_000_000).unwrap();
        assert!(s.accepted("isencao", "1.0").unwrap());
        assert!(!s.accepted("isencao", "2.0").unwrap());
    }

    #[test]
    fn backup_ida_e_volta() {
        use crate::visao::Ponto;
        let a = Store::memory().unwrap();
        a.set("port", "7070").unwrap();
        a.set("elo_chave", "segredo-da-maquina").unwrap();
        a.set("tv_config", "{}").unwrap();
        let cam = a.criar_camera("Portão", "demo", "").unwrap();
        let pts = [Ponto { x: 0.1, y: 0.2 }, Ponto { x: 0.5, y: 0.6 }];
        a.criar_regra(cam, "Porta", "linha", &pts, "", "").unwrap();
        a.telegram_chat_visto(42, "Dono", 1).unwrap();
        a.telegram_autorizar(42, true).unwrap();
        let b = a.exportar_config().unwrap();
        assert!(b.settings.iter().all(|(k, _)| !chave_local(k)));

        let dest = Store::memory().unwrap();
        dest.set("port", "8080").unwrap();
        dest.criar_camera("Velha", "demo", "").unwrap();
        dest.importar_config(&b, 5).unwrap();
        assert_eq!(dest.get("port").unwrap().as_deref(), Some("8080"));
        assert_eq!(dest.get("elo_chave").unwrap(), None);
        assert_eq!(dest.get("tv_config").unwrap().as_deref(), Some("{}"));
        let cams = dest.cameras().unwrap();
        assert_eq!(cams.len(), 1);
        assert_eq!(cams[0].nome, "Portão");
        assert_eq!(dest.regras(cams[0].id).unwrap()[0].pontos, pts.to_vec());
        assert_eq!(dest.telegram_autorizados().unwrap(), vec![42]);
    }
}
