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

#[tokio::test]
async fn selo_da_escola_exige_login_e_e_publico_por_codigo() {
    let Some(s) = estado().await else { return };
    let (_a, sa) = criar_sessao_teste(&s, &format!("a-{}", Uuid::new_v4()), "a@teste").await;
    let (_b, sb) = criar_sessao_teste(&s, &format!("b-{}", Uuid::new_v4()), "b@teste").await;

    // Sem login não gera.
    let (st, _) = chamar(&s, "POST", "/api/escola/selo", None, Some(json!({}))).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Gera com Instagram limpo (sem @, minúsculo) e autorização.
    let (st, v) = chamar(
        &s,
        "POST",
        "/api/escola/selo",
        Some(&sa),
        Some(json!({"instagram": "@Emerson.Assis", "autoriza_repost": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["selo"]["instagram"], "emerson.assis");
    assert_eq!(v["selo"]["autoriza_repost"], true);
    let codigo = v["selo"]["codigo"].as_str().unwrap().to_string();

    // Instagram inválido cai fora e, sem Instagram, não há o que repostar.
    let (st, v) = chamar(
        &s,
        "POST",
        "/api/escola/selo",
        Some(&sa),
        Some(json!({"instagram": "tem espaço!", "autoriza_repost": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["selo"]["instagram"].is_null());
    assert_eq!(v["selo"]["autoriza_repost"], false);
    // Um por técnico: o código não muda ao regerar.
    assert_eq!(v["selo"]["codigo"], json!(codigo));

    // O outro técnico não vê o selo como seu; a página pública vê pelo código.
    let (_, v) = chamar(&s, "GET", "/api/escola/selo", Some(&sb), None).await;
    assert!(v["selo"].is_null());
    let (st, v) = chamar(&s, "GET", &format!("/api/escola/selo/{codigo}"), None, None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["selo"]["codigo"], json!(codigo));
    let (st, _) = chamar(&s, "GET", "/api/escola/selo/nao-existe", None, None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Modo estúdio: receitas compartilhadas (SDD-07, ADR 0013)

fn receita_teste(slug: &str) -> Value {
    let mut r = cf_core::receita::exemplo();
    r.id = slug.into();
    r.estudo.metricas = Some(cf_core::receita::Metricas {
        map50: Some(0.81),
        ..Default::default()
    });
    serde_json::from_str(&r.canonico()).unwrap()
}

async fn chamar_texto(s: &AppState, rota: &str) -> (StatusCode, String) {
    let req = Request::builder().uri(rota).body(Body::empty()).unwrap();
    let res = router(s.clone()).oneshot(req).await.unwrap();
    let st = res.status();
    let b = res.into_body().collect().await.unwrap().to_bytes();
    (st, String::from_utf8_lossy(&b).into_owned())
}

#[tokio::test]
async fn receitas_publicar_listar_baixar_e_apagar() {
    let s = ou_pula!(estado().await);
    let (_a, sa) = criar_sessao_teste(&s, &format!("a-{}", Uuid::new_v4()), "a@teste").await;
    let (_b, sb) = criar_sessao_teste(&s, &format!("b-{}", Uuid::new_v4()), "b@teste").await;
    let slug = format!("teste-{}", &Uuid::new_v4().simple().to_string()[..12]);

    // Sem login não publica.
    let corpo = json!({"receita": receita_teste(&slug), "autor": "Zé das Câmeras"});
    let (st, _) = chamar(
        &s,
        "POST",
        "/api/estudio/receitas",
        None,
        Some(corpo.clone()),
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // Publica; a resposta traz o hash e o aviso de estudo.
    let (st, v) = chamar(
        &s,
        "POST",
        "/api/estudio/receitas",
        Some(&sa),
        Some(corpo.clone()),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["nova"], true);
    assert_eq!(v["sha256"].as_str().unwrap().len(), 64);
    assert!(v["aviso"].as_str().unwrap().contains("Somente para estudo"));
    let id = v["id"].as_str().unwrap().to_string();

    // Publicar de novo com o mesmo id de receita atualiza (não duplica).
    let (_, v) = chamar(&s, "POST", "/api/estudio/receitas", Some(&sa), Some(corpo)).await;
    assert_eq!(v["nova"], false);
    assert_eq!(v["id"], json!(id));

    // Lista pública (sem login): aparece com métricas e autor, sem e-mail.
    let (st, v) = chamar(
        &s,
        "GET",
        &format!("/api/estudio/receitas?q={slug}"),
        None,
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let item = v["receitas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == json!(id))
        .expect("receita pública listada")
        .clone();
    assert_eq!(item["autor"], "Zé das Câmeras");
    assert_eq!(item["classes"], json!(["caixa de banana"]));
    assert!((item["metricas"]["map50"].as_f64().unwrap() - 0.81).abs() < 1e-6);
    assert!(!v.to_string().contains("a@teste"));

    // Outro técnico baixa (conta download) e recebe a receita válida.
    let (st, txt) = chamar_texto(&s, &format!("/api/estudio/receitas/{id}/receita.json")).await;
    assert_eq!(st, StatusCode::OK);
    let baixada = cf_core::receita::ler(&txt).expect("receita baixada é válida");
    assert_eq!(baixada.id, slug);
    let (_, v) = chamar(
        &s,
        "GET",
        &format!("/api/estudio/receitas?q={slug}"),
        Some(&sb),
        None,
    )
    .await;
    assert_eq!(v["receitas"][0]["downloads"], 1);

    // B não apaga a receita de A (404); A apaga.
    let (st, _) = chamar(
        &s,
        "DELETE",
        &format!("/api/estudio/receitas/{id}"),
        Some(&sb),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    let (_, v) = chamar(&s, "GET", "/api/estudio/receitas/minhas", Some(&sa), None).await;
    assert_eq!(v["receitas"].as_array().unwrap().len(), 1);
    let (_, v) = chamar(&s, "GET", "/api/estudio/receitas/minhas", Some(&sb), None).await;
    assert!(v["receitas"].as_array().unwrap().is_empty());
    let (st, _) = chamar(
        &s,
        "DELETE",
        &format!("/api/estudio/receitas/{id}"),
        Some(&sa),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, _) = chamar_texto(&s, &format!("/api/estudio/receitas/{id}/receita.json")).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn receitas_recusam_fotos_biometria_e_email() {
    let s = ou_pula!(estado().await);
    let (_a, sa) = criar_sessao_teste(&s, &format!("a-{}", Uuid::new_v4()), "a@teste").await;

    // Campo de fotos não existe no formato.
    let mut r = receita_teste("com-fotos");
    r["fotos"] = json!(["/9j/4AAQSkZJRgABAQ..."]);
    let (st, v) = chamar(
        &s,
        "POST",
        "/api/estudio/receitas",
        Some(&sa),
        Some(json!({"receita": r})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(v["erro"]
        .as_str()
        .unwrap()
        .contains("campo desconhecido `fotos`"));

    // Foto escondida num texto.
    let mut r = receita_teste("foto-escondida");
    r["estudo"]["notas"] = json!(format!("data:image/jpeg;base64,{}", "A".repeat(300)));
    let (st, _) = chamar(
        &s,
        "POST",
        "/api/estudio/receitas",
        Some(&sa),
        Some(json!({"receita": r})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // Reconhecimento facial / biometria.
    let mut r = receita_teste("portaria");
    r["nome"] = json!("Reconhecimento facial na portaria");
    let (st, v) = chamar(
        &s,
        "POST",
        "/api/estudio/receitas",
        Some(&sa),
        Some(json!({"receita": r})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    assert!(v["erro"].as_str().unwrap().contains("LGPD art. 11"));

    // Autor não pode ser e-mail.
    let (st, _) = chamar(
        &s,
        "POST",
        "/api/estudio/receitas",
        Some(&sa),
        Some(json!({"receita": receita_teste("autor-email"), "autor": "eu@empresa.com"})),
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    // Nada foi gravado.
    let (_, v) = chamar(&s, "GET", "/api/estudio/receitas/minhas", Some(&sa), None).await;
    assert!(v["receitas"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn receitas_rls_privada_e_contexto_publico() {
    let s = ou_pula!(estado().await);
    let (a, sa) = criar_sessao_teste(&s, &format!("a-{}", Uuid::new_v4()), "a@teste").await;
    let (b, _) = criar_sessao_teste(&s, &format!("b-{}", Uuid::new_v4()), "b@teste").await;
    let (_, v) = chamar(
        &s,
        "POST",
        "/api/estudio/receitas",
        Some(&sa),
        Some(json!({"receita": receita_teste("privada"), "publica": false})),
    )
    .await;
    let privada = Uuid::parse_str(v["id"].as_str().unwrap()).unwrap();
    let (_, v) = chamar(
        &s,
        "POST",
        "/api/estudio/receitas",
        Some(&sa),
        Some(json!({"receita": receita_teste("publica")})),
    )
    .await;
    let publica = Uuid::parse_str(v["id"].as_str().unwrap()).unwrap();

    // Privada não aparece nem baixa para ninguém além do dono.
    let (st, _) = chamar_texto(&s, &format!("/api/estudio/receitas/{privada}/receita.json")).await;
    assert_eq!(st, StatusCode::NOT_FOUND);

    let mut c = s.db.get().await.unwrap();
    // Visitante: sem WHERE, só enxerga público; não escreve.
    let tx = db::begin(&mut c, Ctx::Publico).await.unwrap();
    let n: i64 = tx
        .query_one(
            "SELECT count(*) FROM receitas_estudo WHERE id = ANY($1)",
            &[&vec![privada, publica]],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1, "visitante só vê a pública");
    let n = tx
        .execute(
            "UPDATE receitas_estudo SET nome = 'x' WHERE id = $1",
            &[&publica],
        )
        .await
        .unwrap();
    assert_eq!(n, 0, "visitante não altera");
    let ins = tx
        .execute(
            "INSERT INTO receitas_estudo(id, tecnico_id, slug, nome, classes, receita, receita_sha256)
             VALUES ($1, $2, 'invasao', 'x', '{}', '{}', '')",
            &[&Uuid::new_v4(), &a],
        )
        .await;
    assert!(ins.is_err(), "visitante não insere");
    tx.rollback().await.unwrap();

    // Técnico B: vê a pública de A, mas não altera nem apaga, mesmo sem WHERE de dono.
    let tx = db::begin(&mut c, Ctx::Tecnico(b)).await.unwrap();
    let n: i64 = tx
        .query_one(
            "SELECT count(*) FROM receitas_estudo WHERE id = ANY($1)",
            &[&vec![privada, publica]],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 1);
    let n = tx
        .execute(
            "UPDATE receitas_estudo SET nome = 'x' WHERE tecnico_id = $1",
            &[&a],
        )
        .await
        .unwrap();
    assert_eq!(n, 0);
    let n = tx
        .execute("DELETE FROM receitas_estudo WHERE tecnico_id = $1", &[&a])
        .await
        .unwrap();
    assert_eq!(n, 0);
    // E não publica em nome de A.
    let ins = tx
        .execute(
            "INSERT INTO receitas_estudo(id, tecnico_id, slug, nome, classes, receita, receita_sha256)
             VALUES ($1, $2, 'em-nome-de-a', 'x', '{}', '{}', '')",
            &[&Uuid::new_v4(), &a],
        )
        .await;
    assert!(ins.is_err());
    tx.rollback().await.unwrap();

    // O dono vê as duas.
    let tx = db::begin(&mut c, Ctx::Tecnico(a)).await.unwrap();
    let n: i64 = tx
        .query_one(
            "SELECT count(*) FROM receitas_estudo WHERE id = ANY($1)",
            &[&vec![privada, publica]],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(n, 2);
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn notebook_e_esquema_publicos() {
    let s = ou_pula!(estado().await);
    let (st, v) = chamar(
        &s,
        "POST",
        "/api/estudio/notebook",
        None,
        Some(json!({"receita": receita_teste("nb-teste")})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["nbformat"], 4);
    assert_eq!(v["metadata"]["camfacil"]["somente_estudo"], true);
    let (st, txt) = chamar_texto(&s, "/esquemas/receita-v1.schema.json").await;
    assert_eq!(st, StatusCode::OK);
    assert!(txt.contains("camfacil-receita"));
}
