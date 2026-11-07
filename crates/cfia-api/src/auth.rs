//! Login só com Google (OpenID Connect, code + PKCE) e sessões por cookie.

use crate::db::{self, Ctx};
use crate::util::{sha256_b64url, sha256_hex, token};
use crate::{erro, AppState, Falha};
use axum::{
    extract::{FromRequestParts, Query, State},
    http::{header, request::Parts, HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use jsonwebtoken::{jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;

pub const COOKIE: &str = "__Host-cfia_sessao";
const SESSAO_DIAS: i64 = 30;
const GOOGLE_AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_JWKS: &str = "https://www.googleapis.com/oauth2/v3/certs";
const GOOGLE_ISS: [&str; 2] = ["https://accounts.google.com", "accounts.google.com"];

#[derive(Clone)]
pub struct Google {
    pub client_id: String,
    pub client_secret: String,
}

/// Cache das chaves públicas do Google (renovado a cada hora ou ao ver `kid` novo).
#[derive(Default)]
pub struct Jwks {
    inner: RwLock<Option<(JwkSet, Instant)>>,
}

impl Jwks {
    async fn get(&self, http: &reqwest::Client, kid: &str) -> Result<DecodingKey, Falha> {
        {
            let g = self.inner.read().await;
            if let Some((set, at)) = g.as_ref() {
                if at.elapsed() < Duration::from_secs(3600) {
                    if let Some(k) = set.find(kid) {
                        return DecodingKey::from_jwk(k).map_err(|_| {
                            erro(StatusCode::BAD_GATEWAY, "chave do Google inválida")
                        });
                    }
                }
            }
        }
        let set: JwkSet = http
            .get(GOOGLE_JWKS)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|_| {
                erro(
                    StatusCode::BAD_GATEWAY,
                    "não foi possível falar com o Google",
                )
            })?
            .json()
            .await
            .map_err(|_| erro(StatusCode::BAD_GATEWAY, "resposta do Google inválida"))?;
        let key = set
            .find(kid)
            .ok_or_else(|| erro(StatusCode::UNAUTHORIZED, "chave de assinatura desconhecida"))
            .and_then(|k| {
                DecodingKey::from_jwk(k)
                    .map_err(|_| erro(StatusCode::BAD_GATEWAY, "chave do Google inválida"))
            })?;
        *self.inner.write().await = Some((set, Instant::now()));
        Ok(key)
    }
}

/// Técnico autenticado, extraído do cookie de sessão.
#[derive(Debug, Clone)]
pub struct Tecnico {
    pub id: Uuid,
    pub email: String,
    pub nome: String,
}

pub fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|kv| {
            let (k, v) = kv.trim().split_once('=')?;
            (k == COOKIE && !v.is_empty()).then(|| v.to_string())
        })
}

pub async fn sessao(state: &AppState, headers: &HeaderMap) -> Result<Tecnico, Falha> {
    let token =
        cookie_token(headers).ok_or_else(|| erro(StatusCode::UNAUTHORIZED, "faça login"))?;
    let hash = sha256_hex(token.as_bytes());
    let mut c = state.db.get().await.map_err(crate::interno)?;
    let tx = db::begin(&mut c, Ctx::Sistema)
        .await
        .map_err(crate::interno)?;
    let row = tx
        .query_opt(
            "SELECT t.id, t.email, t.nome FROM sessoes s JOIN tecnicos t ON t.id = s.tecnico_id
             WHERE s.token_hash = $1 AND s.expira_em > now()",
            &[&hash],
        )
        .await
        .map_err(crate::interno)?;
    tx.commit().await.map_err(crate::interno)?;
    let row =
        row.ok_or_else(|| erro(StatusCode::UNAUTHORIZED, "sessão expirada, entre de novo"))?;
    Ok(Tecnico {
        id: row.get(0),
        email: row.get(1),
        nome: row.get(2),
    })
}

impl FromRequestParts<AppState> for Tecnico {
    type Rejection = Falha;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        sessao(state, &parts.headers).await
    }
}

fn google(state: &AppState) -> Result<&Google, Falha> {
    state.google.as_ref().ok_or_else(|| {
        erro(
            StatusCode::SERVICE_UNAVAILABLE,
            "login com Google ainda não configurado no servidor",
        )
    })
}

fn redirect_uri(state: &AppState) -> String {
    format!("{}/api/auth/google/callback", state.base_url)
}

/// GET /api/auth/google — inicia o login.
pub async fn iniciar(State(state): State<AppState>) -> Result<Response, Falha> {
    let g = google(&state)?;
    let estado = token(24);
    let nonce = token(24);
    let verificador = token(48);
    let mut c = state.db.get().await.map_err(crate::interno)?;
    let tx = db::begin(&mut c, Ctx::Sistema)
        .await
        .map_err(crate::interno)?;
    tx.execute("DELETE FROM oauth_estados WHERE expira_em < now()", &[])
        .await
        .map_err(crate::interno)?;
    tx.execute(
        "INSERT INTO oauth_estados(estado, nonce, verificador, expira_em) VALUES ($1, $2, $3, now() + interval '10 minutes')",
        &[&estado, &nonce, &verificador],
    )
    .await
    .map_err(crate::interno)?;
    tx.commit().await.map_err(crate::interno)?;

    let url = reqwest::Url::parse_with_params(
        GOOGLE_AUTH,
        &[
            ("client_id", g.client_id.as_str()),
            ("redirect_uri", &redirect_uri(&state)),
            ("response_type", "code"),
            ("scope", "openid email profile"),
            ("state", &estado),
            ("nonce", &nonce),
            ("code_challenge", &sha256_b64url(verificador.as_bytes())),
            ("code_challenge_method", "S256"),
            ("prompt", "select_account"),
        ],
    )
    .map_err(crate::interno)?;
    Ok(Redirect::to(url.as_str()).into_response())
}

#[derive(Deserialize)]
pub struct Retorno {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct TokenResp {
    id_token: String,
}

#[derive(Debug, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: bool,
    pub name: Option<String>,
    pub nonce: Option<String>,
}

/// Regras extras além da assinatura, `iss`, `aud` e `exp`.
pub fn validar_claims(c: &Claims, nonce_esperado: &str) -> Result<String, &'static str> {
    if c.nonce.as_deref() != Some(nonce_esperado) {
        return Err("nonce inválido");
    }
    if c.sub.is_empty() {
        return Err("identidade ausente");
    }
    match (&c.email, c.email_verified) {
        (Some(e), true) if !e.is_empty() => Ok(e.to_lowercase()),
        _ => Err("e-mail do Google não verificado"),
    }
}

/// GET /api/auth/google/callback — retorno do Google, cria a sessão.
pub async fn retorno(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Retorno>,
) -> Result<Response, Falha> {
    let g = google(&state)?.clone();
    if q.error.is_some() {
        return Ok(Redirect::to("/?erro=login_cancelado").into_response());
    }
    let (code, estado) = match (q.code, q.state) {
        (Some(c), Some(s)) => (c, s),
        _ => {
            return Err(erro(
                StatusCode::BAD_REQUEST,
                "retorno do Google incompleto",
            ))
        }
    };

    // Consome o estado (uso único).
    let mut c = state.db.get().await.map_err(crate::interno)?;
    let tx = db::begin(&mut c, Ctx::Sistema)
        .await
        .map_err(crate::interno)?;
    let row = tx
        .query_opt(
            "DELETE FROM oauth_estados WHERE estado = $1 AND expira_em > now() RETURNING nonce, verificador",
            &[&estado],
        )
        .await
        .map_err(crate::interno)?;
    tx.commit().await.map_err(crate::interno)?;
    let row = row.ok_or_else(|| erro(StatusCode::BAD_REQUEST, "login expirado, tente de novo"))?;
    let (nonce, verificador): (String, String) = (row.get(0), row.get(1));

    let tok: TokenResp = state
        .http
        .post(GOOGLE_TOKEN)
        .form(&[
            ("code", code.as_str()),
            ("client_id", &g.client_id),
            ("client_secret", &g.client_secret),
            ("redirect_uri", &redirect_uri(&state)),
            ("grant_type", "authorization_code"),
            ("code_verifier", &verificador),
        ])
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|_| erro(StatusCode::BAD_GATEWAY, "o Google recusou o login"))?
        .json()
        .await
        .map_err(|_| erro(StatusCode::BAD_GATEWAY, "resposta do Google inválida"))?;

    let header = jsonwebtoken::decode_header(&tok.id_token)
        .map_err(|_| erro(StatusCode::UNAUTHORIZED, "token inválido"))?;
    let kid = header
        .kid
        .ok_or_else(|| erro(StatusCode::UNAUTHORIZED, "token sem chave"))?;
    let key = state.jwks.get(&state.http, &kid).await?;
    let mut v = Validation::new(Algorithm::RS256);
    v.set_audience(&[&g.client_id]);
    v.set_issuer(&GOOGLE_ISS);
    let claims = jsonwebtoken::decode::<Claims>(&tok.id_token, &key, &v)
        .map_err(|_| erro(StatusCode::UNAUTHORIZED, "token do Google inválido"))?
        .claims;
    let email = validar_claims(&claims, &nonce).map_err(|m| erro(StatusCode::UNAUTHORIZED, m))?;
    let nome = claims.name.unwrap_or_default();

    let ip = crate::ip(&headers);
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .chars()
        .take(200)
        .collect::<String>();
    let sessao = token(32);
    let tx = db::begin(&mut c, Ctx::Sistema)
        .await
        .map_err(crate::interno)?;
    let id: Uuid = tx
        .query_one(
            "INSERT INTO tecnicos(id, google_sub, email, nome) VALUES ($1, $2, $3, $4)
             ON CONFLICT (google_sub) DO UPDATE SET email = excluded.email, nome = excluded.nome
             RETURNING id",
            &[&Uuid::new_v4(), &claims.sub, &email, &nome],
        )
        .await
        .map_err(crate::interno)?
        .get(0);
    tx.execute(
        "INSERT INTO sessoes(token_hash, tecnico_id, expira_em, ip, agente_usuario)
         VALUES ($1, $2, now() + make_interval(days => $3), $4, $5)",
        &[
            &sha256_hex(sessao.as_bytes()),
            &id,
            &(SESSAO_DIAS as i32),
            &ip,
            &ua,
        ],
    )
    .await
    .map_err(crate::interno)?;
    db::auditar(&tx, Some(id), "login", &email, &ip)
        .await
        .map_err(crate::interno)?;
    tx.commit().await.map_err(crate::interno)?;

    Ok((
        [(
            header::SET_COOKIE,
            cookie_sessao(&sessao, SESSAO_DIAS * 86_400),
        )],
        Redirect::to("/"),
    )
        .into_response())
}

pub fn cookie_sessao(valor: &str, max_age: i64) -> String {
    format!("{COOKIE}={valor}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age={max_age}")
}

/// POST /api/auth/sair
pub async fn sair(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, Falha> {
    if let Some(t) = cookie_token(&headers) {
        let mut c = state.db.get().await.map_err(crate::interno)?;
        let tx = db::begin(&mut c, Ctx::Sistema)
            .await
            .map_err(crate::interno)?;
        tx.execute(
            "DELETE FROM sessoes WHERE token_hash = $1",
            &[&sha256_hex(t.as_bytes())],
        )
        .await
        .map_err(crate::interno)?;
        tx.commit().await.map_err(crate::interno)?;
    }
    Ok((
        [(header::SET_COOKIE, cookie_sessao("", 0))],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

/// Cria uma sessão diretamente (usado pelos testes de integração).
#[cfg(test)]
pub async fn criar_sessao_teste(state: &AppState, sub: &str, email: &str) -> (Uuid, String) {
    let mut c = state.db.get().await.unwrap();
    let tx = db::begin(&mut c, Ctx::Sistema).await.unwrap();
    let id: Uuid = tx
        .query_one(
            "INSERT INTO tecnicos(id, google_sub, email) VALUES ($1, $2, $3)
             ON CONFLICT (google_sub) DO UPDATE SET email = excluded.email RETURNING id",
            &[&Uuid::new_v4(), &sub, &email],
        )
        .await
        .unwrap()
        .get(0);
    let t = token(32);
    tx.execute(
        "INSERT INTO sessoes(token_hash, tecnico_id, expira_em) VALUES ($1, $2, now() + interval '1 day')",
        &[&sha256_hex(t.as_bytes()), &id],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    (id, t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(nonce: &str, email: Option<&str>, verified: bool) -> Claims {
        Claims {
            sub: "123".into(),
            email: email.map(Into::into),
            email_verified: verified,
            name: None,
            nonce: Some(nonce.into()),
        }
    }

    #[test]
    fn claims_validas() {
        assert_eq!(
            validar_claims(&c("n", Some("A@B.com"), true), "n"),
            Ok("a@b.com".into())
        );
    }

    #[test]
    fn claims_invalidas() {
        assert!(validar_claims(&c("x", Some("a@b.com"), true), "n").is_err());
        assert!(validar_claims(&c("n", Some("a@b.com"), false), "n").is_err());
        assert!(validar_claims(&c("n", None, true), "n").is_err());
    }

    #[test]
    fn le_cookie() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            format!("a=1; {COOKIE}=abc; b=2").parse().unwrap(),
        );
        assert_eq!(cookie_token(&h).as_deref(), Some("abc"));
    }
}
