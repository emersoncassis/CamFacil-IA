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
        "/i18n/pt-BR.json",
        "application/json; charset=utf-8",
        "i18n/pt-BR.json"
    ),
    asset!("/icon.svg", "image/svg+xml", "icon.svg"),
];

pub fn find(path: &str) -> Option<&'static Asset> {
    let path = if path == "/index.html" { "/" } else { path };
    ASSETS.iter().find(|a| a.path == path)
}
