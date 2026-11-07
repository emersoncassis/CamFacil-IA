//! Acesso ao Postgres com isolamento por técnico (SDD-08, "Isolamento").
//!
//! Toda transação troca para o papel `cfia_app` (sujeito à Row-Level Security)
//! e declara o contexto: um técnico ou o próprio sistema.

use deadpool_postgres::{Manager, ManagerConfig, Object, Pool, RecyclingMethod, Transaction};
use tokio_postgres::NoTls;
use uuid::Uuid;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

const MIGRACOES: &[(&str, &str)] = &[(
    "0001_inicial",
    include_str!("../migrations/0001_inicial.sql"),
)];

/// Contexto de uma transação.
#[derive(Debug, Clone, Copy)]
pub enum Ctx {
    /// Técnico logado: a RLS só deixa ver e alterar o que é dele.
    Tecnico(Uuid),
    /// O próprio servidor (validar sessão, falar com agentes, pareamento).
    Sistema,
}

pub fn pool(url: &str) -> Result<Pool> {
    let cfg: tokio_postgres::Config = url.parse()?;
    let mgr = Manager::from_config(
        cfg,
        NoTls,
        ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        },
    );
    Ok(Pool::builder(mgr).max_size(16).build()?)
}

/// Aplica as migrações que ainda não rodaram (como o usuário da conexão).
pub async fn migrate(pool: &Pool) -> Result<()> {
    let mut c = pool.get().await?;
    // Trava no banco: várias instâncias subindo juntas não migram ao mesmo tempo.
    c.batch_execute("SELECT pg_advisory_lock(727001)").await?;
    let r = aplicar(&mut c).await;
    c.batch_execute("SELECT pg_advisory_unlock(727001)").await?;
    r
}

async fn aplicar(c: &mut Object) -> Result<()> {
    c.batch_execute(
        "CREATE TABLE IF NOT EXISTS _migracoes (nome text PRIMARY KEY, aplicada_em timestamptz NOT NULL DEFAULT now())",
    )
    .await?;
    for (nome, sql) in MIGRACOES {
        let tx = c.transaction().await?;
        let ja: i64 = tx
            .query_one("SELECT count(*) FROM _migracoes WHERE nome = $1", &[nome])
            .await?
            .get(0);
        if ja == 0 {
            tx.batch_execute(sql).await?;
            tx.execute("INSERT INTO _migracoes(nome) VALUES ($1)", &[nome])
                .await?;
        }
        tx.commit().await?;
    }
    Ok(())
}

/// Abre uma transação no papel `cfia_app` com o contexto indicado.
pub async fn begin(client: &mut Object, ctx: Ctx) -> Result<Transaction<'_>> {
    let tx = client.transaction().await?;
    tx.batch_execute("SET LOCAL ROLE cfia_app").await?;
    match ctx {
        Ctx::Tecnico(id) => {
            tx.execute(
                "SELECT set_config('app.tecnico_id', $1, true), set_config('app.papel', '', true)",
                &[&id.to_string()],
            )
            .await?;
        }
        Ctx::Sistema => {
            tx.batch_execute(
                "SELECT set_config('app.papel', 'sistema', true), set_config('app.tecnico_id', '', true)",
            )
            .await?;
        }
    }
    Ok(tx)
}

/// Registra um evento na auditoria.
pub async fn auditar(
    tx: &Transaction<'_>,
    tecnico: Option<Uuid>,
    tipo: &str,
    alvo: &str,
    ip: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO auditoria(tecnico_id, tipo, alvo, ip) VALUES ($1, $2, $3, $4)",
        &[&tecnico, &tipo, &alvo, &ip],
    )
    .await?;
    Ok(())
}
