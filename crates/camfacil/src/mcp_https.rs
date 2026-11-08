//! Endereço HTTPS do MCP pelo Tailscale (ADR 0015, SDD-04).
//!
//! O Claude (conectores remotos do claude.ai e do app) não aceita HTTP nem
//! certificado autoassinado. O Tailscale emite um certificado válido para
//! `<máquina>.<tailnet>.ts.net` e faz o proxy até `127.0.0.1:<porta>/mcp`:
//! - **Serve**: só aparelhos da rede Tailscale do técnico.
//! - **Funnel**: público na internet (o claude.ai chama a partir da nuvem).
//!   Exige confirmação forte, MCP ligado e token; fica registrado em evento.
//!
//! Só o caminho `/mcp` é publicado e só ele é desligado: o resto da
//! configuração de Serve/Funnel do usuário fica como está.

use crate::api::{bad, ApiResult};
use crate::tailscale::{self as ts, Modo};
use crate::Shared;
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};

fn porta_local(app: &Shared) -> u16 {
    app.store
        .get("port")
        .ok()
        .flatten()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7070)
}

fn registrar(app: &Shared, detalhe: &str) {
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let _ = app.store.registrar_evento(0, None, "mcp", detalhe, agora);
    println!("MCP: {detalhe}");
}

/// Situação para a tela: o que dá para fazer e o que já está publicado.
async fn situacao(app: &Shared) -> Value {
    let porta = porta_local(app);
    let mcp_ligado = crate::mcp::ligado(app);
    let base = json!({
        "sistema": std::env::consts::OS,
        "android": cfg!(target_os = "android"),
        "mcp_ligado": mcp_ligado,
        "links": { "dns": ts::PAINEL_DNS, "funnel": ts::PAINEL_FUNNEL },
        "versao_minima": format!("{}.{}", ts::VERSAO_MINIMA.0, ts::VERSAO_MINIMA.1),
    });
    let mut r = base;
    if cfg!(target_os = "android") || ts::binario().is_none() {
        r["instalado"] = json!(!cfg!(target_os = "android") && ts::binario().is_some());
        return r;
    }
    r["instalado"] = json!(true);
    let versao = ts::versao().await;
    r["versao"] = json!(versao.map(|(a, b, c)| format!("{a}.{b}.{c}")));
    // Sem versão legível (muito antigo) tratamos como antigo.
    r["versao_ok"] = json!(versao.is_some_and(ts::versao_ok));
    let rec = match ts::recursos().await {
        Ok(x) => x,
        Err(e) => {
            r["erro"] = json!(ts::explicar_erro(&e));
            return r;
        }
    };
    r["logado"] = json!(rec.nome.is_some());
    r["nome"] = json!(rec.nome);
    r["https_ok"] = json!(rec.https);
    r["funnel_ok"] = json!(rec.funnel);
    r["funnel_portas"] = json!(rec.portas_funnel);
    let cfg = ts::config_serve().await.unwrap_or_default();
    let publicado = cfg.mcp();
    r["publicado"] = match (publicado, rec.nome.as_deref()) {
        (Some((p, funnel)), Some(nome)) => json!({
            "modo": if funnel { "funnel" } else { "serve" },
            "porta": p,
            "url": ts::url_https(nome, p),
        }),
        _ => Value::Null,
    };
    // Comandos equivalentes, para quem prefere rodar à mão.
    let p_serve = cfg.escolher_porta(Modo::Serve, &[]).unwrap_or(443);
    let p_funnel = cfg
        .escolher_porta(Modo::Funnel, &rec.portas_funnel)
        .unwrap_or(443);
    let linha = |a: Vec<String>| format!("tailscale {}", a.join(" "));
    r["comandos"] = json!({
        "serve": linha(ts::args_publicar(Modo::Serve, p_serve, porta)),
        "funnel": linha(ts::args_publicar(Modo::Funnel, p_funnel, porta)),
        "desligar": publicado.map(|(p, f)| linha(ts::args_despublicar(if f { Modo::Funnel } else { Modo::Serve }, p))),
    });
    r
}

/// `GET /api/mcp/https`.
pub async fn get_https(State(app): State<Shared>) -> Json<Value> {
    Json(situacao(&app).await)
}

#[derive(Deserialize)]
pub struct Pedido {
    /// `serve`, `funnel` ou `off`.
    modo: String,
    /// O técnico confirmou o aviso de internet (obrigatório no Funnel).
    #[serde(default)]
    confirmado: bool,
}

/// Tira só o `/mcp` do Serve/Funnel, onde estiver.
async fn despublicar(app: &Shared) -> Result<bool, String> {
    let Some((p, funnel)) = ts::config_serve().await?.mcp() else {
        return Ok(false);
    };
    let modo = if funnel { Modo::Funnel } else { Modo::Serve };
    ts::aplicar(&ts::args_despublicar(modo, p)).await?;
    registrar(
        app,
        &format!("endereço HTTPS desligado ({}, porta {p})", modo.cmd()),
    );
    Ok(true)
}

/// Desligar o MCP desliga também o endereço HTTPS (Tailscale e ngrok do
/// `/mcp`). Melhor esforço: erro aqui não impede desligar o MCP.
pub async fn despublicar_tudo(app: &Shared) {
    if !cfg!(target_os = "android") && ts::binario().is_some() {
        if let Err(e) = despublicar(app).await {
            eprintln!("MCP: não deu para tirar o /mcp do Tailscale: {e}");
        }
    }
    crate::ngrok::parar_se_mcp(app);
}

/// `POST /api/mcp/https`: `{modo: serve|funnel|off, confirmado}`.
pub async fn post_https(State(app): State<Shared>, Json(p): Json<Pedido>) -> ApiResult {
    if cfg!(target_os = "android") {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            "no Android o Serve e o Funnel do Tailscale não existem; use o app Tailscale só para a rede",
        ));
    }
    if ts::binario().is_none() {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            "Tailscale não instalado neste computador",
        ));
    }
    let modo = match p.modo.as_str() {
        "off" => {
            despublicar(&app)
                .await
                .map_err(|e| bad(StatusCode::BAD_GATEWAY, e))?;
            return Ok(Json(situacao(&app).await));
        }
        "serve" => Modo::Serve,
        "funnel" => Modo::Funnel,
        _ => return Err(bad(StatusCode::BAD_REQUEST, "modo inválido")),
    };
    crate::termos::exigir_tecnico(&app)?;
    if !crate::mcp::ligado(&app) {
        return Err(bad(StatusCode::BAD_REQUEST, "ligue o MCP primeiro"));
    }
    if !crate::mcp::tem_token(&app) {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            "o MCP está sem token; gere um token",
        ));
    }
    if modo == Modo::Funnel && !p.confirmado {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            "o Funnel publica o MCP na internet: confirme o aviso antes",
        ));
    }
    if !ts::versao().await.is_some_and(ts::versao_ok) {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            format!(
                "este Tailscale é antigo; atualize para a {}.{} ou mais nova",
                ts::VERSAO_MINIMA.0,
                ts::VERSAO_MINIMA.1
            ),
        ));
    }
    let rec = ts::recursos()
        .await
        .map_err(|e| bad(StatusCode::BAD_GATEWAY, ts::explicar_erro(&e)))?;
    if rec.nome.is_none() {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            "o Tailscale não está conectado",
        ));
    }
    if !rec.https {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            format!(
                "ligue os certificados HTTPS do tailnet em {} (HTTPS Certificates)",
                ts::PAINEL_DNS
            ),
        ));
    }
    if modo == Modo::Funnel && !rec.funnel {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            format!(
                "o Funnel não está liberado para esta máquina; libere na política em {}",
                ts::PAINEL_FUNNEL
            ),
        ));
    }
    let cfg = ts::config_serve()
        .await
        .map_err(|e| bad(StatusCode::BAD_GATEWAY, ts::explicar_erro(&e)))?;
    // Trocar de modo: tira o /mcp antigo antes (só ele).
    if let Some((_, funnel)) = cfg.mcp() {
        if funnel != (modo == Modo::Funnel) {
            despublicar(&app)
                .await
                .map_err(|e| bad(StatusCode::BAD_GATEWAY, e))?;
        }
    }
    let cfg = ts::config_serve().await.unwrap_or(cfg);
    let porta = cfg
        .mcp()
        .filter(|(_, f)| *f == (modo == Modo::Funnel))
        .map(|(p, _)| p)
        .or_else(|| cfg.escolher_porta(modo, &rec.portas_funnel))
        .ok_or_else(|| {
            bad(
                StatusCode::CONFLICT,
                "todas as portas HTTPS (443, 8443, 10000) já têm outras coisas publicadas; libere uma no Tailscale",
            )
        })?;
    ts::aplicar(&ts::args_publicar(modo, porta, porta_local(&app)))
        .await
        .map_err(|e| bad(StatusCode::BAD_GATEWAY, e))?;
    let nome = rec.nome.unwrap_or_default();
    registrar(
        &app,
        &format!(
            "endereço HTTPS ligado ({}{}): {}",
            modo.cmd(),
            if modo == Modo::Funnel {
                ", público na internet"
            } else {
                ", só a rede Tailscale"
            },
            ts::url_https(&nome, porta)
        ),
    );
    Ok(Json(situacao(&app).await))
}
