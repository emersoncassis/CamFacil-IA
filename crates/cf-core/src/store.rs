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
             );
             CREATE TABLE IF NOT EXISTS whatsapp_contatos (
                 jid        TEXT PRIMARY KEY,
                 nome       TEXT NOT NULL DEFAULT '',
                 autorizado INTEGER NOT NULL DEFAULT 0,
                 visto_em   INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS arquivo_alertas (
                 id         INTEGER PRIMARY KEY,
                 camera_id  INTEGER NOT NULL,
                 regra_id   INTEGER,
                 chave      TEXT NOT NULL,
                 quando     INTEGER NOT NULL,
                 arquivo    TEXT NOT NULL DEFAULT '',
                 chat_id    INTEGER,
                 message_id INTEGER
             );
             CREATE INDEX IF NOT EXISTS arquivo_quando ON arquivo_alertas(camera_id, quando DESC);",
        )?;
        migrar_cameras(&conn)?;
        migrar_macs_antigos(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub(crate) fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
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

/// Chave antiga em `settings` com o MAC aprendido por câmera
/// (`{"7":"58:10:8C:…"}`). Hoje o MAC mora na coluna `cameras.mac`; a chave
/// só aparece em bancos e backups de antes da autocorreção de IP.
pub const CHAVE_MACS_ANTIGA: &str = "rede.macs";

/// Acrescenta as colunas novas de `cameras` em bancos antigos (SQLite não tem
/// `ADD COLUMN IF NOT EXISTS`, então confere pela `table_info`).
fn migrar_cameras(c: &Connection) -> Result<()> {
    let colunas: Vec<String> = {
        let mut st = c.prepare("PRAGMA table_info(cameras)")?;
        let v = st
            .query_map([], |r| r.get::<_, String>(1))?
            .collect::<Result<_>>()?;
        v
    };
    if !colunas.iter().any(|n| n == "auto_ip") {
        c.execute_batch("ALTER TABLE cameras ADD COLUMN auto_ip INTEGER NOT NULL DEFAULT 0;")?;
    }
    if !colunas.iter().any(|n| n == "mac") {
        c.execute_batch("ALTER TABLE cameras ADD COLUMN mac TEXT NOT NULL DEFAULT '';")?;
    }
    if !colunas.iter().any(|n| n == "gravador") {
        c.execute_batch("ALTER TABLE cameras ADD COLUMN gravador TEXT NOT NULL DEFAULT '';")?;
    }
    if !colunas.iter().any(|n| n == "canal") {
        c.execute_batch("ALTER TABLE cameras ADD COLUMN canal INTEGER NOT NULL DEFAULT 0;")?;
    }
    Ok(())
}

/// Leva os MACs da chave antiga `rede.macs` para a coluna `cameras.mac` (só
/// onde a coluna está vazia) e apaga a chave. Serve ao abrir o banco e ao
/// importar backup antigo.
fn migrar_macs_antigos(c: &Connection) -> Result<()> {
    let guardado: Option<String> = c
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![CHAVE_MACS_ANTIGA],
            |r| r.get(0),
        )
        .optional()?;
    let Some(json) = guardado else {
        return Ok(());
    };
    let mapa: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&json).unwrap_or_default();
    for (id, mac) in mapa {
        if let (Ok(id), Some(mac)) = (id.parse::<i64>(), crate::rede::normalizar_mac(&mac)) {
            c.execute(
                "UPDATE cameras SET mac = ?2 WHERE id = ?1 AND mac = ''",
                params![id, mac],
            )?;
        }
    }
    c.execute(
        "DELETE FROM settings WHERE key = ?1",
        params![CHAVE_MACS_ANTIGA],
    )?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Camera {
    pub id: i64,
    pub nome: String,
    /// `rtsp`, `demo`, `arquivo` ou `webcam`.
    pub fonte: String,
    pub url: String,
    pub ativa: bool,
    /// Autocorreção de IP pelo MAC (só RTSP). Padrão desligado.
    pub auto_ip: bool,
    /// MAC aprendido na rede (`AA:BB:…`); vazio enquanto não aprendeu.
    pub mac: String,
    /// Host do gravador (DVR/NVR) de onde a câmera veio; vazio se é câmera IP.
    pub gravador: String,
    /// Canal do gravador (1, 2, …); 0 quando não veio de gravador.
    pub canal: u32,
}

impl Camera {
    pub fn mac_conhecido(&self) -> bool {
        !self.mac.is_empty()
    }
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
        let mut st =
            c.prepare("SELECT id, nome, fonte, url, ativa, auto_ip, mac, gravador, canal FROM cameras ORDER BY id")?;
        let v = st
            .query_map([], |r| {
                Ok(Camera {
                    id: r.get(0)?,
                    nome: r.get(1)?,
                    fonte: r.get(2)?,
                    url: r.get(3)?,
                    ativa: r.get::<_, i64>(4)? != 0,
                    auto_ip: r.get::<_, i64>(5)? != 0,
                    mac: r.get(6)?,
                    gravador: r.get(7)?,
                    canal: r.get::<_, i64>(8)?.max(0) as u32,
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

    /// Grava nome, fonte, endereço, se está ativa e a autocorreção de IP
    /// (o MAC não: ele só muda pela rede, em [`Store::definir_mac_camera`]).
    /// `false` se a câmera não existe.
    pub fn atualizar_camera(&self, cam: &Camera) -> Result<bool> {
        let c = self.lock();
        Ok(c.execute(
            "UPDATE cameras SET nome = ?2, fonte = ?3, url = ?4, ativa = ?5, auto_ip = ?6 WHERE id = ?1",
            params![
                cam.id,
                cam.nome,
                cam.fonte,
                cam.url,
                cam.ativa as i64,
                cam.auto_ip as i64
            ],
        )? > 0)
    }

    /// Marca a câmera como canal de um gravador (host vazio e canal 0 desfazem).
    pub fn definir_gravador_canal(&self, id: i64, gravador: &str, canal: u32) -> Result<bool> {
        Ok(self.lock().execute(
            "UPDATE cameras SET gravador = ?2, canal = ?3 WHERE id = ?1",
            params![id, gravador, canal],
        )? > 0)
    }

    /// Liga ou desliga a autocorreção de IP de uma câmera.
    pub fn definir_auto_ip(&self, id: i64, ligado: bool) -> Result<bool> {
        Ok(self.lock().execute(
            "UPDATE cameras SET auto_ip = ?2 WHERE id = ?1",
            params![id, ligado as i64],
        )? > 0)
    }

    /// Guarda o MAC aprendido da câmera (normalizado; vazio apaga).
    pub fn definir_mac_camera(&self, id: i64, mac: &str) -> Result<bool> {
        let mac = crate::rede::normalizar_mac(mac).unwrap_or_default();
        Ok(self.lock().execute(
            "UPDATE cameras SET mac = ?2 WHERE id = ?1",
            params![id, mac],
        )? > 0)
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
    /// Contatos do WhatsApp (fase 3b); ausente em backups antigos.
    #[serde(default)]
    pub whatsapp: Vec<WhatsAppBackup>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WhatsAppBackup {
    pub jid: String,
    pub nome: String,
    pub autorizado: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CameraBackup {
    pub id: i64,
    pub nome: String,
    pub fonte: String,
    pub url: String,
    pub ativa: bool,
    /// Autocorreção de IP; ausente em backups antigos (desligada).
    #[serde(default)]
    pub auto_ip: bool,
    /// MAC aprendido; ausente em backups antigos (o MAC vinha em `rede.macs`).
    #[serde(default)]
    pub mac: String,
    /// Gravador (host) e canal; ausentes em backups antigos.
    #[serde(default)]
    pub gravador: String,
    #[serde(default)]
    pub canal: u32,
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
    k == "port"
        || k.starts_with("elo_")
        || k.starts_with("bench_")
        || k == "estresse_resultado"
        // MCP liga por máquina: restaurar um backup não abre a porta para a IA.
        || k.starts_with("mcp_")
        // Registro das correções automáticas de IP: histórico, como os eventos.
        || k == "rede.autoip_log"
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
        let mut st =
            c.prepare("SELECT id, nome, fonte, url, ativa, auto_ip, mac, gravador, canal FROM cameras ORDER BY id")?;
        b.cameras = st
            .query_map([], |r| {
                Ok(CameraBackup {
                    id: r.get(0)?,
                    nome: r.get(1)?,
                    fonte: r.get(2)?,
                    url: r.get(3)?,
                    ativa: r.get::<_, i64>(4)? != 0,
                    auto_ip: r.get::<_, i64>(5)? != 0,
                    mac: r.get(6)?,
                    gravador: r.get(7)?,
                    canal: r.get::<_, i64>(8)?.max(0) as u32,
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
        let mut st =
            c.prepare("SELECT jid, nome, autorizado FROM whatsapp_contatos ORDER BY jid")?;
        b.whatsapp = st
            .query_map([], |r| {
                Ok(WhatsAppBackup {
                    jid: r.get(0)?,
                    nome: r.get(1)?,
                    autorizado: r.get::<_, i64>(2)? != 0,
                })
            })?
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
             DELETE FROM telegram_chats; DELETE FROM selos; DELETE FROM whatsapp_contatos;",
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
            let mac = crate::rede::normalizar_mac(&k.mac).unwrap_or_default();
            tx.execute(
                "INSERT INTO cameras (id, nome, fonte, url, ativa, auto_ip, mac, gravador, canal)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    k.id,
                    k.nome,
                    k.fonte,
                    k.url,
                    k.ativa as i64,
                    k.auto_ip as i64,
                    mac,
                    k.gravador,
                    k.canal
                ],
            )?;
        }
        // Backup de antes da autocorreção: o MAC vinha na chave `rede.macs`.
        migrar_macs_antigos(&tx)?;
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
        for w in &b.whatsapp {
            tx.execute(
                "INSERT INTO whatsapp_contatos (jid, nome, autorizado, visto_em) VALUES (?1, ?2, ?3, ?4)",
                params![w.jid, w.nome, w.autorizado as i64, quando],
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

// ---------------- WhatsApp: contatos (fase 3b) ----------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct ContatoWhatsApp {
    pub jid: String,
    pub nome: String,
    pub autorizado: bool,
    pub visto_em: i64,
}

impl Store {
    pub fn whatsapp_contato_visto(&self, jid: &str, nome: &str, quando: i64) -> Result<()> {
        self.lock().execute(
            "INSERT INTO whatsapp_contatos(jid, nome, visto_em) VALUES (?1, ?2, ?3)
             ON CONFLICT(jid) DO UPDATE SET nome = CASE WHEN excluded.nome = '' THEN nome ELSE excluded.nome END, visto_em = excluded.visto_em",
            params![jid, nome, quando],
        )?;
        Ok(())
    }

    pub fn whatsapp_contatos(&self) -> Result<Vec<ContatoWhatsApp>> {
        let c = self.lock();
        let mut st = c.prepare(
            "SELECT jid, nome, autorizado, visto_em FROM whatsapp_contatos ORDER BY autorizado DESC, visto_em DESC",
        )?;
        let v = st
            .query_map([], |r| {
                Ok(ContatoWhatsApp {
                    jid: r.get(0)?,
                    nome: r.get(1)?,
                    autorizado: r.get::<_, i64>(2)? != 0,
                    visto_em: r.get(3)?,
                })
            })?
            .collect();
        v
    }

    pub fn whatsapp_autorizados(&self) -> Result<Vec<String>> {
        Ok(self
            .whatsapp_contatos()?
            .into_iter()
            .filter(|c| c.autorizado)
            .map(|c| c.jid)
            .collect())
    }

    pub fn whatsapp_autorizar(&self, jid: &str, autorizado: bool) -> Result<bool> {
        Ok(self.lock().execute(
            "UPDATE whatsapp_contatos SET autorizado = ?2 WHERE jid = ?1",
            params![jid, autorizado as i64],
        )? > 0)
    }

    pub fn whatsapp_remover(&self, jid: &str) -> Result<bool> {
        Ok(self
            .lock()
            .execute("DELETE FROM whatsapp_contatos WHERE jid = ?1", params![jid])?
            > 0)
    }
}

// ---------------- Arquivo de alertas: foto local e mensagem no Telegram ----------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct AlertaArquivado {
    pub id: i64,
    pub camera_id: i64,
    pub regra_id: Option<i64>,
    pub chave: String,
    pub quando: i64,
    /// Nome do arquivo JPEG na pasta `fotos/cam_<id>/` (vazio se não houve foto).
    pub arquivo: String,
    pub chat_id: Option<i64>,
    pub message_id: Option<i64>,
}

impl Store {
    pub fn arquivar_alerta(
        &self,
        camera_id: i64,
        regra_id: Option<i64>,
        chave: &str,
        quando: i64,
        arquivo: &str,
    ) -> Result<i64> {
        let c = self.lock();
        c.execute(
            "INSERT INTO arquivo_alertas(camera_id, regra_id, chave, quando, arquivo) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![camera_id, regra_id, chave, quando, arquivo],
        )?;
        Ok(c.last_insert_rowid())
    }

    /// Guarda onde a foto ficou no Telegram (chat e mensagem) para o link "ver no arquivo".
    pub fn arquivo_mensagem(&self, id: i64, chat_id: i64, message_id: i64) -> Result<()> {
        self.lock().execute(
            "UPDATE arquivo_alertas SET chat_id = ?2, message_id = ?3 WHERE id = ?1",
            params![id, chat_id, message_id],
        )?;
        Ok(())
    }

    pub fn arquivo_alertas(&self, limite: i64) -> Result<Vec<AlertaArquivado>> {
        let c = self.lock();
        let mut st = c.prepare(
            "SELECT id, camera_id, regra_id, chave, quando, arquivo, chat_id, message_id FROM arquivo_alertas ORDER BY id DESC LIMIT ?1",
        )?;
        let v = st
            .query_map(params![limite], |r| {
                Ok(AlertaArquivado {
                    id: r.get(0)?,
                    camera_id: r.get(1)?,
                    regra_id: r.get(2)?,
                    chave: r.get(3)?,
                    quando: r.get(4)?,
                    arquivo: r.get(5)?,
                    chat_id: r.get(6)?,
                    message_id: r.get(7)?,
                })
            })?
            .collect();
        v
    }

    /// Um alerta do arquivo pelo id.
    pub fn arquivo_alerta(&self, id: i64) -> Result<Option<AlertaArquivado>> {
        self.lock()
            .query_row(
                "SELECT id, camera_id, regra_id, chave, quando, arquivo, chat_id, message_id FROM arquivo_alertas WHERE id = ?1",
                params![id],
                |r| {
                    Ok(AlertaArquivado {
                        id: r.get(0)?,
                        camera_id: r.get(1)?,
                        regra_id: r.get(2)?,
                        chave: r.get(3)?,
                        quando: r.get(4)?,
                        arquivo: r.get(5)?,
                        chat_id: r.get(6)?,
                        message_id: r.get(7)?,
                    })
                },
            )
            .optional()
    }

    /// Nomes dos arquivos de foto que saem da retenção de uma câmera (mantém os
    /// `manter` mais recentes) e já apaga as linhas correspondentes.
    pub fn arquivo_excedentes(&self, camera_id: i64, manter: i64) -> Result<Vec<String>> {
        let c = self.lock();
        let mut st = c.prepare(
            "SELECT id, arquivo FROM arquivo_alertas WHERE camera_id = ?1 AND arquivo <> '' ORDER BY id DESC LIMIT -1 OFFSET ?2",
        )?;
        let velhos: Vec<(i64, String)> = st
            .query_map(params![camera_id, manter], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_>>()?;
        drop(st);
        for (id, _) in &velhos {
            c.execute(
                "UPDATE arquivo_alertas SET arquivo = '' WHERE id = ?1",
                params![id],
            )?;
        }
        Ok(velhos.into_iter().map(|(_, a)| a).collect())
    }

    /// LGPD: apaga eventos e o índice do arquivo deste cliente. Devolve
    /// (eventos, alertas arquivados) apagados. Configurações e câmeras ficam.
    pub fn apagar_dados_cliente(&self) -> Result<(usize, usize)> {
        let c = self.lock();
        let e = c.execute("DELETE FROM eventos", [])?;
        let a = c.execute("DELETE FROM arquivo_alertas", [])?;
        Ok((e, a))
    }

    /// Último aceite de um documento (versão, quando), se houver.
    pub fn ultimo_aceite(&self, document: &str) -> Result<Option<(String, i64)>> {
        self.lock()
            .query_row(
                "SELECT version, accepted_at FROM acceptances WHERE document = ?1 ORDER BY id DESC LIMIT 1",
                params![document],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contatos_do_whatsapp_e_arquivo() {
        let s = Store::memory().unwrap();
        s.whatsapp_contato_visto("5511@s.whatsapp.net", "Zé", 1)
            .unwrap();
        s.whatsapp_contato_visto("5511@s.whatsapp.net", "", 2)
            .unwrap();
        assert_eq!(s.whatsapp_contatos().unwrap()[0].nome, "Zé");
        assert!(s.whatsapp_autorizados().unwrap().is_empty());
        assert!(s.whatsapp_autorizar("5511@s.whatsapp.net", true).unwrap());
        assert_eq!(s.whatsapp_autorizados().unwrap().len(), 1);
        assert!(s.whatsapp_remover("5511@s.whatsapp.net").unwrap());

        for i in 0..5 {
            s.arquivar_alerta(1, Some(7), "regra:7", 100 + i, &format!("f{i}.jpg"))
                .unwrap();
        }
        let id = s.arquivo_alertas(1).unwrap()[0].id;
        s.arquivo_mensagem(id, -100123, 55).unwrap();
        assert_eq!(s.arquivo_alertas(1).unwrap()[0].message_id, Some(55));
        let fora = s.arquivo_excedentes(1, 3).unwrap();
        assert_eq!(fora, vec!["f1.jpg", "f0.jpg"]);
        assert!(s.arquivo_excedentes(1, 3).unwrap().is_empty());
        s.registrar_evento(1, None, "saude", "escura", 11).unwrap();
        assert_eq!(s.apagar_dados_cliente().unwrap(), (1, 5));
        assert!(s.ultimo_aceite("tecnico").unwrap().is_none());
        s.accept("tecnico", "1.0", 5).unwrap();
        assert_eq!(s.ultimo_aceite("tecnico").unwrap(), Some(("1.0".into(), 5)));
    }

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
        assert!(dest.whatsapp_contatos().unwrap().is_empty());
    }

    #[test]
    fn backup_leva_auto_ip_e_mac() {
        let a = Store::memory().unwrap();
        let cam = a
            .criar_camera("Portão", "rtsp", "rtsp://a:b@192.168.1.108:554/x")
            .unwrap();
        a.definir_auto_ip(cam, true).unwrap();
        a.definir_mac_camera(cam, "58-10-8c-00-00-01").unwrap();
        a.definir_gravador_canal(cam, "192.168.1.108", 3).unwrap();
        let b = a.exportar_config().unwrap();
        let json = serde_json::to_string(&b).unwrap();
        let b: ConfigBackup = serde_json::from_str(&json).unwrap();
        let dest = Store::memory().unwrap();
        dest.importar_config(&b, 1).unwrap();
        let c = &dest.cameras().unwrap()[0];
        assert!(c.auto_ip);
        assert_eq!(c.mac, "58:10:8C:00:00:01");
        assert_eq!((c.gravador.as_str(), c.canal), ("192.168.1.108", 3));
    }

    #[test]
    fn backup_antigo_sem_auto_ip_traz_mac_da_chave_antiga() {
        // Backup da versão 0.6: câmera sem auto_ip/mac e MAC em `rede.macs`.
        let json = r#"{"settings":[["rede.macs","{\"3\":\"3C:EF:8C:00:00:02\"}"]],
            "cameras":[{"id":3,"nome":"Fundos","fonte":"rtsp","url":"rtsp://x@10.0.0.9/","ativa":true}],
            "regras":[],"telegram":[],"selos":[]}"#;
        let b: ConfigBackup = serde_json::from_str(json).unwrap();
        let dest = Store::memory().unwrap();
        dest.importar_config(&b, 1).unwrap();
        let c = &dest.cameras().unwrap()[0];
        assert!(!c.auto_ip);
        assert_eq!(c.mac, "3C:EF:8C:00:00:02");
        assert_eq!(dest.get(CHAVE_MACS_ANTIGA).unwrap(), None);
    }

    #[test]
    fn migra_banco_antigo_sem_colunas_novas() {
        let dir = std::env::temp_dir().join(format!("cfia-migra-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let arq = dir.join("antigo.db");
        let _ = std::fs::remove_file(&arq);
        {
            let c = Connection::open(&arq).unwrap();
            c.execute_batch(
                "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE cameras (id INTEGER PRIMARY KEY, nome TEXT NOT NULL,
                     fonte TEXT NOT NULL, url TEXT NOT NULL DEFAULT '', ativa INTEGER NOT NULL DEFAULT 1);
                 INSERT INTO cameras (id, nome, fonte, url) VALUES (7, 'Rua', 'rtsp', 'rtsp://u:p@192.168.0.50/s');
                 INSERT INTO settings VALUES ('rede.macs', '{\"7\":\"44:19:b6:00:00:07\"}');",
            )
            .unwrap();
        }
        let s = Store::open(&arq).unwrap();
        let c = &s.cameras().unwrap()[0];
        assert_eq!(c.id, 7);
        assert!(!c.auto_ip);
        assert_eq!(c.mac, "44:19:B6:00:00:07");
        assert_eq!(s.get(CHAVE_MACS_ANTIGA).unwrap(), None);
        drop(s);
        // Abrir de novo não quebra (migração idempotente).
        let s = Store::open(&arq).unwrap();
        assert_eq!(s.cameras().unwrap().len(), 1);
        drop(s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
