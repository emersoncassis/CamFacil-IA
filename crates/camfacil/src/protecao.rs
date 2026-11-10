//! Proteção da dashboard local contra sites de terceiros abertos no mesmo
//! navegador do técnico.
//!
//! - **DNS rebinding**: um site malicioso faz o próprio domínio apontar para
//!   127.0.0.1 e passa a ler e mexer na API como se fosse a dashboard. Por
//!   isso o cabeçalho `Host` precisa ser um endereço que o técnico realmente
//!   usa: `localhost`, um IP, um nome de rede local (sem ponto ou `.local`),
//!   o Tailscale (`*.ts.net`) ou o túnel do ngrok.
//! - **CSRF**: um site de terceiros não pode mandar pedidos para cá (por
//!   exemplo importar um backup forjado). Se o navegador informa `Origin`,
//!   ele tem que ser o próprio endereço da dashboard.
//!
//! Clientes que não são navegador (MCP, curl, Evolution API) não mandam
//! `Origin` e seguem funcionando.

use axum::{
    extract::Request,
    http::{header, HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

/// Domínios dos túneis que a dashboard usa para acesso de fora.
const TUNEIS: &[&str] = &[
    ".ts.net",
    ".ngrok-free.app",
    ".ngrok-free.dev",
    ".ngrok.app",
    ".ngrok.dev",
    ".ngrok.io",
    ".ngrok.pizza",
];

/// Separa o nome do host da porta (`exemplo:7070`, `[::1]:7070`).
fn sem_porta(autoridade: &str) -> &str {
    let a = autoridade.trim();
    if let Some(resto) = a.strip_prefix('[') {
        return resto.split(']').next().unwrap_or("");
    }
    match a.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') && p.chars().all(|c| c.is_ascii_digit()) => h,
        _ => a,
    }
}

/// `Host` aceito: só endereços que não dá para um site de fora controlar.
pub fn host_confiavel(host: &str) -> bool {
    let h = sem_porta(host).trim_end_matches('.').to_ascii_lowercase();
    if h.is_empty() {
        return false;
    }
    if h == "localhost" || h.ends_with(".localhost") || h.ends_with(".local") {
        return true;
    }
    if h.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    // Nome de máquina da rede local (ex.: `loja-pc`): sem ponto, não existe
    // no DNS público.
    if !h.contains('.') {
        return h
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    }
    TUNEIS.iter().any(|t| h.ends_with(t))
}

/// `Origin` aceito: ausente (cliente que não é navegador) ou o mesmo
/// endereço do `Host`.
pub fn origem_confiavel(origem: Option<&str>, host: Option<&str>) -> bool {
    let Some(o) = origem.map(str::trim) else {
        return true;
    };
    let Some((esquema, autoridade)) = o.split_once("://") else {
        return false; // inclui "null" (iframe isolado, arquivo local)
    };
    if !matches!(esquema, "http" | "https") {
        return false;
    }
    let autoridade = autoridade.trim_end_matches('/');
    host.is_some_and(|h| h.trim().eq_ignore_ascii_case(autoridade))
}

fn texto(h: &HeaderMap, nome: header::HeaderName) -> Option<&str> {
    h.get(nome).and_then(|v| v.to_str().ok())
}

/// Middleware aplicado a todas as rotas da dashboard local.
pub async fn navegador(req: Request, next: Next) -> Response {
    let h = req.headers();
    let host = texto(h, header::HOST);
    if host.is_some_and(|x| !host_confiavel(x)) {
        return recusar("endereço não permitido");
    }
    if !origem_confiavel(texto(h, header::ORIGIN), host) {
        return recusar("origem não permitida");
    }
    // Navegador antigo sem `Origin`: recusa pedidos que mudam algo vindos de
    // outro site.
    let muda = !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if muda && texto(h, header::HeaderName::from_static("sec-fetch-site")) == Some("cross-site") {
        return recusar("origem não permitida");
    }
    next.run(req).await
}

fn recusar(msg: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        axum::Json(serde_json::json!({ "erro": msg })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts_da_casa() {
        for h in [
            "localhost:7070",
            "LOCALHOST",
            "127.0.0.1:7070",
            "[::1]:7070",
            "192.168.0.10:7070",
            "100.64.1.2",
            "loja-pc:7070",
            "loja-pc.local",
            "loja-pc.tail1234.ts.net",
            "abc123.ngrok-free.app",
        ] {
            assert!(host_confiavel(h), "{h}");
        }
    }

    #[test]
    fn hosts_de_fora() {
        for h in [
            "atacante.com",
            "atacante.com:7070",
            "127.0.0.1.atacante.com",
            "localhost.atacante.com",
            "ts.net.atacante.com",
            "",
        ] {
            assert!(!host_confiavel(h), "{h}");
        }
    }

    #[test]
    fn origem() {
        assert!(origem_confiavel(None, Some("localhost:7070")));
        assert!(origem_confiavel(
            Some("http://localhost:7070"),
            Some("localhost:7070")
        ));
        assert!(origem_confiavel(
            Some("https://loja-pc.tail1234.ts.net"),
            Some("loja-pc.tail1234.ts.net")
        ));
        assert!(!origem_confiavel(
            Some("https://atacante.com"),
            Some("localhost:7070")
        ));
        assert!(!origem_confiavel(Some("null"), Some("localhost:7070")));
        assert!(!origem_confiavel(Some("http://localhost:7070"), None));
        assert!(!origem_confiavel(
            Some("http://localhost:8080"),
            Some("localhost:7070")
        ));
    }
}
