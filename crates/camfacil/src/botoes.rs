//! Botões de portão e relé (SDD-06, regra 7 do CLAUDE.md).
//!
//! O técnico cadastra um "botão" = uma chamada HTTP a um relé de rede,
//! controlador de acesso ou automação (URL, método, usuário/senha opcionais).
//! Quem executa é este programa, na rede da loja; o celular só pede.
//!
//! Segurança:
//! - a ação inteira (URL, usuário, senha, corpo) fica **cifrada** pelo cofre,
//!   como o token do Telegram; a lista só mostra o endereço sem segredos;
//! - acionar exige **PIN** (PBKDF2, nunca guardado em claro) e depois uma
//!   **confirmação** com um código de uso único que vale 20 s;
//! - 5 PINs errados travam os botões por 5 min;
//! - todo acionamento (e toda recusa) vira evento `acionamento` /
//!   `acionamento_negado` com quem, de onde e o resultado;
//! - o MCP **não** aciona botões (só lista).

use crate::api::{bad, internal, ApiResult};
use crate::Shared;
use axum::{
    extract::{ConnectInfo, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use cf_core::store::Store;
use cf_notify::cofre::{self, Cofre};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const CHAVE_LISTA: &str = "botoes";
const CHAVE_PIN: &str = "botoes_pin";
const ITER_PIN: u32 = 60_000;
const TENTATIVAS: u32 = 5;
const TRAVA: Duration = Duration::from_secs(300);
const VALIDADE_CONFIRMACAO: Duration = Duration::from_secs(20);
pub const MAX_BOTOES: usize = 12;

/// O que aparece na lista (sem segredos).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Botao {
    pub id: i64,
    pub nome: String,
    /// `portao`, `rele` ou `luz` (só muda o ícone).
    pub icone: String,
    pub criado_em: i64,
}

/// A ação HTTP, guardada cifrada.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Acao {
    pub url: String,
    pub metodo: String,
    #[serde(default)]
    pub usuario: String,
    #[serde(default)]
    pub senha: String,
    #[serde(default)]
    pub corpo: String,
}

fn chave_acao(id: i64) -> String {
    format!("botao_acao_{id}")
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

pub fn listar(store: &Store) -> Vec<Botao> {
    store
        .get(CHAVE_LISTA)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn salvar_lista(store: &Store, l: &[Botao]) -> Result<(), String> {
    let s = serde_json::to_string(l).map_err(|e| e.to_string())?;
    store.set(CHAVE_LISTA, &s).map_err(|e| e.to_string())
}

fn ler_acao(store: &Store, cofre: &Cofre, id: i64) -> Option<Acao> {
    let g = store.get(&chave_acao(id)).ok().flatten()?;
    serde_json::from_str(&cofre.decifrar(&g)?).ok()
}

/// Endereço sem usuário/senha nem parâmetros (podem levar token).
pub fn url_sem_segredos(url: &str) -> String {
    let (esquema, resto) = url.split_once("://").unwrap_or(("http", url));
    let autoridade = resto.split(['/', '?', '#']).next().unwrap_or("");
    let host = autoridade.rsplit('@').next().unwrap_or("");
    let caminho = resto[autoridade.len()..]
        .split(['?', '#'])
        .next()
        .unwrap_or("");
    let extra = if resto[autoridade.len()..].contains('?') {
        "?…"
    } else {
        ""
    };
    format!("{esquema}://{host}{caminho}{extra}")
}

/// Valida e normaliza uma ação vinda da tela.
pub fn validar_acao(a: &Acao) -> Result<Acao, String> {
    let url = a.url.trim().to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) || url.len() < 10 {
        return Err("o endereço precisa começar com http:// ou https://".into());
    }
    if url.len() > 500 || url.chars().any(char::is_whitespace) {
        return Err("endereço inválido".into());
    }
    let metodo = a.metodo.trim().to_uppercase();
    if !matches!(metodo.as_str(), "GET" | "POST" | "PUT") {
        return Err("método deve ser GET, POST ou PUT".into());
    }
    if a.corpo.len() > 2000 {
        return Err("corpo grande demais (máx. 2000 caracteres)".into());
    }
    Ok(Acao {
        url,
        metodo,
        usuario: a.usuario.trim().chars().take(80).collect(),
        senha: a.senha.chars().take(120).collect(),
        corpo: a.corpo.clone(),
    })
}

// ---------------- PIN ----------------

fn pin_valido(pin: &str) -> bool {
    (4..=8).contains(&pin.len()) && pin.chars().all(|c| c.is_ascii_digit())
}

/// `pbkdf2$iter$sal$hash` (hex).
fn hash_pin(pin: &str, iter: u32) -> String {
    let sal = cofre::aleatorio(16);
    let h = crate::backup::pbkdf2_sha256(pin.as_bytes(), &sal, iter);
    format!("pbkdf2${iter}${}${}", hex(&sal), hex(&h))
}

fn conferir_pin(guardado: &str, pin: &str) -> bool {
    let p: Vec<&str> = guardado.split('$').collect();
    if p.len() != 4 || p[0] != "pbkdf2" {
        return false;
    }
    let Ok(iter) = p[1].parse::<u32>() else {
        return false;
    };
    let de_hex = |s: &str| -> Option<Vec<u8>> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
            .collect()
    };
    let (Some(sal), Some(h)) = (de_hex(p[2]), de_hex(p[3])) else {
        return false;
    };
    let calc = crate::backup::pbkdf2_sha256(pin.as_bytes(), &sal, iter);
    // Comparação em tempo constante.
    h.len() == 32 && calc.iter().zip(h.iter()).fold(0u8, |a, (x, y)| a | (x ^ y)) == 0
}

fn tem_pin(store: &Store) -> bool {
    store
        .get(CHAVE_PIN)
        .ok()
        .flatten()
        .is_some_and(|s| !s.is_empty())
}

// ---------------- Trava e confirmações (memória) ----------------

#[derive(Default)]
struct Portaria {
    erros: u32,
    travado_ate: Option<Instant>,
    /// código → (botão, quem, origem, validade)
    pendentes: HashMap<String, (i64, String, String, Instant)>,
}

fn portaria() -> &'static Mutex<Portaria> {
    static P: OnceLock<Mutex<Portaria>> = OnceLock::new();
    P.get_or_init(Mutex::default)
}

fn trava_restante(p: &Portaria, agora: Instant) -> Option<u64> {
    p.travado_ate
        .filter(|t| *t > agora)
        .map(|t| (t - agora).as_secs().max(1))
}

// ---------------- Quem e de onde ----------------

fn origem(addr: &SocketAddr, headers: &HeaderMap) -> String {
    let ip = addr.ip();
    let app = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|u| u.contains("CamFacilApp") || u.contains("; wv)"));
    let via = if app { "app" } else { "painel" };
    if ip.is_loopback() {
        format!("{via} neste aparelho")
    } else {
        format!("{via} {ip}")
    }
}

fn limpar_nome(s: &str) -> String {
    let n: String = s
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(40)
        .collect();
    if n.is_empty() {
        "sem nome".into()
    } else {
        n
    }
}

fn registrar(store: &Store, tipo: &str, detalhe: &str) {
    let _ = store.registrar_evento(0, None, tipo, detalhe, agora());
}

// ---------------- Disparo ----------------

async fn disparar(a: &Acao) -> Result<u16, String> {
    let http = cf_notify::tls::raizes_do_sistema(reqwest::Client::builder())
        .timeout(Duration::from_secs(8))
        .user_agent(concat!("CamFacilIA/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let montar = |auth: Option<&str>| {
        let mut r = match a.metodo.as_str() {
            "POST" => http.post(&a.url),
            "PUT" => http.put(&a.url),
            _ => http.get(&a.url),
        };
        match auth {
            Some(h) => r = r.header(reqwest::header::AUTHORIZATION, h),
            None if !a.usuario.is_empty() => r = r.basic_auth(&a.usuario, Some(&a.senha)),
            None => {}
        }
        if !a.corpo.is_empty() && a.metodo != "GET" {
            let tipo = if a.corpo.trim_start().starts_with(['{', '[']) {
                "application/json"
            } else {
                "application/x-www-form-urlencoded"
            };
            r = r
                .header(reqwest::header::CONTENT_TYPE, tipo)
                .body(a.corpo.clone());
        }
        r
    };
    let erro = |e: reqwest::Error| {
        if e.is_timeout() {
            "o equipamento não respondeu em 8 s".to_string()
        } else if e.is_connect() {
            "não consegui conectar no equipamento (IP, porta ou rede)".to_string()
        } else {
            "falha na chamada ao equipamento".to_string()
        }
    };
    let mut resp = montar(None).send().await.map_err(erro)?;
    // Relé/controladora com HTTP Digest (Intelbras, Hikvision…): responde ao
    // desafio e tenta de novo, uma vez.
    if resp.status().as_u16() == 401 && !a.usuario.is_empty() {
        if let Some(h) = digest_para(&resp, &a.metodo, &a.usuario, &a.senha) {
            resp = montar(Some(&h)).send().await.map_err(erro)?;
        }
    }
    let st = resp.status().as_u16();
    if resp.status().is_success() {
        Ok(st)
    } else if st == 401 || st == 403 {
        Err(format!(
            "o equipamento recusou o usuário/senha (HTTP {st}); confira, ou use autenticação na URL"
        ))
    } else {
        Err(format!("o equipamento respondeu HTTP {st}"))
    }
}

/// `Authorization: Digest …` para o desafio de uma resposta 401.
fn digest_para(
    resp: &reqwest::Response,
    metodo: &str,
    usuario: &str,
    senha: &str,
) -> Option<String> {
    let desafios: Vec<&str> = resp
        .headers()
        .get_all(reqwest::header::WWW_AUTHENTICATE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    let d = cf_core::digest::Desafio::escolher(desafios)?;
    let u = resp.url();
    let uri = match u.query() {
        Some(q) => format!("{}?{q}", u.path()),
        None => u.path().to_string(),
    };
    Some(d.autorizacao(
        metodo,
        &uri,
        usuario,
        senha,
        &cf_core::digest::novo_cnonce(),
        1,
    ))
}

// ---------------- API ----------------

fn cofre_ou_erro() -> Result<Cofre, (StatusCode, Json<Value>)> {
    Cofre::abrir().map_err(|e| {
        bad(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("cofre de segredos indisponível: {e}"),
        )
    })
}

/// `GET /api/botoes`: botões (sem segredos), se há PIN e se está travado.
pub async fn get_botoes(State(app): State<Shared>) -> ApiResult {
    let cofre = cofre_ou_erro()?;
    let lista: Vec<Value> = listar(&app.store)
        .into_iter()
        .map(|b| {
            let a = ler_acao(&app.store, &cofre, b.id);
            json!({
                "id": b.id,
                "nome": b.nome,
                "icone": b.icone,
                "metodo": a.as_ref().map(|a| a.metodo.clone()),
                "endereco": a.as_ref().map(|a| url_sem_segredos(&a.url)),
                "com_senha": a.as_ref().is_some_and(|a| !a.usuario.is_empty()),
            })
        })
        .collect();
    let travado = trava_restante(&portaria().lock().unwrap(), Instant::now());
    Ok(Json(json!({
        "botoes": lista,
        "tem_pin": tem_pin(&app.store),
        "travado_s": travado,
    })))
}

#[derive(Deserialize)]
pub struct NovoBotao {
    nome: String,
    #[serde(default)]
    icone: String,
    #[serde(flatten)]
    acao: Acao,
}

fn icone_valido(s: &str) -> String {
    match s {
        "rele" | "luz" => s.to_string(),
        _ => "portao".into(),
    }
}

/// `POST /api/botoes`: cadastra (técnico, com termo aceito).
pub async fn post_botao(State(app): State<Shared>, Json(n): Json<NovoBotao>) -> ApiResult {
    crate::termos::exigir_tecnico(&app)?;
    let nome: String = n.nome.trim().chars().take(30).collect();
    if nome.is_empty() {
        return Err(bad(StatusCode::BAD_REQUEST, "dê um nome para o botão"));
    }
    let acao = validar_acao(&n.acao).map_err(|e| bad(StatusCode::BAD_REQUEST, e))?;
    let cofre = cofre_ou_erro()?;
    let mut lista = listar(&app.store);
    if lista.len() >= MAX_BOTOES {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            format!("no máximo {MAX_BOTOES} botões"),
        ));
    }
    let id = lista.iter().map(|b| b.id).max().unwrap_or(0) + 1;
    let s = serde_json::to_string(&acao).map_err(internal)?;
    app.store
        .set(&chave_acao(id), &cofre.cifrar(&s))
        .map_err(internal)?;
    lista.push(Botao {
        id,
        nome: nome.clone(),
        icone: icone_valido(&n.icone),
        criado_em: agora(),
    });
    salvar_lista(&app.store, &lista).map_err(internal)?;
    registrar(
        &app.store,
        "botao_config",
        &format!("botão criado: {nome} ({})", url_sem_segredos(&acao.url)),
    );
    Ok(Json(json!({ "id": id })))
}

/// `DELETE /api/botoes/{id}`.
pub async fn delete_botao(State(app): State<Shared>, Path(id): Path<i64>) -> ApiResult {
    let mut lista = listar(&app.store);
    let Some(pos) = lista.iter().position(|b| b.id == id) else {
        return Err(bad(StatusCode::NOT_FOUND, "botão não encontrado"));
    };
    let b = lista.remove(pos);
    salvar_lista(&app.store, &lista).map_err(internal)?;
    app.store.set(&chave_acao(id), "").map_err(internal)?;
    registrar(
        &app.store,
        "botao_config",
        &format!("botão removido: {}", b.nome),
    );
    Ok(Json(json!({ "removido": true })))
}

#[derive(Deserialize)]
pub struct PinReq {
    #[serde(default)]
    atual: String,
    novo: String,
}

/// `POST /api/botoes/pin`: define ou troca o PIN (trocar exige o atual).
pub async fn post_pin(State(app): State<Shared>, Json(r): Json<PinReq>) -> ApiResult {
    crate::termos::exigir_tecnico(&app)?;
    if !pin_valido(&r.novo) {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            "o PIN deve ter de 4 a 8 números",
        ));
    }
    if let Some(g) = app
        .store
        .get(CHAVE_PIN)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
    {
        let atual = r.atual.clone();
        let ok = tokio::task::spawn_blocking(move || conferir_pin(&g, &atual))
            .await
            .unwrap_or(false);
        if !ok {
            return Err(bad(StatusCode::FORBIDDEN, "PIN atual errado"));
        }
    }
    let novo = r.novo.clone();
    let h = tokio::task::spawn_blocking(move || hash_pin(&novo, ITER_PIN))
        .await
        .map_err(internal)?;
    app.store.set(CHAVE_PIN, &h).map_err(internal)?;
    registrar(&app.store, "botao_config", "PIN dos botões definido");
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PrepararReq {
    pin: String,
    #[serde(default)]
    quem: String,
}

/// `POST /api/botoes/{id}/preparar`: confere o PIN e devolve um código de
/// confirmação de uso único (20 s). Nada é acionado aqui.
pub async fn post_preparar(
    State(app): State<Shared>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(r): Json<PrepararReq>,
) -> ApiResult {
    let botao = listar(&app.store)
        .into_iter()
        .find(|b| b.id == id)
        .ok_or_else(|| bad(StatusCode::NOT_FOUND, "botão não encontrado"))?;
    let quem = limpar_nome(&r.quem);
    let onde = origem(&addr, &headers);
    if let Some(s) = trava_restante(&portaria().lock().unwrap(), Instant::now()) {
        return Err(bad(
            StatusCode::TOO_MANY_REQUESTS,
            format!("muitos PINs errados; tente de novo em {s} s"),
        ));
    }
    let Some(g) = app
        .store
        .get(CHAVE_PIN)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
    else {
        return Err(bad(
            StatusCode::PRECONDITION_FAILED,
            "defina um PIN em Integrações → Portão e relé antes de usar os botões",
        ));
    };
    let pin = r.pin.clone();
    let ok = tokio::task::spawn_blocking(move || conferir_pin(&g, &pin))
        .await
        .unwrap_or(false);
    if !ok {
        let restam = {
            let mut p = portaria().lock().unwrap();
            p.erros += 1;
            if p.erros >= TENTATIVAS {
                p.erros = 0;
                p.travado_ate = Some(Instant::now() + TRAVA);
            }
            TENTATIVAS - p.erros
        };
        registrar(
            &app.store,
            "acionamento_negado",
            &format!("{} · {quem} · {onde} · PIN errado", botao.nome),
        );
        return Err(bad(
            StatusCode::FORBIDDEN,
            if restam == TENTATIVAS {
                "PIN errado. Botões travados por 5 minutos.".to_string()
            } else {
                format!("PIN errado ({restam} tentativas antes de travar)")
            },
        ));
    }
    let codigo = hex(&cofre::aleatorio(12));
    {
        let mut p = portaria().lock().unwrap();
        p.erros = 0;
        let agora = Instant::now();
        p.pendentes.retain(|_, v| v.3 > agora);
        p.pendentes.insert(
            codigo.clone(),
            (id, quem, onde, agora + VALIDADE_CONFIRMACAO),
        );
    }
    Ok(Json(json!({
        "confirmacao": codigo,
        "expira_s": VALIDADE_CONFIRMACAO.as_secs(),
        "nome": botao.nome,
    })))
}

#[derive(Deserialize)]
pub struct AcionarReq {
    confirmacao: String,
}

/// `POST /api/botoes/{id}/acionar`: com o código da confirmação, executa a
/// ação e grava o log (quem, de onde, quando, resultado).
pub async fn post_acionar(
    State(app): State<Shared>,
    Path(id): Path<i64>,
    Json(r): Json<AcionarReq>,
) -> ApiResult {
    let pend = {
        let mut p = portaria().lock().unwrap();
        p.pendentes.remove(r.confirmacao.trim())
    };
    let Some((bid, quem, onde, validade)) = pend else {
        return Err(bad(
            StatusCode::FORBIDDEN,
            "confirmação inválida ou já usada; digite o PIN de novo",
        ));
    };
    if bid != id || validade < Instant::now() {
        return Err(bad(
            StatusCode::FORBIDDEN,
            "a confirmação expirou; digite o PIN de novo",
        ));
    }
    let botao = listar(&app.store)
        .into_iter()
        .find(|b| b.id == id)
        .ok_or_else(|| bad(StatusCode::NOT_FOUND, "botão não encontrado"))?;
    let cofre = cofre_ou_erro()?;
    let acao = ler_acao(&app.store, &cofre, id).ok_or_else(|| {
        bad(
            StatusCode::CONFLICT,
            "a ação deste botão não pôde ser lida (cofre trocado?); cadastre de novo",
        )
    })?;
    let r = disparar(&acao).await;
    let resultado = match &r {
        Ok(st) => format!("ok (HTTP {st})"),
        Err(e) => format!("falhou: {e}"),
    };
    registrar(
        &app.store,
        "acionamento",
        &format!("{} · {quem} · {onde} · {resultado}", botao.nome),
    );
    match r {
        Ok(st) => Ok(Json(json!({ "ok": true, "status": st }))),
        Err(e) => Err(bad(StatusCode::BAD_GATEWAY, e)),
    }
}

/// `GET /api/botoes/log`: últimos acionamentos e recusas.
pub async fn get_log(State(app): State<Shared>) -> ApiResult {
    let mut v = app
        .store
        .eventos_tipo("acionamento", 50)
        .map_err(internal)?;
    v.extend(
        app.store
            .eventos_tipo("acionamento_negado", 50)
            .map_err(internal)?,
    );
    v.sort_by_key(|e| std::cmp::Reverse(e.quando));
    v.truncate(50);
    let l: Vec<Value> = v
        .into_iter()
        .map(|e| json!({ "quando": e.quando, "tipo": e.tipo, "detalhe": e.detalhe }))
        .collect();
    Ok(Json(json!({ "log": l })))
}

/// `GET /botoes`: tela própria para o celular (o app Android abre no WebView).
pub async fn pagina() -> Response {
    match crate::assets::find("/botoes.html") {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_hash_confere() {
        let h = hash_pin("1234", 1_000);
        assert!(h.starts_with("pbkdf2$1000$"));
        assert!(conferir_pin(&h, "1234"));
        assert!(!conferir_pin(&h, "1235"));
        assert!(!conferir_pin("lixo", "1234"));
        assert!(pin_valido("0000") && pin_valido("12345678"));
        assert!(!pin_valido("123") && !pin_valido("12a4") && !pin_valido("123456789"));
    }

    #[test]
    fn url_sem_segredos_esconde_usuario_e_parametros() {
        assert_eq!(
            url_sem_segredos("http://admin:123@10.0.0.9:8080/relay/0?turn=on&auth=x"),
            "http://10.0.0.9:8080/relay/0?…"
        );
        assert_eq!(
            url_sem_segredos("https://portao.local/abrir"),
            "https://portao.local/abrir"
        );
    }

    #[test]
    fn valida_acao() {
        let a = Acao {
            url: " http://10.0.0.9/relay/0 ".into(),
            metodo: "post".into(),
            ..Default::default()
        };
        let v = validar_acao(&a).unwrap();
        assert_eq!(v.url, "http://10.0.0.9/relay/0");
        assert_eq!(v.metodo, "POST");
        assert!(validar_acao(&Acao {
            url: "ftp://x/y".into(),
            metodo: "GET".into(),
            ..Default::default()
        })
        .is_err());
        assert!(validar_acao(&Acao {
            url: "http://10.0.0.9/x".into(),
            metodo: "DELETE".into(),
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn lista_e_acao_cifrada_no_banco() {
        let s = Store::memory().unwrap();
        let dir = std::env::temp_dir().join(format!("cf-botoes-{}", std::process::id()));
        let cofre = Cofre::abrir_em(&dir).unwrap();
        salvar_lista(
            &s,
            &[Botao {
                id: 1,
                nome: "Portão".into(),
                icone: "portao".into(),
                criado_em: 1,
            }],
        )
        .unwrap();
        let a = Acao {
            url: "http://10.0.0.9/relay?auth=segredo".into(),
            metodo: "GET".into(),
            usuario: "admin".into(),
            senha: "s3nh4".into(),
            corpo: String::new(),
        };
        s.set(
            &chave_acao(1),
            &cofre.cifrar(&serde_json::to_string(&a).unwrap()),
        )
        .unwrap();
        let cru = s.get(&chave_acao(1)).unwrap().unwrap();
        assert!(cru.starts_with("enc:"));
        assert!(!cru.contains("segredo") && !cru.contains("s3nh4"));
        assert_eq!(ler_acao(&s, &cofre, 1), Some(a));
        assert_eq!(listar(&s).len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn dispara_no_rele_simulado() {
        use std::io::{Read, Write};
        // O cliente HTTP precisa do provedor de criptografia; não depende de
        // outro teste ter instalado antes.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let servidor = std::thread::spawn(move || {
            let (mut s, _) = l.accept().unwrap();
            let mut buf = vec![0u8; 2048];
            let n = s.read(&mut buf).unwrap();
            let pedido = String::from_utf8_lossy(&buf[..n]).to_string();
            s.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
                .unwrap();
            pedido
        });
        let a = Acao {
            url: format!("http://127.0.0.1:{porta}/relay/0?turn=on"),
            metodo: "POST".into(),
            usuario: "admin".into(),
            senha: "x".into(),
            corpo: "{\"pulso\":1}".into(),
        };
        assert_eq!(disparar(&a).await, Ok(200));
        let pedido = servidor.join().unwrap();
        assert!(pedido.starts_with("POST /relay/0?turn=on"));
        assert!(pedido.to_lowercase().contains("authorization: basic"));
    }

    #[tokio::test]
    async fn dispara_em_rele_com_digest() {
        use std::io::{Read, Write};
        let _ = rustls::crypto::ring::default_provider().install_default();
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let porta = l.local_addr().unwrap().port();
        let servidor = std::thread::spawn(move || {
            let mut pedidos = Vec::new();
            for _ in 0..2 {
                let (mut s, _) = l.accept().unwrap();
                let mut buf = vec![0u8; 4096];
                let n = s.read(&mut buf).unwrap();
                let p = String::from_utf8_lossy(&buf[..n]).to_string();
                let r: &[u8] = if p.contains("Digest username=\"admin\"") {
                    b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok"
                } else {
                    b"HTTP/1.1 401 Unauthorized\r\nwww-authenticate: Digest realm=\"rele\", qop=\"auth\", nonce=\"n1\"\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                };
                s.write_all(r).unwrap();
                pedidos.push(p);
            }
            pedidos
        });
        let a = Acao {
            url: format!(
                "http://127.0.0.1:{porta}/cgi-bin/accessControl.cgi?action=openDoor&channel=1"
            ),
            metodo: "GET".into(),
            usuario: "admin".into(),
            senha: "x".into(),
            corpo: String::new(),
        };
        assert_eq!(disparar(&a).await, Ok(200));
        let p = servidor.join().unwrap();
        assert!(p[1].contains(r#"uri="/cgi-bin/accessControl.cgi?action=openDoor&channel=1""#));
        assert!(p[1].contains("qop=auth"));
    }
}
