//! Dashboard estática embutida no binário (sem build de Node, sem arquivos soltos).

pub struct Asset {
    pub path: &'static str,
    pub mime: &'static str,
    pub body: &'static [u8],
}

macro_rules! asset {
    ($path:literal, $mime:literal, $file:literal) => {
        Asset {
            path: $path,
            mime: $mime,
            body: include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/", $file)),
        }
    };
}

pub const ASSETS: &[Asset] = &[
    asset!("/", "text/html; charset=utf-8", "index.html"),
    asset!("/tokens.css", "text/css; charset=utf-8", "tokens.css"),
    asset!("/app.css", "text/css; charset=utf-8", "app.css"),
    asset!("/app.js", "text/javascript; charset=utf-8", "app.js"),
    asset!(
        "/integracoes-extra.css",
        "text/css; charset=utf-8",
        "integracoes-extra.css"
    ),
    asset!(
        "/integracoes-extra.js",
        "text/javascript; charset=utf-8",
        "integracoes-extra.js"
    ),
    asset!(
        "/i18n/pt-BR.json",
        "application/json; charset=utf-8",
        "i18n/pt-BR.json"
    ),
    asset!("/icon.svg", "image/svg+xml", "icon.svg"),
    // Fase 5: Modo TV e tela Manutenção.
    asset!("/tv.html", "text/html; charset=utf-8", "tv.html"),
    asset!("/tv.css", "text/css; charset=utf-8", "tv.css"),
    asset!("/tv.js", "text/javascript; charset=utf-8", "tv.js"),
    asset!(
        "/manutencao.css",
        "text/css; charset=utf-8",
        "manutencao.css"
    ),
    asset!(
        "/manutencao.js",
        "text/javascript; charset=utf-8",
        "manutencao.js"
    ),
    asset!("/rede.css", "text/css; charset=utf-8", "rede.css"),
    asset!("/rede.js", "text/javascript; charset=utf-8", "rede.js"),
    asset!("/ao-vivo", "text/html; charset=utf-8", "aovivo.html"),
    asset!("/aovivo.js", "text/javascript; charset=utf-8", "aovivo.js"),
    asset!("/botoes.html", "text/html; charset=utf-8", "botoes.html"),
    asset!("/botoes.js", "text/javascript; charset=utf-8", "botoes.js"),
    asset!("/botoes.css", "text/css; charset=utf-8", "botoes.css"),
    asset!("/fase6.js", "text/javascript; charset=utf-8", "fase6.js"),
    // Tela única (zero rolagem): sub-abas, acordeões e paginação.
    asset!("/tela.css", "text/css; charset=utf-8", "tela.css"),
    asset!("/tela.js", "text/javascript; charset=utf-8", "tela.js"),
];

pub fn find(path: &str) -> Option<&'static Asset> {
    let path = if path == "/index.html" { "/" } else { path };
    ASSETS.iter().find(|a| a.path == path)
}
