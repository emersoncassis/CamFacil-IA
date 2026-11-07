//! Testes de integração contra Postgres real (SDD-08, critérios de aceite).
//! Rodam quando `TEST_DATABASE_URL` está definida; senão, são ignorados.

use crate::auth::{criar_sessao_teste, COOKIE};
use crate::db::{self, Ctx};
use crate::util::{sha256_hex, B64};
use crate::{api, auth, hub, router, AppState, Inner};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::mpsc::unbounded_channel;
use tower::ServiceExt;
use uuid::Uuid;

const BASE: &str = "https://teste.local";

async fn estado() -> Option<AppState> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let pool = db::pool(&url).unwrap();
    db::migrate(&pool).await.unwrap();
    Some(AppState(Arc::new(Inner {
        db: pool,
        http: reqwest::Client::new(),
        google: None,
        jwks: auth::Jwks::default(),
        hub: hub::Hub::default(),
        limite: api::Limite::default(),
        base_url: BASE.into(),
    })))
}

async fn chamar(
    s: &AppState,
    metodo: &str,
    rota: &str,
    sessao: Option<&str>,
    corpo: Option<Value>,
) -> (StatusCode, Value) {
    let mut r = Request::builder()
        .method(metodo)
        .uri(rota)
        .header("origin", BASE);
    if let Some(t) = sessao {
        r = r.header("cookie", format!("{COOKIE}={t}"));
    }
    let req = match corpo {
        Some(v) => r
            .header("content-type", "application/json")
            .body(Body::from(v.to_string())),
        None => r.body(Body::empty()),
    }
    .unwrap();
    let res = router(s.clone()).oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn chave_dispositivo_fake() -> String {
    let mut spki = vec![0x30, 0x59, 0x30, 0x13];
    let mut resto = [0u8; 87];
    getrandom::fill(&mut resto).unwrap();
    spki.extend_from_slice(&resto);
    B64.encode(spki)
}

async fn agente_de(s: &AppState, dono: Option<Uuid>) -> Uuid {
    let mut c = s.db.get().await.unwrap();
    let tx = db::begin(&mut c, Ctx::Sistema).await.unwrap();
    let id = Uuid::new_v4();
    tx.execute(
        "INSERT INTO agentes(id, chave_publica, dono_id) VALUES ($1, $2, $3)",
        &[&id, &format!("chave-{id}"), &dono],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    id
}

macro_rules! ou_pula {
    ($e:expr) => {
        match $e {
            Some(s) => s,
            None => {
                eprintln!("TEST_DATABASE_URL ausente: teste de integração ignorado");
                return;
            }
        }
    };
}

#[tokio::test]
async fn tecnico_b_nao_ve_nada_do_tecnico_a() {
    let s = ou_pula!(estado().await);
    let (a, sa) = criar_sessao_teste(&s, &format!("a-{}", Uuid::new_v4()), "a@teste").await;
    let (_b, sb) = criar_sessao_teste(&s, &format!("b-{}", Uuid::new_v4()), "b@teste").await;
    let agente = agente_de(&s, Some(a)).await;

    // A vê o próprio agente.
    let (st, v) = chamar(&s, "GET", "/api/agentes", Some(&sa), None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["agentes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["id"] == agente.to_string()));

    // B não vê, e editar dá 404 (não revela que existe).
    let (_, v) = chamar(&s, "GET", "/api/agentes", Some(&sb), None).await;
    assert!(!v["agentes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["id"] == agente.to_string()));
    let (st, _) = chamar(
        &s,
        "PATCH",
        &format!("/api/agentes/{agente}"),
        Some(&sb),
        Some(json!({"apelido_cliente": "invasor"})),
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    // Dispositivo de A: B não revoga.
    let (st, v) = chamar(
        &s,
        "POST",
        "/api/dispositivos",
        Some(&sa),
        Some(json!({"chave_publica": chave_dispositivo_fake(), "apelido": "notebook"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let disp = v["id"].as_str().unwrap().to_string();
    let (st, _) = chamar(
        &s,
        "DELETE",
        &format!("/api/dispositivos/{disp}"),
        Some(&sb),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (st, _) = chamar(
        &s,
        "DELETE",
        &format!("/api/dispositivos/{disp}"),
        Some(&sa),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn rls_bloqueia_mesmo_sem_where() {
    let s = ou_pula!(estado().await);
    let (a, _) = criar_sessao_teste(&s, &format!("a-{}", Uuid::new_v4()), "a@teste").await;
    let (b, _) = criar_sessao_teste(&s, &format!("b-{}", Uuid::new_v4()), "b@teste").await;
    agente_de(&s, Some(a)).await;
    let mut c = s.db.get().await.unwrap();
    let tx = db::begin(&mut c, Ctx::Tecnico(b)).await.unwrap();
    // Consulta "esquecendo" o filtro: o banco ainda assim só devolve o que é de B.
    let n: i64 = tx
        .query_one("SELECT count(*) FROM agentes WHERE dono_id = $1", &[&a])
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 0);
    let n: i64 = tx
        .query_one("SELECT count(*) FROM tecnicos", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1, "B só enxerga a si mesmo");
    let n: i64 = tx
        .query_one("SELECT count(*) FROM pareamentos", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 0, "pareamentos só no contexto do sistema");
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn sem_login_e_sem_origem() {
    let s = ou_pula!(estado().await);
    let (st, _) = chamar(&s, "GET", "/api/agentes", None, None).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    // POST sem Origin é recusado mesmo com sessão.
    let (_, sa) = criar_sessao_teste(&s, &format!("a-{}", Uuid::new_v4()), "a@teste").await;
    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/sair")
        .header("cookie", format!("{COOKIE}={sa}"))
        .body(Body::empty())
        .unwrap();
    let res = router(s.clone()).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn pareamento_completo_com_confirmacao_local() {
    let s = ou_pula!(estado().await);
    let (a, sa) = criar_sessao_teste(&s, &format!("a-{}", Uuid::new_v4()), "a@teste").await;
    let (_b, sb) = criar_sessao_teste(&s, &format!("b-{}", Uuid::new_v4()), "b@teste").await;
    let agente = agente_de(&s, None).await;

    // Agente "on-line" com um canal falso.
    let (tx_ag, mut rx_ag) = unbounded_channel::<String>();
    s.hub.registrar_agente_teste(agente, tx_ag.clone());

    // O agente publica o hash do código.
    let codigo = format!("COD-{}", Uuid::new_v4());
    hub::tratar_agente(
        &s,
        agente,
        &json!({"tipo": "novo_codigo", "codigo_hash": sha256_hex(codigo.as_bytes())}),
        &tx_ag,
    )
    .await
    .unwrap();
    assert!(rx_ag.recv().await.unwrap().contains("codigo_ok"));

    // Código errado: 400.
    let (_, v) = chamar(
        &s,
        "POST",
        "/api/dispositivos",
        Some(&sa),
        Some(json!({"chave_publica": chave_dispositivo_fake()})),
    )
    .await;
    let disp_a = v["id"].as_str().unwrap().to_string();
    let (st, _) = chamar(
        &s,
        "POST",
        "/api/parear",
        Some(&sa),
        Some(json!({"codigo": "errado", "dispositivo_id": disp_a})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // B não pode usar o dispositivo de A.
    let (st, _) = chamar(
        &s,
        "POST",
        "/api/parear",
        Some(&sb),
        Some(json!({"codigo": codigo, "dispositivo_id": disp_a})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // A pede o pareamento: o agente recebe o pedido com o e-mail de A.
    let (st, v) = chamar(
        &s,
        "POST",
        "/api/parear",
        Some(&sa),
        Some(json!({"codigo": codigo, "dispositivo_id": disp_a})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let pedido: Value = serde_json::from_str(&rx_ag.recv().await.unwrap()).unwrap();
    assert_eq!(pedido["tipo"], "pedido_pareamento");
    assert_eq!(pedido["email"], "a@teste");

    // Código já consumido: segunda tentativa falha.
    let (st, _) = chamar(
        &s,
        "POST",
        "/api/parear",
        Some(&sa),
        Some(json!({"codigo": codigo, "dispositivo_id": disp_a})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // Confirmação na tela local.
    hub::tratar_agente(
        &s,
        agente,
        &json!({"tipo": "confirmar_pareamento", "pedido_id": pedido["pedido_id"], "aceito": true}),
        &tx_ag,
    )
    .await
    .unwrap();
    let mut c = s.db.get().await.unwrap();
    let tx = db::begin(&mut c, Ctx::Sistema).await.unwrap();
    let dono: Option<Uuid> = tx
        .query_one("SELECT dono_id FROM agentes WHERE id = $1", &[&agente])
        .await
        .unwrap()
        .get(0);
    tx.commit().await.unwrap();
    assert_eq!(dono, Some(a));

    // Já pareado: novo código é recusado pelo próprio servidor.
    hub::tratar_agente(
        &s,
        agente,
        &json!({"tipo": "novo_codigo", "codigo_hash": sha256_hex(b"outro")}),
        &tx_ag,
    )
    .await
    .unwrap();
    let _confirmacao = rx_ag.recv().await.unwrap();
    assert!(rx_ag.recv().await.unwrap().contains("já pareado"));

    // O agente manda o resumo: só o dono enxerga, já filtrado.
    hub::tratar_agente(
        &s,
        agente,
        &json!({"tipo": "estado", "estado": {"cameras": [{"nome": "Portão", "online": true, "fps": 5.0, "url": "rtsp://x"}], "cpu": 20.0}}),
        &tx_ag,
    )
    .await
    .unwrap();
    let (_, v) = chamar(&s, "GET", "/api/agentes", Some(&sa), None).await;
    let meu = v["agentes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == json!(agente))
        .unwrap();
    assert_eq!(meu["estado"]["cameras"][0]["nome"], "Portão");
    assert!(!meu["estado"].to_string().contains("rtsp://"));
    let (_, v) = chamar(&s, "GET", "/api/agentes", Some(&sb), None).await;
    assert!(!v.to_string().contains("Portão"));
}
