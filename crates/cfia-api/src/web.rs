//! Web app estático do domínio, embutido no binário.

use axum::{
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};

struct Arquivo {
    caminho: &'static str,
    tipo: &'static str,
    corpo: &'static [u8],
}

macro_rules! arq {
    ($c:literal, $t:literal, $f:literal) => {
        Arquivo {
            caminho: $c,
            tipo: $t,
            corpo: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $f)),
        }
    };
}

const ARQUIVOS: &[Arquivo] = &[
    arq!("/", "text/html; charset=utf-8", "web/index.html"),
    arq!("/app.js", "text/javascript; charset=utf-8", "web/app.js"),
    arq!("/app.css", "text/css; charset=utf-8", "web/app.css"),
    arq!(
        "/tokens.css",
        "text/css; charset=utf-8",
        "../../web/tokens.css"
    ),
    arq!("/icon.svg", "image/svg+xml", "../../web/icon.svg"),
    arq!("/img/loja.webp", "image/webp", "web/img/loja.webp"),
    arq!("/img/pessoa.webp", "image/webp", "web/img/pessoa.webp"),
    arq!(
        "/privacidade",
        "text/html; charset=utf-8",
        "web/privacidade.html"
    ),
    arq!("/termos", "text/html; charset=utf-8", "web/termos.html"),
    // Instaladores de um comando (curl | sh, irm | iex).
    arq!(
        "/instalar.sh",
        "text/plain; charset=utf-8",
        "../../scripts/install.sh"
    ),
    arq!(
        "/instalar.ps1",
        "text/plain; charset=utf-8",
        "../../scripts/install.ps1"
    ),
    arq!(
        "/instalar-painel.sh",
        "text/plain; charset=utf-8",
        "../../scripts/instalar-painel.sh"
    ),
];

pub async fn estatico(uri: Uri) -> Response {
    // Rotas do app de página única caem no index.
    let p = match uri.path() {
        "/parear" | "/agentes" | "/index.html" => "/",
        p => p,
    };
    match ARQUIVOS.iter().find(|a| a.caminho == p) {
        Some(a) => (
            [
                (header::CONTENT_TYPE, a.tipo),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            a.corpo,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "não encontrado").into_response(),
    }
}
