//! Sinalização WebRTC e mensagens de pareamento (SDD-08).
//!
//! O servidor só repassa mensagens entre um técnico e os agentes **dele**.
//! Vídeo e comandos nunca passam por aqui.

use crate::db::{self, Ctx};
use crate::util::{sha256_hex, token, B64};
use crate::{auth, AppState};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use uuid::Uuid;

/// Prefixo assinado pelo agente, para a assinatura não servir em outro contexto.
pub const PREFIXO_DESAFIO: &str = "cfia-agente-v1:";
/// Tamanho máximo de uma mensagem de sinalização.
const MAX_MSG: usize = 64 * 1024;

type Tx = UnboundedSender<String>;

#[derive(Default)]
pub struct Hub {
    agentes: Mutex<HashMap<Uuid, Tx>>,
    paineis: Mutex<HashMap<Uuid, Vec<(u64, Tx)>>>,
    seq: AtomicU64,
}

impl Hub {
    pub fn online(&self, agente: &Uuid) -> bool {
        self.agentes.lock().unwrap().contains_key(agente)
    }

    pub fn para_agente(&self, agente: &Uuid, msg: Value) -> bool {
        match self.agentes.lock().unwrap().get(agente) {
            Some(tx) => tx.send(msg.to_string()).is_ok(),
            None => false,
        }
    }

    pub fn para_tecnico(&self, tecnico: &Uuid, msg: Value) {
        let texto = msg.to_string();
        if let Some(v) = self.paineis.lock().unwrap().get(tecnico) {
            for (_, tx) in v {
                let _ = tx.send(texto.clone());
            }
        }
    }

    #[cfg(test)]
    pub fn registrar_agente_teste(&self, agente: Uuid, tx: Tx) {
        self.agentes.lock().unwrap().insert(agente, tx);
    }

    fn entrar_painel(&self, tecnico: Uuid, tx: Tx) -> u64 {
        let id = self.seq.fetch_add(1, Ordering::Relaxed);
        self.paineis
            .lock()
            .unwrap()
            .entry(tecnico)
            .or_default()
            .push((id, tx));
        id
    }

    fn sair_painel(&self, tecnico: &Uuid, id: u64) {
        let mut p = self.paineis.lock().unwrap();
        if let Some(v) = p.get_mut(tecnico) {
            v.retain(|(i, _)| *i != id);
            if v.is_empty() {
                p.remove(tecnico);
            }
        }
    }
}

/// Verifica a assinatura Ed25519 do agente sobre o desafio.
pub fn verificar_agente(chave_b64: &str, assinatura_b64: &str, nonce: &str) -> bool {
    let (Ok(chave), Ok(sig)) = (B64.decode(chave_b64), B64.decode(assinatura_b64)) else {
        return false;
    };
    let (Ok(chave), Ok(sig)): (Result<[u8; 32], _>, Result<[u8; 64], _>) =
        (chave.try_into(), sig.try_into())
    else {
        return false;
    };
    let Ok(vk) = ed25519_dalek::VerifyingKey::from_bytes(&chave) else {
        return false;
    };
    let sig = ed25519_dalek::Signature::from_bytes(&sig);
    vk.verify_strict(format!("{PREFIXO_DESAFIO}{nonce}").as_bytes(), &sig)
        .is_ok()
}

// ---------------------------------------------------------------------------
// Agente
// ---------------------------------------------------------------------------

/// WS /ws/agente
pub async fn ws_agente(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(MAX_MSG)
        .on_upgrade(move |socket| async move {
            if let Err(e) = sessao_agente(state, socket).await {
                eprintln!("agente desconectado: {e}");
            }
        })
}

async fn sessao_agente(state: AppState, socket: WebSocket) -> db::Result<()> {
    let (mut sink, mut stream) = socket.split();
    let nonce = token(32);
    sink.send(Message::Text(
        json!({"tipo": "desafio", "nonce": nonce})
            .to_string()
            .into(),
    ))
    .await?;

    // Primeira mensagem: identificação assinada, em até 10 s.
    let ola = tokio::time::timeout(Duration::from_secs(10), stream.next()).await;
    let Ok(Some(Ok(Message::Text(t)))) = ola else {
        return Ok(());
    };
    let v: Value = serde_json::from_str(&t)?;
    let chave = v["chave_publica"].as_str().unwrap_or("");
    let assinatura = v["assinatura"].as_str().unwrap_or("");
    let versao: String = v["versao"]
        .as_str()
        .unwrap_or("")
        .chars()
        .take(32)
        .collect();
    if v["tipo"] != "ola" || !verificar_agente(chave, assinatura, &nonce) {
        let _ = sink
            .send(Message::Text(
                json!({"tipo": "erro", "mensagem": "assinatura inválida"})
                    .to_string()
                    .into(),
            ))
            .await;
        return Ok(());
    }

    let mut c = state.db.get().await?;
    let tx = db::begin(&mut c, Ctx::Sistema).await?;
    let row = tx
        .query_one(
            "INSERT INTO agentes(id, chave_publica, versao, ultimo_contato) VALUES ($1, $2, $3, now())
             ON CONFLICT (chave_publica) DO UPDATE SET versao = excluded.versao, ultimo_contato = now()
             RETURNING id, dono_id",
            &[&Uuid::new_v4(), &chave, &versao],
        )
        .await?;
    let agente: Uuid = row.get(0);
    let dono: Option<Uuid> = row.get(1);
    let email: Option<String> = match dono {
        Some(d) => tx
            .query_opt("SELECT email FROM tecnicos WHERE id = $1", &[&d])
            .await?
            .map(|r| r.get(0)),
        None => None,
    };
    tx.commit().await?;

    let (out_tx, mut out_rx) = unbounded_channel::<String>();
    if let Some(antigo) = state
        .hub
        .agentes
        .lock()
        .unwrap()
        .insert(agente, out_tx.clone())
    {
        let _ = antigo.send(json!({"tipo": "substituido"}).to_string());
    }
    let _ = out_tx.send(
        json!({"tipo": "bem_vindo", "agente_id": agente, "pareado": dono.is_some(), "dono_email": email})
            .to_string(),
    );
    if let Some(d) = dono {
        state
            .hub
            .para_tecnico(&d, json!({"tipo": "agente_online", "agente_id": agente}));
    }

    let escritor = tokio::spawn(async move {
        while let Some(m) = out_rx.recv().await {
            if sink.send(Message::Text(m.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = stream.next().await {
        let Message::Text(t) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&t) else {
            continue;
        };
        if let Err(e) = tratar_agente(&state, agente, &v, &out_tx).await {
            eprintln!("erro do agente {agente}: {e}");
        }
    }

    // Saída: só remove se ainda for esta conexão.
    {
        let mut a = state.hub.agentes.lock().unwrap();
        if a.get(&agente).is_some_and(|t| t.same_channel(&out_tx)) {
            a.remove(&agente);
        }
    }
    escritor.abort();
    if let Some(d) = dono_atual(&state, agente).await? {
        state
            .hub
            .para_tecnico(&d, json!({"tipo": "agente_offline", "agente_id": agente}));
    }
    Ok(())
}

async fn dono_atual(state: &AppState, agente: Uuid) -> db::Result<Option<Uuid>> {
    let mut c = state.db.get().await?;
    let tx = db::begin(&mut c, Ctx::Sistema).await?;
    let r = tx
        .query_opt("SELECT dono_id FROM agentes WHERE id = $1", &[&agente])
        .await?
        .and_then(|r| r.get::<_, Option<Uuid>>(0));
    tx.commit().await?;
    Ok(r)
}

pub(crate) async fn tratar_agente(
    state: &AppState,
    agente: Uuid,
    v: &Value,
    out: &Tx,
) -> db::Result<()> {
    let responder = |m: Value| {
        let _ = out.send(m.to_string());
    };
    match v["tipo"].as_str().unwrap_or("") {
        "ping" => responder(json!({"tipo": "pong"})),

        // Resumo periódico do que o agente está vendo. Guarda o último e
        // repassa ao técnico dono, se ele estiver com o painel aberto.
        "estado" => {
            let estado = limpar_estado(&v["estado"]);
            let mut c = state.db.get().await?;
            let tx = db::begin(&mut c, Ctx::Sistema).await?;
            let dono: Option<Uuid> = tx
                .query_opt(
                    "UPDATE agentes SET estado = $2, estado_em = now(), ultimo_contato = now()
                     WHERE id = $1 RETURNING dono_id",
                    &[&agente, &estado],
                )
                .await?
                .and_then(|r| r.get(0));
            tx.commit().await?;
            if let Some(d) = dono {
                state.hub.para_tecnico(
                    &d,
                    json!({"tipo": "estado", "agente_id": agente, "estado": estado}),
                );
            }
        }

        // O agente gerou um código e mostra o QR; o servidor guarda só o hash.
        "novo_codigo" => {
            let hash = v["codigo_hash"].as_str().unwrap_or("");
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                responder(json!({"tipo": "erro", "mensagem": "código inválido"}));
                return Ok(());
            }
            let mut c = state.db.get().await?;
            let tx = db::begin(&mut c, Ctx::Sistema).await?;
            let tem_dono: bool = tx
                .query_one(
                    "SELECT dono_id IS NOT NULL FROM agentes WHERE id = $1",
                    &[&agente],
                )
                .await?
                .get(0);
            if tem_dono {
                tx.commit().await?;
                responder(
                    json!({"tipo": "erro", "mensagem": "agente já pareado; desparear no local antes"}),
                );
                return Ok(());
            }
            tx.execute("DELETE FROM pareamentos WHERE agente_id = $1", &[&agente])
                .await?;
            tx.execute(
                "INSERT INTO pareamentos(codigo_hash, agente_id, expira_em) VALUES ($1, $2, now() + interval '10 minutes')",
                &[&hash.to_lowercase(), &agente],
            )
            .await?;
            tx.commit().await?;
            responder(json!({"tipo": "codigo_ok", "expira_em_s": 600}));
        }

        // Confirmação feita na tela local do agente.
        "confirmar_pareamento" => {
            let Some(pedido) = v["pedido_id"].as_str().and_then(|s| s.parse::<Uuid>().ok()) else {
                return Ok(());
            };
            let aceito = v["aceito"].as_bool().unwrap_or(false);
            let mut c = state.db.get().await?;
            let tx = db::begin(&mut c, Ctx::Sistema).await?;
            let row = tx
                .query_opt(
                    "UPDATE pareamentos SET usado_em = now()
                     WHERE pedido_id = $1 AND agente_id = $2 AND usado_em IS NULL AND expira_em > now()
                     RETURNING tecnico_id",
                    &[&pedido, &agente],
                )
                .await?;
            let Some(row) = row else {
                tx.commit().await?;
                responder(json!({"tipo": "erro", "mensagem": "pedido expirado"}));
                return Ok(());
            };
            let tecnico: Uuid = row.get(0);
            if aceito {
                tx.execute(
                    "UPDATE agentes SET dono_id = $1, pareado_em = now() WHERE id = $2 AND dono_id IS NULL",
                    &[&tecnico, &agente],
                )
                .await?;
                db::auditar(&tx, Some(tecnico), "pareamento", &agente.to_string(), "").await?;
            } else {
                db::auditar(
                    &tx,
                    Some(tecnico),
                    "pareamento_recusado",
                    &agente.to_string(),
                    "",
                )
                .await?;
            }
            tx.commit().await?;
            responder(json!({"tipo": "pareamento_concluido", "aceito": aceito}));
            state.hub.para_tecnico(
                &tecnico,
                json!({"tipo": if aceito { "pareado" } else { "pareamento_recusado" }, "agente_id": agente}),
            );
        }

        // Desparear só pode partir do próprio agente (presença no local).
        "desparear" => {
            let mut c = state.db.get().await?;
            let tx = db::begin(&mut c, Ctx::Sistema).await?;
            let antigo: Option<Uuid> = tx
                .query_opt(
                    "UPDATE agentes a SET dono_id = NULL, pareado_em = NULL
                     FROM (SELECT dono_id FROM agentes WHERE id = $1) old
                     WHERE a.id = $1 RETURNING old.dono_id",
                    &[&agente],
                )
                .await?
                .and_then(|r| r.get(0));
            if let Some(d) = antigo {
                db::auditar(&tx, Some(d), "despareado", &agente.to_string(), "").await?;
            }
            tx.commit().await?;
            responder(json!({"tipo": "despareado"}));
            if let Some(d) = antigo {
                state
                    .hub
                    .para_tecnico(&d, json!({"tipo": "despareado", "agente_id": agente}));
            }
        }

        // Sinalização WebRTC do agente para o técnico dono.
        "sinal" => {
            let Some(para) = v["para_tecnico"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
            else {
                return Ok(());
            };
            if dono_atual(state, agente).await? == Some(para) {
                state.hub.para_tecnico(
                    &para,
                    json!({"tipo": "sinal", "agente_id": agente, "sessao": v["sessao"], "dados": v["dados"]}),
                );
            }
        }
        _ => {}
    }
    Ok(())
}

/// Aceita só os campos conhecidos, com tamanho limitado: o agente não
/// consegue encher o banco nem mandar o que não deve (imagem, senha, URL).
pub(crate) fn limpar_estado(v: &Value) -> Value {
    let txt = |x: &Value, n: usize| -> Value {
        x.as_str()
            .map(|s| Value::String(s.chars().take(n).collect()))
            .unwrap_or(Value::Null)
    };
    let num = |x: &Value| {
        x.as_f64()
            .filter(|f| f.is_finite())
            .map(|f| json!((f * 10.0).round() / 10.0))
            .unwrap_or(Value::Null)
    };
    let cameras: Vec<Value> = v["cameras"]
        .as_array()
        .map(|a| {
            a.iter()
                .take(64)
                .map(|c| {
                    json!({
                        "nome": txt(&c["nome"], 60),
                        "online": c["online"].as_bool().unwrap_or(false),
                        "fps": num(&c["fps"]),
                        "erro": txt(&c["erro"], 120),
                        "saude": txt(&c["saude"], 30),
                        "regras": c["regras"].as_u64().unwrap_or(0).min(100),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let alertas: Vec<Value> = v["alertas"]
        .as_array()
        .map(|a| {
            a.iter()
                .take(20)
                .map(|e| {
                    json!({
                        "camera": txt(&e["camera"], 60),
                        "regra": txt(&e["regra"], 60),
                        "tipo": txt(&e["tipo"], 30),
                        "quando": e["quando"].as_i64(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    json!({
        "cameras": cameras,
        "alertas": alertas,
        "cpu": num(&v["cpu"]),
        "ram_usada_mb": v["ram_usada_mb"].as_u64(),
        "ram_total_mb": v["ram_total_mb"].as_u64(),
        "proprio_mb": v["proprio_mb"].as_u64(),
        "sistema": txt(&v["sistema"], 80),
        "maquina": txt(&v["maquina"], 80),
        "versao": txt(&v["versao"], 20),
        "ligado_s": v["ligado_s"].as_u64(),
        "modulos": v["modulos"].as_array().map(|a| a.iter().take(10).map(|m| txt(m, 20)).collect::<Vec<_>>()),
    })
}

// ---------------------------------------------------------------------------
// Painel (navegador do técnico)
// ---------------------------------------------------------------------------

/// WS /ws/painel
pub async fn ws_painel(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    // Proteção contra uso do WebSocket por outro site (CSWSH).
    if !crate::origem_ok(&state, &headers) {
        return (StatusCode::FORBIDDEN, "origem não permitida").into_response();
    }
    let tecnico = match auth::sessao(&state, &headers).await {
        Ok(t) => t,
        Err(e) => return e.into_response(),
    };
    ws.max_message_size(MAX_MSG)
        .on_upgrade(move |socket| async move {
            if let Err(e) = sessao_painel(state, tecnico.id, socket).await {
                eprintln!("painel desconectado: {e}");
            }
        })
}

async fn sessao_painel(state: AppState, tecnico: Uuid, socket: WebSocket) -> db::Result<()> {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = unbounded_channel::<String>();
    let id = state.hub.entrar_painel(tecnico, out_tx.clone());
    let escritor = tokio::spawn(async move {
        while let Some(m) = out_rx.recv().await {
            if sink.send(Message::Text(m.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(msg)) = stream.next().await {
        let Message::Text(t) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&t) else {
            continue;
        };
        if v["tipo"] != "sinal" {
            continue;
        }
        let Some(agente) = v["agente_id"].as_str().and_then(|s| s.parse::<Uuid>().ok()) else {
            continue;
        };
        // Com a RLS no contexto do técnico, agente de outro técnico simplesmente não existe.
        let mut c = state.db.get().await?;
        let tx = db::begin(&mut c, Ctx::Tecnico(tecnico)).await?;
        let dono = tx
            .query_opt("SELECT 1 FROM agentes WHERE id = $1", &[&agente])
            .await?
            .is_some();
        tx.commit().await?;
        let entregue = dono
            && state.hub.para_agente(
                &agente,
                json!({"tipo": "sinal", "de_tecnico": tecnico, "sessao": v["sessao"], "dados": v["dados"]}),
            );
        if !entregue {
            let _ = out_tx.send(
                json!({"tipo": "erro", "agente_id": agente, "mensagem": "agente indisponível"})
                    .to_string(),
            );
        }
    }

    state.hub.sair_painel(&tecnico, id);
    escritor.abort();
    Ok(())
}

/// Hash do código de pareamento, igual ao que o agente envia.
pub fn hash_codigo(codigo: &str) -> String {
    sha256_hex(codigo.trim().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn assinatura_do_agente() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let pk = B64.encode(sk.verifying_key().to_bytes());
        let sig = B64.encode(
            sk.sign(format!("{PREFIXO_DESAFIO}abc").as_bytes())
                .to_bytes(),
        );
        assert!(verificar_agente(&pk, &sig, "abc"));
        assert!(!verificar_agente(&pk, &sig, "outro"));
        assert!(!verificar_agente("lixo", &sig, "abc"));
    }
}

#[cfg(test)]
mod testes_estado {
    use super::limpar_estado;
    use serde_json::json;

    #[test]
    fn so_passa_o_que_e_conhecido_e_corta_tamanho() {
        let e = limpar_estado(&json!({
            "cameras": [{"nome": "x".repeat(500), "online": true, "fps": 5.04, "url": "rtsp://admin:senha@1.2.3.4/", "senha": "123"}],
            "imagem": "base64...",
            "cpu": 12.345,
            "alertas": (0..50).map(|i| json!({"camera": "Portão", "tipo": "linha_cruzada", "quando": i})).collect::<Vec<_>>(),
        }));
        let s = e.to_string();
        assert!(!s.contains("rtsp://") && !s.contains("senha") && !s.contains("imagem"));
        assert_eq!(
            e["cameras"][0]["nome"].as_str().unwrap().chars().count(),
            60
        );
        assert_eq!(e["alertas"].as_array().unwrap().len(), 20);
        assert_eq!(e["cpu"], json!(12.3));
    }
}
