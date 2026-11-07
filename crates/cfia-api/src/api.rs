//! Rotas REST do técnico. Toda consulta roda no contexto do técnico da sessão:
//! a Row-Level Security garante que recurso de outro técnico "não existe" (404).

use crate::auth::Tecnico;
use crate::db::{self, Ctx};
use crate::hub::hash_codigo;
use crate::util::{fingerprint, B64};
use crate::{erro, interno, AppState, Falha};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use uuid::Uuid;

type Res = Result<Json<Value>, Falha>;

pub async fn saude() -> Json<Value> {
    Json(json!({"ok": true, "versao": env!("CARGO_PKG_VERSION")}))
}

pub async fn eu(t: Tecnico) -> Json<Value> {
    Json(json!({"id": t.id, "email": t.email, "nome": t.nome}))
}

// ---------------- Dispositivos ----------------

pub async fn listar_dispositivos(State(s): State<AppState>, t: Tecnico) -> Res {
    let mut c = s.db.get().await.map_err(interno)?;
    let tx = db::begin(&mut c, Ctx::Tecnico(t.id))
        .await
        .map_err(interno)?;
    let rows = tx
        .query(
            "SELECT id, apelido, chave_publica, criado_em::text, revogado_em IS NOT NULL
             FROM dispositivos ORDER BY criado_em DESC",
            &[],
        )
        .await
        .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    let lista: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<_, Uuid>(0),
                "apelido": r.get::<_, String>(1),
                "impressao": fingerprint(&r.get::<_, String>(2)),
                "criado_em": r.get::<_, String>(3),
                "revogado": r.get::<_, bool>(4),
            })
        })
        .collect();
    Ok(Json(json!({"dispositivos": lista})))
}

#[derive(Deserialize)]
pub struct NovoDispositivo {
    chave_publica: String,
    #[serde(default)]
    apelido: String,
}

/// Chave pública ECDSA P-256 em SPKI (91 bytes) codificada em base64.
fn chave_p256_valida(b64: &str) -> bool {
    matches!(B64.decode(b64), Ok(b) if b.len() == 91 && b.starts_with(&[0x30, 0x59, 0x30, 0x13]))
}

pub async fn criar_dispositivo(
    State(s): State<AppState>,
    t: Tecnico,
    headers: HeaderMap,
    Json(n): Json<NovoDispositivo>,
) -> Res {
    if !chave_p256_valida(&n.chave_publica) {
        return Err(erro(
            StatusCode::BAD_REQUEST,
            "chave do dispositivo inválida",
        ));
    }
    let apelido: String = n.apelido.chars().take(60).collect();
    let mut c = s.db.get().await.map_err(interno)?;
    let tx = db::begin(&mut c, Ctx::Tecnico(t.id))
        .await
        .map_err(interno)?;
    let id = Uuid::new_v4();
    tx.execute(
        "INSERT INTO dispositivos(id, tecnico_id, chave_publica, apelido) VALUES ($1, $2, $3, $4)",
        &[&id, &t.id, &n.chave_publica, &apelido],
    )
    .await
    .map_err(interno)?;
    db::auditar(
        &tx,
        Some(t.id),
        "dispositivo",
        &id.to_string(),
        &crate::ip(&headers),
    )
    .await
    .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    Ok(Json(
        json!({"id": id, "impressao": fingerprint(&n.chave_publica)}),
    ))
}

pub async fn revogar_dispositivo(
    State(s): State<AppState>,
    t: Tecnico,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Res {
    let mut c = s.db.get().await.map_err(interno)?;
    let tx = db::begin(&mut c, Ctx::Tecnico(t.id))
        .await
        .map_err(interno)?;
    let chave: Option<String> = tx
        .query_opt(
            "UPDATE dispositivos SET revogado_em = now() WHERE id = $1 AND revogado_em IS NULL RETURNING chave_publica",
            &[&id],
        )
        .await
        .map_err(interno)?
        .map(|r| r.get(0));
    let Some(chave) = chave else {
        return Err(erro(StatusCode::NOT_FOUND, "dispositivo não encontrado"));
    };
    let agentes: Vec<Uuid> = tx
        .query("SELECT id FROM agentes", &[])
        .await
        .map_err(interno)?
        .iter()
        .map(|r| r.get(0))
        .collect();
    db::auditar(
        &tx,
        Some(t.id),
        "revogacao",
        &id.to_string(),
        &crate::ip(&headers),
    )
    .await
    .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    // Os agentes on-line removem a chave da lista local na hora.
    for a in agentes {
        s.hub.para_agente(
            &a,
            json!({"tipo": "revogar_dispositivo", "chave_publica": chave}),
        );
    }
    Ok(Json(json!({"revogado": true})))
}

// ---------------- Agentes ----------------

pub async fn listar_agentes(State(s): State<AppState>, t: Tecnico) -> Res {
    let mut c = s.db.get().await.map_err(interno)?;
    let tx = db::begin(&mut c, Ctx::Tecnico(t.id))
        .await
        .map_err(interno)?;
    let rows = tx
        .query(
            "SELECT id, apelido_cliente, chave_publica, versao,
                    pareado_em::text, ultimo_contato::text, estado, estado_em::text
             FROM agentes ORDER BY pareado_em DESC NULLS LAST",
            &[],
        )
        .await
        .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    let lista: Vec<Value> = rows
        .iter()
        .map(|r| {
            let id: Uuid = r.get(0);
            json!({
                "id": id,
                "apelido_cliente": r.get::<_, String>(1),
                "impressao": fingerprint(&r.get::<_, String>(2)),
                "versao": r.get::<_, String>(3),
                "pareado_em": r.get::<_, Option<String>>(4),
                "ultimo_contato": r.get::<_, Option<String>>(5),
                "online": s.hub.online(&id),
                "estado": r.get::<_, Option<Value>>(6),
                "estado_em": r.get::<_, Option<String>>(7),
            })
        })
        .collect();
    Ok(Json(json!({"agentes": lista})))
}

#[derive(Deserialize)]
pub struct EditarAgente {
    apelido_cliente: String,
}

pub async fn editar_agente(
    State(s): State<AppState>,
    t: Tecnico,
    Path(id): Path<Uuid>,
    Json(e): Json<EditarAgente>,
) -> Res {
    let apelido: String = e.apelido_cliente.trim().chars().take(60).collect();
    let mut c = s.db.get().await.map_err(interno)?;
    let tx = db::begin(&mut c, Ctx::Tecnico(t.id))
        .await
        .map_err(interno)?;
    let n = tx
        .execute(
            "UPDATE agentes SET apelido_cliente = $1 WHERE id = $2",
            &[&apelido, &id],
        )
        .await
        .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    if n == 0 {
        return Err(erro(StatusCode::NOT_FOUND, "agente não encontrado"));
    }
    Ok(Json(json!({"id": id, "apelido_cliente": apelido})))
}

// ---------------- Pareamento ----------------

/// Limite simples de tentativas por IP (5 por minuto).
#[derive(Default)]
pub struct Limite {
    mapa: Mutex<HashMap<String, (Instant, u32)>>,
}

impl Limite {
    pub fn permitir(&self, chave: &str, max: u32, janela: Duration) -> bool {
        let mut m = self.mapa.lock().unwrap();
        let agora = Instant::now();
        m.retain(|_, (t, _)| agora.duration_since(*t) < janela);
        let e = m.entry(chave.to_string()).or_insert((agora, 0));
        e.1 += 1;
        e.1 <= max
    }
}

#[derive(Deserialize)]
pub struct Parear {
    codigo: String,
    dispositivo_id: Uuid,
}

pub async fn parear(
    State(s): State<AppState>,
    t: Tecnico,
    headers: HeaderMap,
    Json(p): Json<Parear>,
) -> Res {
    let ip = crate::ip(&headers);
    if !s
        .limite
        .permitir(&format!("parear:{ip}"), 5, Duration::from_secs(60))
    {
        return Err(erro(
            StatusCode::TOO_MANY_REQUESTS,
            "muitas tentativas; aguarde um minuto",
        ));
    }
    let mut c = s.db.get().await.map_err(interno)?;

    // O dispositivo precisa ser do técnico e estar ativo (contexto do técnico).
    let tx = db::begin(&mut c, Ctx::Tecnico(t.id))
        .await
        .map_err(interno)?;
    let chave: Option<String> = tx
        .query_opt(
            "SELECT chave_publica FROM dispositivos WHERE id = $1 AND revogado_em IS NULL",
            &[&p.dispositivo_id],
        )
        .await
        .map_err(interno)?
        .map(|r| r.get(0));
    tx.commit().await.map_err(interno)?;
    let Some(chave_dispositivo) = chave else {
        return Err(erro(
            StatusCode::BAD_REQUEST,
            "dispositivo não registrado nesta conta",
        ));
    };

    // Código: uso único, prazo curto (contexto do sistema).
    let pedido = Uuid::new_v4();
    let tx = db::begin(&mut c, Ctx::Sistema).await.map_err(interno)?;
    let row = tx
        .query_opt(
            "UPDATE pareamentos p SET tecnico_id = $2, dispositivo_id = $3, pedido_id = $4
             FROM agentes a
             WHERE p.codigo_hash = $1 AND p.agente_id = a.id AND p.usado_em IS NULL
               AND p.expira_em > now() AND p.tecnico_id IS NULL
             RETURNING a.id, a.dono_id, a.chave_publica",
            &[&hash_codigo(&p.codigo), &t.id, &p.dispositivo_id, &pedido],
        )
        .await
        .map_err(interno)?;
    let Some(row) = row else {
        db::auditar(&tx, Some(t.id), "pareamento_invalido", "", &ip)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        return Err(erro(
            StatusCode::BAD_REQUEST,
            "código inválido, já usado ou expirado; gere outro QR no agente",
        ));
    };
    let agente: Uuid = row.get(0);
    let dono: Option<Uuid> = row.get(1);
    let chave_agente: String = row.get(2);
    if dono.is_some() {
        tx.rollback().await.map_err(interno)?;
        return Err(erro(
            StatusCode::CONFLICT,
            "este agente já tem dono; desparear no local antes",
        ));
    }
    db::auditar(
        &tx,
        Some(t.id),
        "pareamento_pedido",
        &agente.to_string(),
        &ip,
    )
    .await
    .map_err(interno)?;
    tx.commit().await.map_err(interno)?;

    let entregue = s.hub.para_agente(
        &agente,
        json!({
            "tipo": "pedido_pareamento",
            "pedido_id": pedido,
            "tecnico_id": t.id,
            "email": t.email,
            "dispositivo": {"id": p.dispositivo_id, "chave_publica": chave_dispositivo},
        }),
    );
    if !entregue {
        return Err(erro(
            StatusCode::SERVICE_UNAVAILABLE,
            "o agente está desconectado",
        ));
    }
    Ok(Json(json!({
        "estado": "aguardando_confirmacao",
        "agente_id": agente,
        "impressao_agente": fingerprint(&chave_agente),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limite_por_janela() {
        let l = Limite::default();
        for _ in 0..5 {
            assert!(l.permitir("a", 5, Duration::from_secs(60)));
        }
        assert!(!l.permitir("a", 5, Duration::from_secs(60)));
        assert!(l.permitir("b", 5, Duration::from_secs(60)));
    }

    #[test]
    fn chave_spki() {
        let mut spki = vec![0x30, 0x59, 0x30, 0x13];
        spki.resize(91, 1);
        assert!(chave_p256_valida(&B64.encode(&spki)));
        assert!(!chave_p256_valida(&B64.encode([1u8; 91])));
        assert!(!chave_p256_valida("xx"));
    }
}
