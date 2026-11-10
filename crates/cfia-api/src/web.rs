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
    // Apoio: mesmo script e QR do site de documentação (uma fonte só).
    arq!(
        "/apoio.js",
        "text/javascript; charset=utf-8",
        "../../site-docs/assets/js/apoio.js"
    ),
    arq!(
        "/mural.json",
        "application/json; charset=utf-8",
        "../../site-docs/assets/mural.json"
    ),
    arq!(
        "/img/pix-qr.svg",
        "image/svg+xml",
        "../../site-docs/assets/img/pix-qr.svg"
    ),
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
    arq!(
        "/estudio.js",
        "text/javascript; charset=utf-8",
        "web/estudio.js"
    ),
    // Seção "Por dentro" da landing: cartão do mapa e diagramas (SVG puro).
    arq!(
        "/img/mapa-interativo.svg",
        "image/svg+xml",
        "../../docs/assets/mapa-interativo.svg"
    ),
    arq!(
        "/img/diagrama-como.svg",
        "image/svg+xml",
        "web/img/diagrama-como.svg"
    ),
    arq!(
        "/img/diagrama-seguranca.svg",
        "image/svg+xml",
        "web/img/diagrama-seguranca.svg"
    ),
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

/// `/baixar/<arquivo>`: manda para o arquivo da versão atual com o número da
/// versão no nome (ex.: camfacil-0.8.2-windows-x86_64.exe), para a pessoa
/// saber qual versão baixou. O site e o programa saem da mesma versão.
pub async fn baixar(axum::extract::Path(arquivo): axum::extract::Path<String>) -> Response {
    const PERMITIDOS: &[&str] = &[
        "camfacil-windows-x86_64.exe",
        "camfacil-linux-x86_64",
        "camfacil-linux-arm64",
        "camfacil-android.apk",
    ];
    if !PERMITIDOS.contains(&arquivo.as_str()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let v = env!("CARGO_PKG_VERSION");
    let nome = arquivo.replacen("camfacil-", &format!("camfacil-{v}-"), 1);
    let url = format!("https://github.com/emersoncassis/CamFacil-IA/releases/download/v{v}/{nome}");
    (
        StatusCode::FOUND,
        [
            (header::LOCATION, url),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
    )
        .into_response()
}

/// Páginas públicas do site, na ordem em que aparecem no sitemap.
const PUBLICAS: &[&str] = &[
    "/",
    "/aprender",
    "/manifesto",
    "/estudio",
    "/privacidade",
    "/termos",
];

/// `GET /robots.txt`: o site é público; a API não precisa ser indexada.
pub async fn robots(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
) -> Response {
    let corpo = format!(
        "User-agent: *\nAllow: /\nDisallow: /api/\nDisallow: /ws/\n\nSitemap: {}/sitemap.xml\n",
        state.base_url
    );
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], corpo).into_response()
}

/// `GET /sitemap.xml`: páginas públicas com o endereço deste servidor.
pub async fn sitemap(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
) -> Response {
    let urls: String = PUBLICAS
        .iter()
        .map(|p| format!("  <url><loc>{}{}</loc></url>\n", state.base_url, p))
        .collect();
    let corpo = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n{urls}</urlset>\n"
    );
    (
        [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
        corpo,
    )
        .into_response()
}
