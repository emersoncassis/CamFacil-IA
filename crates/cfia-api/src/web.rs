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
    // Escolinha de IA: aprender do zero, no navegador.
    arq!("/aprender", "text/html; charset=utf-8", "web/escola.html"),
    arq!(
        "/escola.js",
        "text/javascript; charset=utf-8",
        "web/escola.js"
    ),
    arq!("/escola.css", "text/css; charset=utf-8", "web/escola.css"),
    arq!("/selo.js", "text/javascript; charset=utf-8", "web/selo.js"),
    arq!("/selo", "text/html; charset=utf-8", "web/selo.html"),
    arq!("/img/loja.webp", "image/webp", "web/img/loja.webp"),
    arq!("/img/pessoa.webp", "image/webp", "web/img/pessoa.webp"),
    arq!(
        "/img/pessoa-frente.webp",
        "image/webp",
        "web/img/pessoa-frente.webp"
    ),
    arq!(
        "/img/pessoa-costas.webp",
        "image/webp",
        "web/img/pessoa-costas.webp"
    ),
    arq!("/img/selo-og.png", "image/png", "web/img/selo-og.png"),
    // Narração na voz do autor (Voice Studio, Opus mono 24 kbps).
    arq!(
        "/audio/escola-00-abertura.ogg",
        "audio/ogg",
        "web/audio/escola-00-abertura.ogg"
    ),
    arq!(
        "/audio/escola-01.ogg",
        "audio/ogg",
        "web/audio/escola-01.ogg"
    ),
    arq!(
        "/audio/escola-02.ogg",
        "audio/ogg",
        "web/audio/escola-02.ogg"
    ),
    arq!(
        "/audio/escola-03.ogg",
        "audio/ogg",
        "web/audio/escola-03.ogg"
    ),
    arq!(
        "/audio/escola-04.ogg",
        "audio/ogg",
        "web/audio/escola-04.ogg"
    ),
    arq!(
        "/audio/escola-05.ogg",
        "audio/ogg",
        "web/audio/escola-05.ogg"
    ),
    arq!(
        "/audio/escola-06.ogg",
        "audio/ogg",
        "web/audio/escola-06.ogg"
    ),
    arq!(
        "/audio/escola-acerto-linha.ogg",
        "audio/ogg",
        "web/audio/escola-acerto-linha.ogg"
    ),
    arq!(
        "/audio/escola-acerto-saude.ogg",
        "audio/ogg",
        "web/audio/escola-acerto-saude.ogg"
    ),
    arq!(
        "/audio/escola-acerto-zona.ogg",
        "audio/ogg",
        "web/audio/escola-acerto-zona.ogg"
    ),
    arq!(
        "/audio/escola-erro-borda.ogg",
        "audio/ogg",
        "web/audio/escola-erro-borda.ogg"
    ),
    arq!(
        "/audio/escola-erro-caminho.ogg",
        "audio/ogg",
        "web/audio/escola-erro-caminho.ogg"
    ),
    arq!(
        "/audio/escola-erro-curta.ogg",
        "audio/ogg",
        "web/audio/escola-erro-curta.ogg"
    ),
    arq!(
        "/audio/escola-erro-zona-falso.ogg",
        "audio/ogg",
        "web/audio/escola-erro-zona-falso.ogg"
    ),
    arq!(
        "/audio/escola-erro-zona-vazia.ogg",
        "audio/ogg",
        "web/audio/escola-erro-zona-vazia.ogg"
    ),
    arq!(
        "/audio/escola-avance.ogg",
        "audio/ogg",
        "web/audio/escola-avance.ogg"
    ),
    arq!(
        "/audio/escola-selo.ogg",
        "audio/ogg",
        "web/audio/escola-selo.ogg"
    ),
    arq!(
        "/audio/site-01-boas-vindas.ogg",
        "audio/ogg",
        "web/audio/site-01-boas-vindas.ogg"
    ),
    arq!(
        "/audio/site-02-por-que.ogg",
        "audio/ogg",
        "web/audio/site-02-por-que.ogg"
    ),
    arq!(
        "/audio/site-03-passo-a-passo.ogg",
        "audio/ogg",
        "web/audio/site-03-passo-a-passo.ogg"
    ),
    arq!(
        "/audio/site-04-download.ogg",
        "audio/ogg",
        "web/audio/site-04-download.ogg"
    ),
    arq!(
        "/audio/site-05-painel.ogg",
        "audio/ogg",
        "web/audio/site-05-painel.ogg"
    ),
    arq!(
        "/privacidade",
        "text/html; charset=utf-8",
        "web/privacidade.html"
    ),
    arq!("/termos", "text/html; charset=utf-8", "web/termos.html"),
    arq!(
        "/audio/manifesto-01.ogg",
        "audio/ogg",
        "web/audio/manifesto-01.ogg"
    ),
    arq!(
        "/audio/manifesto-02.ogg",
        "audio/ogg",
        "web/audio/manifesto-02.ogg"
    ),
    arq!(
        "/audio/manifesto-03.ogg",
        "audio/ogg",
        "web/audio/manifesto-03.ogg"
    ),
    arq!(
        "/audio/manifesto-04.ogg",
        "audio/ogg",
        "web/audio/manifesto-04.ogg"
    ),
    arq!(
        "/manifesto.js",
        "text/javascript; charset=utf-8",
        "web/manifesto.js"
    ),
    arq!(
        "/manifesto",
        "text/html; charset=utf-8",
        "web/manifesto.html"
    ),
    arq!(
        "/img/manifesto-og.png",
        "image/png",
        "web/img/manifesto-og.png"
    ),
    arq!(
        "/img/formacao-og.png",
        "image/png",
        "web/img/formacao-og.png"
    ),
    arq!("/estudio", "text/html; charset=utf-8", "web/estudio.html"),
    arq!("/doc.js", "text/javascript; charset=utf-8", "web/doc.js"),
    arq!("/estudio.css", "text/css; charset=utf-8", "web/estudio.css"),
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
        p if p.starts_with("/selo/") => "/selo",
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
