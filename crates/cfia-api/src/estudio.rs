//! Modo estúdio no site (SDD-07, ADR 0013): o técnico compartilha o que
//! aprendeu publicando a **receita** (JSON validado pelo núcleo) e as métricas
//! do treino. Nunca fotos de clientes, rótulos nem o modelo: a receita não tem
//! campo para isso e o validador recusa campos desconhecidos e arquivos
//! embutidos em texto.
//!
//! - `POST /api/estudio/receitas` (logado): publica ou atualiza (por `id` da receita).
//! - `GET  /api/estudio/receitas?q=` (público): lista as públicas.
//! - `GET  /api/estudio/receitas/minhas` (logado): as do técnico, inclusive privadas.
//! - `GET  /api/estudio/receitas/{id}/receita.json` (público): baixa e conta o download.
//! - `DELETE /api/estudio/receitas/{id}` (dono): apaga.
//! - `POST /api/estudio/notebook`: gera o notebook do Colab a partir de uma receita.

use crate::auth::Tecnico;
use crate::db::{self, Ctx};
use crate::{erro, interno, AppState, Falha};
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use cf_core::receita::{self, Receita, AVISO_ESTUDO};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

type Res = Result<Json<Value>, Falha>;

fn invalida(e: Vec<String>) -> Falha {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({ "erro": e.join("; "), "erros": e })),
    )
}

const COLS: &str =
    "id, slug, nome, descricao, classes, metricas, autor, downloads, receita_sha256, \
     to_char(criado_em, 'YYYY-MM-DD'), to_char(atualizado_em, 'YYYY-MM-DD'), publica";

fn resumo(r: &tokio_postgres::Row) -> Value {
    json!({
        "id": r.get::<_, Uuid>(0),
        "receita_id": r.get::<_, String>(1),
        "nome": r.get::<_, String>(2),
        "descricao": r.get::<_, String>(3),
        "classes": r.get::<_, Vec<String>>(4),
        "metricas": r.get::<_, Option<Value>>(5),
        "autor": r.get::<_, String>(6),
        "downloads": r.get::<_, i32>(7),
        "sha256": r.get::<_, String>(8),
        "criado_em": r.get::<_, String>(9),
        "atualizado_em": r.get::<_, String>(10),
        "publica": r.get::<_, bool>(11),
        "somente_estudo": true,
    })
}

fn limpar_autor(a: Option<String>) -> Result<String, Falha> {
    let a: String = a.unwrap_or_default().trim().chars().take(40).collect();
    if a.contains('@') {
        return Err(erro(
            StatusCode::BAD_REQUEST,
            "autor: use um apelido, não um e-mail",
        ));
    }
    if a.chars().any(char::is_control) {
        return Err(erro(StatusCode::BAD_REQUEST, "autor: caracteres inválidos"));
    }
    Ok(a)
}

#[derive(Deserialize)]
pub struct Publicar {
    receita: Value,
    /// Apelido público (opcional). Vazio = usa o da receita, se houver.
    #[serde(default)]
    autor: Option<String>,
    #[serde(default = "sim")]
    publica: bool,
}

fn sim() -> bool {
    true
}

/// POST /api/estudio/receitas
pub async fn publicar(
    State(s): State<AppState>,
    t: Tecnico,
    headers: HeaderMap,
    Json(p): Json<Publicar>,
) -> Res {
    if !s
        .limite
        .permitir(&format!("receita:{}", t.id), 30, Duration::from_secs(3600))
    {
        return Err(erro(
            StatusCode::TOO_MANY_REQUESTS,
            "muitas publicações seguidas; tente daqui a pouco",
        ));
    }
    let r: Receita = receita::de_valor(p.receita).map_err(invalida)?;
    let autor = match limpar_autor(p.autor)? {
        a if a.is_empty() => r.estudo.autor.clone(),
        a => a,
    };
    let canonico: Value = serde_json::from_str(&r.canonico()).map_err(interno)?;
    let metricas = r
        .estudo
        .metricas
        .map(|m| serde_json::to_value(m).unwrap_or(Value::Null));
    let mut c = s.db.get().await.map_err(interno)?;
    let tx = db::begin(&mut c, Ctx::Tecnico(t.id))
        .await
        .map_err(interno)?;
    let row = tx
        .query_one(
            "INSERT INTO receitas_estudo(id, tecnico_id, slug, nome, descricao, classes, receita,
                 receita_sha256, metricas, autor, publica)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (tecnico_id, slug) DO UPDATE SET nome = excluded.nome,
                 descricao = excluded.descricao, classes = excluded.classes,
                 receita = excluded.receita, receita_sha256 = excluded.receita_sha256,
                 metricas = excluded.metricas, autor = excluded.autor,
                 publica = excluded.publica, atualizado_em = now()
             RETURNING id, (xmax = 0)",
            &[
                &Uuid::new_v4(),
                &t.id,
                &r.id,
                &r.nome,
                &r.descricao,
                &r.nomes_classes(),
                &canonico,
                &r.sha256(),
                &metricas,
                &autor,
                &p.publica,
            ],
        )
        .await
        .map_err(interno)?;
    let id: Uuid = row.get(0);
    let nova: bool = row.get(1);
    db::auditar(
        &tx,
        Some(t.id),
        "receita_estudo",
        &id.to_string(),
        &crate::ip(&headers),
    )
    .await
    .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    Ok(Json(json!({
        "id": id,
        "receita_id": r.id,
        "sha256": r.sha256(),
        "nova": nova,
        "publica": p.publica,
        "aviso": AVISO_ESTUDO,
    })))
}

#[derive(Deserialize)]
pub struct Busca {
    #[serde(default)]
    q: Option<String>,
}

/// GET /api/estudio/receitas — públicas, as mais recentes primeiro.
pub async fn listar(State(s): State<AppState>, Query(b): Query<Busca>) -> Res {
    let q: Option<String> =
        b.q.map(|q| {
            q.chars()
                .filter(|c| !matches!(c, '%' | '_' | '\\'))
                .take(60)
                .collect::<String>()
                .trim()
                .to_string()
        })
        .filter(|q| !q.is_empty());
    let mut c = s.db.get().await.map_err(interno)?;
    // Contexto público: a RLS só deixa ver o que está marcado como público.
    let tx = db::begin(&mut c, Ctx::Publico).await.map_err(interno)?;
    let rows = tx
        .query(
            &format!(
                "SELECT {COLS} FROM receitas_estudo
                 WHERE publica AND ($1::text IS NULL
                     OR nome ILIKE '%' || $1 || '%'
                     OR slug ILIKE '%' || $1 || '%'
                     OR array_to_string(classes, ' ') ILIKE '%' || $1 || '%')
                 ORDER BY atualizado_em DESC LIMIT 100"
            ),
            &[&q],
        )
        .await
        .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    let lista: Vec<Value> = rows.iter().map(resumo).collect();
    Ok(Json(json!({ "receitas": lista, "aviso": AVISO_ESTUDO })))
}

/// GET /api/estudio/receitas/minhas
pub async fn minhas(State(s): State<AppState>, t: Tecnico) -> Res {
    let mut c = s.db.get().await.map_err(interno)?;
    let tx = db::begin(&mut c, Ctx::Tecnico(t.id))
        .await
        .map_err(interno)?;
    let rows = tx
        .query(
            &format!(
                "SELECT {COLS} FROM receitas_estudo WHERE tecnico_id = $1
                 ORDER BY atualizado_em DESC"
            ),
            &[&t.id],
        )
        .await
        .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    let lista: Vec<Value> = rows.iter().map(resumo).collect();
    Ok(Json(json!({ "receitas": lista })))
}

/// GET /api/estudio/receitas/{id}/receita.json — baixa a receita pública.
pub async fn baixar(State(s): State<AppState>, Path(id): Path<Uuid>) -> Result<Response, Falha> {
    let mut c = s.db.get().await.map_err(interno)?;
    let tx = db::begin(&mut c, Ctx::Publico).await.map_err(interno)?;
    let row = tx
        .query_opt(
            "SELECT receita, slug FROM receitas_estudo WHERE id = $1 AND publica",
            &[&id],
        )
        .await
        .map_err(interno)?
        .ok_or_else(|| erro(StatusCode::NOT_FOUND, "receita não encontrada"))?;
    tx.commit().await.map_err(interno)?;
    let receita: Value = row.get(0);
    let slug: String = row.get(1);
    // Contar o download é coisa do servidor (o visitante não altera a linha).
    let tx = db::begin(&mut c, Ctx::Sistema).await.map_err(interno)?;
    tx.execute(
        "UPDATE receitas_estudo SET downloads = downloads + 1 WHERE id = $1",
        &[&id],
    )
    .await
    .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    let corpo = serde_json::to_string_pretty(&receita).map_err(interno)?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "application/json; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{slug}.receita.json\""),
            ),
        ],
        corpo,
    )
        .into_response())
}

/// DELETE /api/estudio/receitas/{id} — só o dono (para os outros, 404).
pub async fn apagar(
    State(s): State<AppState>,
    t: Tecnico,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Res {
    let mut c = s.db.get().await.map_err(interno)?;
    let tx = db::begin(&mut c, Ctx::Tecnico(t.id))
        .await
        .map_err(interno)?;
    let n = tx
        .execute("DELETE FROM receitas_estudo WHERE id = $1", &[&id])
        .await
        .map_err(interno)?;
    if n == 0 {
        return Err(erro(StatusCode::NOT_FOUND, "receita não encontrada"));
    }
    db::auditar(
        &tx,
        Some(t.id),
        "receita_estudo_apagada",
        &id.to_string(),
        &crate::ip(&headers),
    )
    .await
    .map_err(interno)?;
    tx.commit().await.map_err(interno)?;
    Ok(Json(json!({ "apagada": true })))
}

#[derive(Deserialize)]
pub struct PedidoNotebook {
    receita: Value,
}

/// POST /api/estudio/notebook — gera o `.ipynb` (nada é gravado no servidor).
pub async fn notebook(Json(p): Json<PedidoNotebook>) -> Result<Response, Falha> {
    let r = receita::de_valor(p.receita).map_err(invalida)?;
    let nb = cf_core::notebook::gerar(&r).map_err(invalida)?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/x-ipynb+json".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"{}\"",
                    cf_core::notebook::nome_arquivo(&r)
                ),
            ),
        ],
        nb,
    )
        .into_response())
}

/// GET /esquemas/receita-v1.schema.json — esquema público do formato.
pub async fn esquema() -> Response {
    (
        [(header::CONTENT_TYPE, "application/schema+json")],
        receita::ESQUEMA,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autor_sem_email() {
        assert_eq!(limpar_autor(None).unwrap(), "");
        assert_eq!(
            limpar_autor(Some("  Zé da Câmera ".into())).unwrap(),
            "Zé da Câmera"
        );
        assert!(limpar_autor(Some("ze@x.com".into())).is_err());
        assert_eq!(limpar_autor(Some("a".repeat(90))).unwrap().len(), 40);
    }

    #[tokio::test]
    async fn notebook_sem_banco() {
        let v: Value = serde_json::from_str(&receita::exemplo().canonico()).unwrap();
        let res = notebook(Json(PedidoNotebook { receita: v.clone() }))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.headers()[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("camfacil-estudio-contar-caixas-banana.ipynb"));
        let mut ruim = v;
        ruim["nome"] = json!("biometria do caixa");
        let Err((st, Json(e))) = notebook(Json(PedidoNotebook { receita: ruim })).await else {
            panic!("devia recusar");
        };
        assert_eq!(st, StatusCode::BAD_REQUEST);
        assert!(e["erro"].as_str().unwrap().contains("LGPD"));
    }
}
