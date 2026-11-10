//! Acesso à dashboard pela rede (ADR 0017, SDD-05 "Segurança").
//!
//! Na própria máquina a dashboard abre direto. Qualquer outro caminho (rede
//! da loja, IP do Tailscale, Tailscale Serve/Funnel, ngrok) precisa de um
//! **aparelho autorizado**, por um destes meios:
//!
//! - **PIN**: 6 dígitos gerados pelo programa e mostrados só na tela da
//!   máquina (aba Segurança). O técnico que instala sabe; quem está longe
//!   liga para o cliente e pede. PIN fraco (sequência, repetição, data,
//!   "123456") é recusado.
//! - **Confirmação dos dois lados**: o aparelho (celular, TV) pede acesso e
//!   mostra um código; a máquina mostra o mesmo código e alguém clica em
//!   "Autorizar". O QR da aba Segurança só leva o celular até essa tela.
//!
//! O aparelho autorizado fica lembrado por 30 dias **sem uso** (cada uso
//! renova). Só a própria máquina vê o PIN, autoriza, bloqueia ou remove
//! aparelhos: quem entra de fora não consegue se autopromover.
//!
//! Defesas (OWASP ASVS V2/V3, NIST SP 800-63B): PIN cifrado no cofre, token do
//! aparelho guardado só como SHA-256, cookie `HttpOnly; SameSite=Strict`,
//! comparação em tempo constante, trava progressiva (5 erros → 5 min, depois
//! 10, 20… até 24 h), atraso a cada erro, limite de pedidos pendentes e log de
//! tudo (`acesso`, `acesso_negado`, `acesso_config`). Nada disso entra no
//! backup: restaurar um backup não libera aparelhos em outra máquina.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use cf_core::store::Store;
use cf_notify::cofre::{self, Cofre};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::Shared;

const CHAVE_PIN: &str = "acesso_pin";
const CHAVE_APARELHOS: &str = "acesso_aparelhos";
const CHAVE_BLOQUEIO: &str = "acesso_bloqueado";
const COOKIE: &str = "cf_acesso";
/// Cabeçalho alternativo ao cookie (app Android em segundo plano).
const CABECALHO: &str = "x-camfacil-acesso";
/// 30 dias sem uso.
pub const VALIDADE_S: i64 = 30 * 86_400;
const MAX_APARELHOS: usize = 50;
const TENTATIVAS: u32 = 5;
const TRAVA_BASE_S: u64 = 300;
const TRAVA_MAX_S: u64 = 86_400;
const ATRASO_ERRO: Duration = Duration::from_millis(800);
const PEDIDO_VALIDADE: Duration = Duration::from_secs(180);
const MAX_PEDIDOS: usize = 5;
/// Intervalo mínimo entre gravações do "último uso" de um aparelho.
const TOQUE: Duration = Duration::from_secs(300);

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn erro(s: StatusCode, msg: &str) -> (StatusCode, Json<Value>) {
    (s, Json(json!({ "erro": msg })))
}

fn interno<E: std::fmt::Display>(e: E) -> (StatusCode, Json<Value>) {
    erro(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string())
}

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn sha(s: &str) -> String {
    hex(Sha256::digest(s.as_bytes()).as_slice())
}

fn iguais(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn numero_aleatorio(digitos: u32) -> String {
    let b = cofre::aleatorio(8);
    let n = u64::from_le_bytes(b.try_into().unwrap_or([0; 8])) % 10u64.pow(digitos);
    format!("{n:0w$}", w = digitos as usize)
}

// ---------------- Onde está quem pede ----------------

/// Pedido feito na própria máquina, sem passar por túnel (Tailscale Serve,
/// Funnel e ngrok conectam em 127.0.0.1, mas marcam o encaminhamento).
pub fn eh_local(addr: Option<SocketAddr>, h: &HeaderMap) -> bool {
    let loopback = addr.is_none_or(|a| a.ip().is_loopback());
    loopback
        && ![
            "x-forwarded-for",
            "forwarded",
            "x-forwarded-host",
            "x-real-ip",
            "tailscale-user-login",
        ]
        .iter()
        .any(|n| h.contains_key(*n))
}

fn ip_de(addr: Option<SocketAddr>, h: &HeaderMap) -> String {
    let via_proxy = h
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(|s| s.trim().chars().take(45).collect::<String>());
    via_proxy
        .filter(|s| !s.is_empty())
        .or_else(|| addr.map(|a| a.ip().to_string()))
        .unwrap_or_else(|| "?".into())
}

fn so_local(addr: SocketAddr, h: &HeaderMap) -> Result<(), (StatusCode, Json<Value>)> {
    if eh_local(Some(addr), h) {
        Ok(())
    } else {
        Err(erro(
            StatusCode::FORBIDDEN,
            "isso só pode ser feito na tela da própria máquina",
        ))
    }
}

// ---------------- PIN ----------------

const PINS_COMUNS: &[&str] = &[
    "123456", "654321", "000000", "111111", "121212", "123123", "112233", "102030", "159753",
    "147258", "789456", "456789", "696969", "101010", "202020", "131313", "123321", "246810",
    "12345678", "87654321", "11223344", "12341234",
];

/// Motivo de recusa, ou `None` se o PIN é aceitável.
pub fn pin_fraco(p: &str) -> Option<&'static str> {
    if !(6..=8).contains(&p.len()) || !p.bytes().all(|c| c.is_ascii_digit()) {
        return Some("use de 6 a 8 números");
    }
    let d: Vec<i32> = p.bytes().map(|c| (c - b'0') as i32).collect();
    if d.iter().all(|x| *x == d[0]) {
        return Some("números todos iguais são os primeiros que um invasor tenta");
    }
    let passos: Vec<i32> = d.windows(2).map(|w| (w[1] - w[0]).rem_euclid(10)).collect();
    if passos.iter().all(|s| *s == 1) || passos.iter().all(|s| *s == 9) {
        return Some("sequência (1, 2, 3…) é o segundo palpite de qualquer robô");
    }
    for bloco in [2usize, 3, 4] {
        if p.len().is_multiple_of(bloco) && p.len() > bloco {
            let b = &p[..bloco];
            if p.as_bytes().chunks(bloco).all(|c| c == b.as_bytes()) {
                return Some("padrão repetido é fácil de adivinhar");
            }
        }
    }
    if PINS_COMUNS.contains(&p) {
        return Some("esse PIN está na lista dos mais usados do mundo");
    }
    if parece_data(p) {
        return Some("parece uma data (aniversário é fácil de descobrir)");
    }
    None
}

fn parece_data(p: &str) -> bool {
    let n = |s: &str| s.parse::<u32>().unwrap_or(0);
    let dia_mes = |dd: u32, mm: u32| (1..=31).contains(&dd) && (1..=12).contains(&mm);
    match p.len() {
        // DDMMAA
        6 => dia_mes(n(&p[0..2]), n(&p[2..4])),
        // DDMMAAAA ou AAAAMMDD
        8 => {
            (dia_mes(n(&p[0..2]), n(&p[2..4])) && (1930..=2035).contains(&n(&p[4..8])))
                || ((1930..=2035).contains(&n(&p[0..4])) && dia_mes(n(&p[6..8]), n(&p[4..6])))
        }
        _ => false,
    }
}

pub fn gerar_pin() -> String {
    loop {
        let p = numero_aleatorio(6);
        if pin_fraco(&p).is_none() {
            return p;
        }
    }
}

pub fn pin_atual(store: &Store) -> Option<String> {
    let g = store.get(CHAVE_PIN).ok().flatten()?;
    Cofre::abrir().ok()?.decifrar(&g)
}

fn definir_pin(store: &Store, pin: &str) -> Result<(), String> {
    let c = Cofre::abrir()?;
    store
        .set(CHAVE_PIN, &c.cifrar(pin))
        .map_err(|e| e.to_string())
}

/// Garante que existe um PIN (chamado ao iniciar o programa).
pub fn garantir_pin(store: &Store) {
    if pin_atual(store).is_none() {
        if let Err(e) = definir_pin(store, &gerar_pin()) {
            eprintln!("Acesso pela rede: não consegui guardar o PIN ({e}).");
        }
    }
}

// ---------------- Aparelhos ----------------

#[derive(Clone, Serialize, Deserialize)]
pub struct Aparelho {
    pub id: String,
    pub nome: String,
    /// SHA-256 do token do aparelho (o token em si só existe no aparelho).
    hash: String,
    /// "pin" ou "confirmação".
    pub metodo: String,
    pub criado_em: i64,
    pub ultimo_uso: i64,
    pub ip: String,
    #[serde(default)]
    pub bloqueado: bool,
}

fn trava_lista() -> &'static Mutex<()> {
    static T: OnceLock<Mutex<()>> = OnceLock::new();
    T.get_or_init(Mutex::default)
}

fn ler_aparelhos(store: &Store) -> Vec<Aparelho> {
    let limite = agora() - VALIDADE_S;
    store
        .get(CHAVE_APARELHOS)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Vec<Aparelho>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|a| a.ultimo_uso > limite)
        .collect()
}

fn salvar_aparelhos(store: &Store, l: &[Aparelho]) -> Result<(), String> {
    let s = serde_json::to_string(l).map_err(|e| e.to_string())?;
    store.set(CHAVE_APARELHOS, &s).map_err(|e| e.to_string())
}

fn alterar<F: FnOnce(&mut Vec<Aparelho>) -> R, R>(store: &Store, f: F) -> Result<R, String> {
    let _g = trava_lista().lock().unwrap_or_else(|e| e.into_inner());
    let mut l = ler_aparelhos(store);
    let r = f(&mut l);
    salvar_aparelhos(store, &l)?;
    Ok(r)
}

fn limpar_nome(s: &str) -> String {
    let n: String = s
        .trim()
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '<' | '>' | '"' | '\''))
        .take(40)
        .collect();
    if n.is_empty() {
        "aparelho sem nome".into()
    } else {
        n
    }
}

/// Cria o aparelho e devolve o valor do cookie (`id.token`).
fn novo_aparelho(store: &Store, nome: &str, metodo: &str, ip: &str) -> Result<String, String> {
    let id = hex(&cofre::aleatorio(6));
    let token = hex(&cofre::aleatorio(32));
    let a = Aparelho {
        id: id.clone(),
        nome: limpar_nome(nome),
        hash: sha(&token),
        metodo: metodo.into(),
        criado_em: agora(),
        ultimo_uso: agora(),
        ip: ip.into(),
        bloqueado: false,
    };
    alterar(store, |l| {
        l.push(a);
        // Mais de 50: sai o usado há mais tempo.
        while l.len() > MAX_APARELHOS {
            if let Some(i) = l
                .iter()
                .enumerate()
                .min_by_key(|(_, x)| x.ultimo_uso)
                .map(|(i, _)| i)
            {
                l.remove(i);
            }
        }
    })?;
    Ok(format!("{id}.{token}"))
}

fn credencial(h: &HeaderMap) -> Option<String> {
    if let Some(v) = h.get(CABECALHO).and_then(|v| v.to_str().ok()) {
        return Some(v.trim().to_string());
    }
    h.get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|kv| {
            kv.trim()
                .strip_prefix(&format!("{COOKIE}="))
                .map(String::from)
        })
}

fn aparelho_da_credencial(store: &Store, cred: &str) -> Option<Aparelho> {
    let (id, token) = cred.split_once('.')?;
    let h = sha(token);
    ler_aparelhos(store)
        .into_iter()
        .find(|a| a.id == id && iguais(&a.hash, &h))
}

fn cookie(valor: &str, max_age: i64, https: bool) -> HeaderValue {
    let seguro = if https { "; Secure" } else { "" };
    HeaderValue::from_str(&format!(
        "{COOKIE}={valor}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}{seguro}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static(""))
}

fn via_https(h: &HeaderMap) -> bool {
    h.get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("https"))
}

fn bloqueado_global(store: &Store) -> bool {
    store.get(CHAVE_BLOQUEIO).ok().flatten().as_deref() == Some("1")
}

fn registrar(store: &Store, tipo: &str, detalhe: &str) {
    let _ = store.registrar_evento(0, None, tipo, detalhe, agora());
}

// ---------------- Estado em memória (trava e pedidos) ----------------

#[derive(PartialEq)]
enum Situacao {
    Aguardando,
    Aprovado(String),
    Recusado,
}

struct Pedido {
    id: String,
    segredo_hash: String,
    nome: String,
    ip: String,
    codigo: String,
    criado: Instant,
    situacao: Situacao,
}

#[derive(Default)]
struct Portaria {
    erros: u32,
    travas: u32,
    travado_ate: Option<Instant>,
    pedidos: Vec<Pedido>,
    toques: HashMap<String, Instant>,
}

fn portaria() -> &'static Mutex<Portaria> {
    static P: OnceLock<Mutex<Portaria>> = OnceLock::new();
    P.get_or_init(Mutex::default)
}

fn trava_restante(p: &Portaria) -> Option<u64> {
    let agora = Instant::now();
    p.travado_ate
        .filter(|t| *t > agora)
        .map(|t| (t - agora).as_secs().max(1))
}

fn limpar_pedidos(p: &mut Portaria) {
    p.pedidos.retain(|x| x.criado.elapsed() < PEDIDO_VALIDADE);
}

// ---------------- Middleware ----------------

/// Caminhos que quem está de fora pode abrir sem aparelho autorizado.
fn livre(p: &str) -> bool {
    if p == "/mcp" || p == "/mcp/" {
        return true; // token próprio (mcp.rs)
    }
    if p.starts_with("/api/whatsapp/webhook/") && p != "/api/whatsapp/webhook/registrar" {
        return true; // token na URL (Evolution API)
    }
    if matches!(
        p,
        "/entrar" | "/api/acesso/entrar" | "/api/acesso/pedir" | "/api/acesso/eu"
    ) || p.starts_with("/api/acesso/pedido/")
    {
        return true;
    }
    // Arquivos da interface (sem dados): CSS, JS, ícones, textos.
    let estatico = !p.starts_with("/api/")
        && !p.starts_with("/mcp")
        && [".css", ".js", ".svg", ".png", ".ico", ".woff2"]
            .iter()
            .any(|e| p.ends_with(e));
    estatico || p == "/i18n/pt-BR.json"
}

fn pedir_entrada(path: &str, motivo: &str) -> Response {
    if path.starts_with("/api/") || path.starts_with("/mcp") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "erro": motivo, "entrar": "/entrar" })),
        )
            .into_response();
    }
    let volta: String = path
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '_' | '.'))
        .take(80)
        .collect();
    let mut r = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(&format!("/entrar?volta={volta}")) {
        r.headers_mut().insert(header::LOCATION, v);
    }
    r
}

/// Exige aparelho autorizado em tudo que não vem da própria máquina.
pub async fn exigir(State(app): State<Shared>, req: Request, next: Next) -> Response {
    let addr = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0);
    if eh_local(addr, req.headers()) {
        return next.run(req).await;
    }
    let path = req.uri().path().to_string();
    if livre(&path) {
        return next.run(req).await;
    }
    if bloqueado_global(&app.store) {
        return pedir_entrada(&path, "o acesso pela rede está bloqueado na máquina");
    }
    let Some(cred) = credencial(req.headers()) else {
        return pedir_entrada(&path, "este aparelho ainda não tem acesso: entre com o PIN");
    };
    let Some(a) = aparelho_da_credencial(&app.store, &cred) else {
        return pedir_entrada(&path, "acesso vencido ou removido: entre de novo");
    };
    if a.bloqueado {
        return pedir_entrada(&path, "este aparelho foi bloqueado na máquina");
    }
    // Renova o "último uso" (e o cookie) no máximo a cada 5 minutos.
    let renovar = {
        let mut p = portaria().lock().unwrap_or_else(|e| e.into_inner());
        let t = p.toques.entry(a.id.clone()).or_insert_with(|| {
            Instant::now()
                .checked_sub(TOQUE + Duration::from_secs(1))
                .unwrap_or_else(Instant::now)
        });
        if t.elapsed() >= TOQUE {
            *t = Instant::now();
            true
        } else {
            false
        }
    };
    let https = via_https(req.headers());
    if renovar {
        let ip = ip_de(addr, req.headers());
        let id = a.id.clone();
        let _ = alterar(&app.store, |l| {
            if let Some(x) = l.iter_mut().find(|x| x.id == id) {
                x.ultimo_uso = agora();
                x.ip = ip;
            }
        });
    }
    let mut r = next.run(req).await;
    if renovar {
        r.headers_mut()
            .append(header::SET_COOKIE, cookie(&cred, VALIDADE_S, https));
    }
    r
}

// ---------------- Rotas: quem está de fora ----------------

#[derive(Deserialize)]
pub struct EntrarReq {
    pin: String,
    #[serde(default)]
    nome: String,
}

/// `POST /api/acesso/entrar`: PIN → aparelho autorizado (cookie).
pub async fn post_entrar(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(r): Json<EntrarReq>,
) -> Response {
    let ip = ip_de(Some(addr), &headers);
    if let Some(s) = trava_restante(&portaria().lock().unwrap_or_else(|e| e.into_inner())) {
        registrar(
            &app.store,
            "acesso_negado",
            &format!("PIN durante a trava · {ip}"),
        );
        return erro(
            StatusCode::TOO_MANY_REQUESTS,
            &format!(
                "muitas tentativas erradas: espere {} min ou peça autorização na máquina",
                s.div_ceil(60)
            ),
        )
        .into_response();
    }
    let certo = pin_atual(&app.store).is_some_and(|p| iguais(&p, r.pin.trim()));
    if !certo {
        tokio::time::sleep(ATRASO_ERRO).await;
        let msg = {
            let mut p = portaria().lock().unwrap_or_else(|e| e.into_inner());
            p.erros += 1;
            if p.erros >= TENTATIVAS {
                p.erros = 0;
                p.travas += 1;
                let s = (TRAVA_BASE_S << (p.travas - 1).min(10)).min(TRAVA_MAX_S);
                p.travado_ate = Some(Instant::now() + Duration::from_secs(s));
                format!("PIN errado. Acesso pelo PIN travado por {} min.", s / 60)
            } else {
                format!("PIN errado ({} de {TENTATIVAS}).", p.erros)
            }
        };
        registrar(&app.store, "acesso_negado", &format!("PIN errado · {ip}"));
        return erro(StatusCode::UNAUTHORIZED, &msg).into_response();
    }
    {
        let mut p = portaria().lock().unwrap_or_else(|e| e.into_inner());
        p.erros = 0;
        p.travas = 0;
        p.travado_ate = None;
    }
    let nome = limpar_nome(&r.nome);
    let valor = match novo_aparelho(&app.store, &nome, "pin", &ip) {
        Ok(v) => v,
        Err(e) => return interno(e).into_response(),
    };
    registrar(
        &app.store,
        "acesso",
        &format!("{nome} entrou com o PIN · {ip}"),
    );
    let mut resp = Json(json!({ "ok": true, "nome": nome })).into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        cookie(&valor, VALIDADE_S, via_https(&headers)),
    );
    resp
}

#[derive(Deserialize)]
pub struct PedirReq {
    #[serde(default)]
    nome: String,
}

/// `POST /api/acesso/pedir`: o aparelho pede acesso e recebe um código para
/// comparar com o que aparece na máquina.
pub async fn post_pedir(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(r): Json<PedirReq>,
) -> ApiResult {
    let ip = ip_de(Some(addr), &headers);
    let nome = limpar_nome(&r.nome);
    let segredo = hex(&cofre::aleatorio(16));
    let id = hex(&cofre::aleatorio(6));
    let codigo = numero_aleatorio(6);
    {
        let mut p = portaria().lock().unwrap_or_else(|e| e.into_inner());
        limpar_pedidos(&mut p);
        // Um pedido por endereço; no máximo 5 esperando.
        p.pedidos
            .retain(|x| x.ip != ip || x.situacao != Situacao::Aguardando);
        if p.pedidos.len() >= MAX_PEDIDOS {
            return Err(erro(
                StatusCode::TOO_MANY_REQUESTS,
                "há pedidos demais esperando: tente de novo em alguns minutos",
            ));
        }
        p.pedidos.push(Pedido {
            id: id.clone(),
            segredo_hash: sha(&segredo),
            nome: nome.clone(),
            ip: ip.clone(),
            codigo: codigo.clone(),
            criado: Instant::now(),
            situacao: Situacao::Aguardando,
        });
    }
    registrar(&app.store, "acesso", &format!("{nome} pediu acesso · {ip}"));
    Ok(Json(json!({
        "id": id,
        "segredo": segredo,
        "codigo": codigo,
        "expira_s": PEDIDO_VALIDADE.as_secs(),
    })))
}

#[derive(Deserialize)]
pub struct SegredoQ {
    s: String,
}

/// `GET /api/acesso/pedido/{id}?s=…`: o aparelho acompanha o pedido; quando a
/// máquina autoriza, recebe o cookie uma única vez.
pub async fn get_pedido(
    Path(id): Path<String>,
    Query(q): Query<SegredoQ>,
    headers: HeaderMap,
) -> Response {
    let mut p = portaria().lock().unwrap_or_else(|e| e.into_inner());
    limpar_pedidos(&mut p);
    let h = sha(&q.s);
    let Some(i) = p
        .pedidos
        .iter()
        .position(|x| x.id == id && iguais(&x.segredo_hash, &h))
    else {
        return Json(json!({ "estado": "expirado" })).into_response();
    };
    match &p.pedidos[i].situacao {
        Situacao::Aguardando => Json(json!({
            "estado": "aguardando",
            "expira_s": PEDIDO_VALIDADE.saturating_sub(p.pedidos[i].criado.elapsed()).as_secs(),
        }))
        .into_response(),
        Situacao::Recusado => {
            p.pedidos.remove(i);
            Json(json!({ "estado": "recusado" })).into_response()
        }
        Situacao::Aprovado(valor) => {
            let c = cookie(valor, VALIDADE_S, via_https(&headers));
            p.pedidos.remove(i);
            let mut r = Json(json!({ "estado": "aprovado" })).into_response();
            r.headers_mut().insert(header::SET_COOKIE, c);
            r
        }
    }
}

/// `GET /api/acesso/eu`: este aparelho tem acesso? (usado pela tela de entrada)
pub async fn get_eu(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Json<Value> {
    if eh_local(Some(addr), &headers) {
        return Json(json!({ "local": true, "liberado": true }));
    }
    let a = credencial(&headers).and_then(|c| aparelho_da_credencial(&app.store, &c));
    let liberado = a.as_ref().is_some_and(|a| !a.bloqueado) && !bloqueado_global(&app.store);
    Json(json!({
        "local": false,
        "liberado": liberado,
        "nome": a.map(|a| a.nome),
        "bloqueado_geral": bloqueado_global(&app.store),
    }))
}

/// `POST /api/acesso/sair`: este aparelho abre mão do acesso.
pub async fn post_sair(State(app): State<Shared>, headers: HeaderMap) -> Response {
    if let Some(a) = credencial(&headers).and_then(|c| aparelho_da_credencial(&app.store, &c)) {
        let id = a.id.clone();
        let _ = alterar(&app.store, |l| l.retain(|x| x.id != id));
        registrar(&app.store, "acesso", &format!("{} saiu", a.nome));
    }
    let mut r = Json(json!({ "ok": true })).into_response();
    r.headers_mut()
        .insert(header::SET_COOKIE, cookie("", 0, via_https(&headers)));
    r
}

/// `GET /entrar`: tela de entrada para quem vem da rede.
pub async fn pagina() -> Response {
    match crate::assets::find("/entrar.html") {
        Some(a) => (
            [
                (header::CONTENT_TYPE, a.mime),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            a.body,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "não encontrado").into_response(),
    }
}

// ---------------- Rotas: só na própria máquina ----------------

async fn enderecos(app: &Shared) -> Vec<Value> {
    let porta = app
        .store
        .get("port")
        .ok()
        .flatten()
        .unwrap_or_else(|| "7070".into());
    let mut v = Vec::new();
    if std::env::args().any(|a| a == "--lan") {
        if let Some(ip) = cf_core::conectar::ip_local() {
            v.push(json!({ "rotulo": "rede da loja", "url": format!("http://{ip}:{porta}") }));
        }
    }
    let s = crate::tailscale::situacao().await;
    if let Some(ip) = s.ip {
        let base = crate::api::url_tailscale(app, ip);
        if let Some(n) = s.nome {
            v.push(json!({ "rotulo": "Tailscale", "url": base.replace(&ip.to_string(), &n) }));
        }
        v.push(json!({ "rotulo": "Tailscale (IP)", "url": base }));
    }
    v
}

/// `GET /api/acesso`: na máquina, tudo (PIN, aparelhos, pedidos); de fora,
/// só o próprio aparelho.
pub async fn get_acesso(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> ApiResult {
    if !eh_local(Some(addr), &headers) {
        let a = credencial(&headers).and_then(|c| aparelho_da_credencial(&app.store, &c));
        return Ok(Json(json!({
            "local": false,
            "aparelho": a.map(|a| json!({
                "nome": a.nome, "metodo": a.metodo,
                "expira_em": a.ultimo_uso + VALIDADE_S,
            })),
        })));
    }
    let aparelhos: Vec<Value> = ler_aparelhos(&app.store)
        .into_iter()
        .rev()
        .map(|a| {
            json!({
                "id": a.id, "nome": a.nome, "metodo": a.metodo, "ip": a.ip,
                "criado_em": a.criado_em, "ultimo_uso": a.ultimo_uso,
                "expira_em": a.ultimo_uso + VALIDADE_S, "bloqueado": a.bloqueado,
            })
        })
        .collect();
    let (pedidos, travado_s) = {
        let mut p = portaria().lock().unwrap_or_else(|e| e.into_inner());
        limpar_pedidos(&mut p);
        let l: Vec<Value> = p
            .pedidos
            .iter()
            .filter(|x| x.situacao == Situacao::Aguardando)
            .map(|x| {
                json!({
                    "id": x.id, "nome": x.nome, "ip": x.ip, "codigo": x.codigo,
                    "expira_s": PEDIDO_VALIDADE.saturating_sub(x.criado.elapsed()).as_secs(),
                })
            })
            .collect();
        (l, trava_restante(&p))
    };
    Ok(Json(json!({
        "local": true,
        "pin": pin_atual(&app.store),
        "bloqueado": bloqueado_global(&app.store),
        "validade_dias": VALIDADE_S / 86_400,
        "aparelhos": aparelhos,
        "pedidos": pedidos,
        "travado_s": travado_s,
        "enderecos": enderecos(&app).await,
    })))
}

#[derive(Deserialize)]
pub struct PinReq {
    #[serde(default)]
    pin: Option<String>,
}

/// `POST /api/acesso/pin`: gera outro PIN (ou define um, se for forte).
pub async fn post_pin(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(r): Json<PinReq>,
) -> ApiResult {
    so_local(addr, &headers)?;
    let novo = match r
        .pin
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
    {
        Some(p) => {
            if let Some(m) = pin_fraco(&p) {
                return Err(erro(StatusCode::BAD_REQUEST, m));
            }
            p
        }
        None => gerar_pin(),
    };
    definir_pin(&app.store, &novo).map_err(interno)?;
    registrar(&app.store, "acesso_config", "PIN de acesso trocado");
    Ok(Json(json!({ "pin": novo })))
}

#[derive(Deserialize)]
pub struct BloqueioReq {
    bloqueado: bool,
}

/// `POST /api/acesso/bloqueio`: bloqueia (ou libera) todo acesso pela rede.
pub async fn post_bloqueio(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(r): Json<BloqueioReq>,
) -> ApiResult {
    so_local(addr, &headers)?;
    app.store
        .set(CHAVE_BLOQUEIO, if r.bloqueado { "1" } else { "0" })
        .map_err(interno)?;
    registrar(
        &app.store,
        "acesso_config",
        if r.bloqueado {
            "acesso pela rede bloqueado"
        } else {
            "acesso pela rede liberado"
        },
    );
    Ok(Json(json!({ "bloqueado": r.bloqueado })))
}

/// `POST /api/acesso/aparelhos/{id}` `{ "bloqueado": bool }`.
pub async fn post_aparelho(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<BloqueioReq>,
) -> ApiResult {
    so_local(addr, &headers)?;
    let nome = alterar(&app.store, |l| {
        l.iter_mut().find(|x| x.id == id).map(|x| {
            x.bloqueado = r.bloqueado;
            x.nome.clone()
        })
    })
    .map_err(interno)?
    .ok_or_else(|| erro(StatusCode::NOT_FOUND, "aparelho não encontrado"))?;
    let acao = if r.bloqueado { "bloqueado" } else { "liberado" };
    registrar(&app.store, "acesso_config", &format!("{nome} {acao}"));
    Ok(Json(json!({ "ok": true })))
}

/// `DELETE /api/acesso/aparelhos/{id}`.
pub async fn delete_aparelho(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult {
    so_local(addr, &headers)?;
    let nome = alterar(&app.store, |l| {
        let n = l.iter().find(|x| x.id == id).map(|x| x.nome.clone());
        l.retain(|x| x.id != id);
        n
    })
    .map_err(interno)?
    .ok_or_else(|| erro(StatusCode::NOT_FOUND, "aparelho não encontrado"))?;
    registrar(&app.store, "acesso_config", &format!("{nome} removido"));
    Ok(Json(json!({ "ok": true })))
}

/// `POST /api/acesso/desconectar`: remove todos os aparelhos.
pub async fn post_desconectar(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> ApiResult {
    so_local(addr, &headers)?;
    let n = alterar(&app.store, |l| {
        let n = l.len();
        l.clear();
        n
    })
    .map_err(interno)?;
    registrar(
        &app.store,
        "acesso_config",
        &format!("{n} aparelho(s) desconectado(s)"),
    );
    Ok(Json(json!({ "removidos": n })))
}

#[derive(Deserialize)]
pub struct DecisaoReq {
    aprovar: bool,
}

/// `POST /api/acesso/pedidos/{id}`: a máquina autoriza ou recusa.
pub async fn post_decidir(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<DecisaoReq>,
) -> ApiResult {
    so_local(addr, &headers)?;
    let (nome, ip) = {
        let mut p = portaria().lock().unwrap_or_else(|e| e.into_inner());
        limpar_pedidos(&mut p);
        let x = p
            .pedidos
            .iter_mut()
            .find(|x| x.id == id && x.situacao == Situacao::Aguardando)
            .ok_or_else(|| erro(StatusCode::NOT_FOUND, "pedido não encontrado ou vencido"))?;
        if !r.aprovar {
            x.situacao = Situacao::Recusado;
        }
        (x.nome.clone(), x.ip.clone())
    };
    if !r.aprovar {
        registrar(
            &app.store,
            "acesso_negado",
            &format!("pedido de {nome} recusado · {ip}"),
        );
        return Ok(Json(json!({ "ok": true })));
    }
    let valor = novo_aparelho(&app.store, &nome, "confirmação", &ip).map_err(interno)?;
    {
        let mut p = portaria().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(x) = p.pedidos.iter_mut().find(|x| x.id == id) {
            x.situacao = Situacao::Aprovado(valor);
        }
    }
    registrar(
        &app.store,
        "acesso",
        &format!("{nome} autorizado na máquina · {ip}"),
    );
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct QrQ {
    #[serde(default)]
    e: usize,
}

/// `GET /api/acesso/qr.svg?e=N`: QR que leva o celular até a tela de entrada.
pub async fn get_qr(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(q): Query<QrQ>,
) -> Response {
    if so_local(addr, &headers).is_err() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let lista = enderecos(&app).await;
    let svg = lista
        .get(q.e)
        .and_then(|v| v["url"].as_str())
        .and_then(|u| crate::elo::qr_svg(&format!("{u}/entrar")));
    match svg {
        Some(s) => (
            [
                (header::CONTENT_TYPE, "image/svg+xml"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            s,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "sem endereço de rede").into_response(),
    }
}

/// `GET /api/acesso/registro`: últimos acessos, recusas e mudanças.
pub async fn get_registro(State(app): State<Shared>) -> ApiResult {
    let mut l = Vec::new();
    for tipo in [
        "acesso",
        "acesso_negado",
        "acesso_config",
        "mcp",
        "mcp_negado",
    ] {
        l.extend(app.store.eventos_tipo(tipo, 60).map_err(interno)?);
    }
    l.sort_by_key(|e| std::cmp::Reverse((e.quando, e.id)));
    l.truncate(60);
    let v: Vec<Value> = l
        .into_iter()
        .map(|e| json!({ "tipo": e.tipo, "detalhe": e.detalhe, "quando": e.quando }))
        .collect();
    Ok(Json(json!({ "eventos": v })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_fracos() {
        for p in [
            "123456",
            "654321",
            "000000",
            "999999",
            "121212",
            "123123",
            "112233",
            "890123",
            "210987",
            "250190",
            "25011990",
            "19900125",
            "12345",
            "abcdef",
            "123456789",
        ] {
            assert!(pin_fraco(p).is_some(), "{p} deveria ser fraco");
        }
        for p in ["482913", "730518", "95827163", "604738"] {
            assert!(pin_fraco(p).is_none(), "{p} deveria passar");
        }
    }

    #[test]
    fn pin_gerado_e_forte() {
        for _ in 0..200 {
            let p = gerar_pin();
            assert_eq!(p.len(), 6);
            assert!(pin_fraco(&p).is_none(), "{p}");
        }
    }

    #[test]
    fn local_so_sem_tunel() {
        let lo: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let lan: SocketAddr = "192.168.0.20:5000".parse().unwrap();
        let ts: SocketAddr = "100.64.1.2:5000".parse().unwrap();
        let vazio = HeaderMap::new();
        assert!(eh_local(Some(lo), &vazio));
        assert!(!eh_local(Some(lan), &vazio));
        assert!(!eh_local(Some(ts), &vazio));
        let mut tunel = HeaderMap::new();
        tunel.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.9"));
        assert!(
            !eh_local(Some(lo), &tunel),
            "Tailscale Serve/ngrok não é local"
        );
    }

    #[test]
    fn caminhos_livres() {
        assert!(livre("/entrar"));
        assert!(livre("/app.css"));
        assert!(livre("/i18n/pt-BR.json"));
        assert!(livre("/mcp"));
        assert!(livre("/api/whatsapp/webhook/abc"));
        assert!(livre("/api/acesso/pedido/abc"));
        assert!(!livre("/"));
        assert!(!livre("/tv"));
        assert!(
            !livre("/api/elo/qr.svg"),
            "API com extensão não é arquivo estático"
        );
        assert!(!livre("/api/cameras/1/foto.jpg"));
        assert!(!livre("/api/whatsapp/webhook/registrar"));
        assert!(!livre("/api/acesso"));
        assert!(!livre("/api/acesso/pin"));
    }

    #[test]
    fn credencial_do_cookie_e_do_cabecalho() {
        let mut h = HeaderMap::new();
        h.insert(
            header::COOKIE,
            HeaderValue::from_static("outro=1; cf_acesso=ab.cd; x=2"),
        );
        assert_eq!(credencial(&h).as_deref(), Some("ab.cd"));
        let mut h2 = HeaderMap::new();
        h2.insert(CABECALHO, HeaderValue::from_static("ef.gh"));
        assert_eq!(credencial(&h2).as_deref(), Some("ef.gh"));
    }

    #[test]
    fn acesso_fica_fora_do_backup() {
        for k in [CHAVE_PIN, CHAVE_APARELHOS, CHAVE_BLOQUEIO] {
            assert!(cf_core::store::chave_local(k), "{k} não pode ir no backup");
        }
    }

    #[test]
    fn nome_sem_html() {
        assert_eq!(limpar_nome("<script>Ana</script>"), "scriptAna/script");
        assert_eq!(limpar_nome("  "), "aparelho sem nome");
    }
}
