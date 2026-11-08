//! Servidor MCP mínimo (SDD-04, ADR 0012): configurar o CamFácil IA
//! conversando com uma IA (Claude Desktop, Cursor, etc.).
//!
//! - **Desligado por padrão**; liga na dashboard (Integrações → MCP), com aviso.
//! - Transporte HTTP: `POST /mcp` (JSON-RPC 2.0, resposta JSON simples, sem
//!   SSE) no mesmo servidor da dashboard, com **token** obrigatório
//!   (`Authorization: Bearer …`), guardado cifrado pelo cofre.
//! - Transporte stdio: `camfacil --mcp` é uma ponte que lê JSON-RPC por linha
//!   no stdin e repassa ao programa já aberto nesta máquina (lê o token do
//!   banco local; nada sai da máquina).
//! - **Leitura por padrão**; criar/remover regras e ligar/desligar módulos só
//!   com "Permitir alterações" ligado. Botões de portão/relé **nunca** são
//!   acionados pelo MCP (só listados): ação física exige PIN e confirmação.
//! - JSON-RPC à mão (≈ 400 linhas) em vez do `rmcp`: o SDK puxa `schemars`,
//!   macros e transportes que não usamos; aqui só reaproveitamos `serde_json`
//!   e o `axum` que já estão no binário.

use crate::api::{bad, internal, ApiResult};
use crate::{App, Shared};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use cf_notify::cofre::{self, Cofre};
use serde::Deserialize;
use serde_json::{json, Value};

const CHAVE_LIGADO: &str = "mcp_ligado";
const CHAVE_ESCRITA: &str = "mcp_escrita";
const CHAVE_TOKEN: &str = "mcp_token";
/// Versões do protocolo que este servidor fala (a mais nova primeiro).
const VERSOES: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn ligado(app: &App) -> bool {
    app.store.get(CHAVE_LIGADO).ok().flatten().as_deref() == Some("1")
}

fn escrita(app: &App) -> bool {
    app.store.get(CHAVE_ESCRITA).ok().flatten().as_deref() == Some("1")
}

fn token(store: &cf_core::store::Store, cofre: &Cofre) -> Option<String> {
    let g = store.get(CHAVE_TOKEN).ok().flatten()?;
    cofre.decifrar(&g).filter(|t| !t.is_empty())
}

fn novo_token(store: &cf_core::store::Store, cofre: &Cofre) -> Result<String, String> {
    let t: String = cofre::aleatorio(24)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    store
        .set(CHAVE_TOKEN, &cofre.cifrar(&t))
        .map_err(|e| e.to_string())?;
    Ok(t)
}

fn iguais(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

// ---------------- Ferramentas ----------------

fn ferramentas() -> Value {
    let obj = |props: Value, req: &[&str]| json!({ "type": "object", "properties": props, "required": req });
    json!([
        {
            "name": "listar_cameras",
            "description": "Lista as câmeras cadastradas (id, nome, fonte, se está ao vivo e a saúde da imagem). Nunca mostra senha nem endereço RTSP.",
            "inputSchema": obj(json!({}), &[]),
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "ver_estado",
            "description": "Estado geral: versão, câmeras ao vivo, módulos de IA ligados e se a IA de nível médio está pronta.",
            "inputSchema": obj(json!({}), &[]),
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "listar_regras",
            "description": "Regras (zonas e linhas virtuais) de uma câmera. Pontos em coordenadas de 0 a 1 (x da esquerda para a direita, y de cima para baixo).",
            "inputSchema": obj(json!({ "camera_id": { "type": "integer" } }), &["camera_id"]),
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "criar_regra",
            "description": "Cria uma regra numa câmera. forma=zona (3 a 24 pontos, área) ou linha (2 pontos, contagem de passagens). Pontos de 0 a 1. Precisa de 'Permitir alterações' ligado na dashboard.",
            "inputSchema": obj(json!({
                "camera_id": { "type": "integer" },
                "nome": { "type": "string", "maxLength": 40 },
                "forma": { "type": "string", "enum": ["zona", "linha"] },
                "pontos": { "type": "array", "items": obj(json!({ "x": { "type": "number", "minimum": 0, "maximum": 1 }, "y": { "type": "number", "minimum": 0, "maximum": 1 } }), &["x", "y"]) },
                "rotulo_on": { "type": "string", "description": "Texto do balão quando a zona está ocupada (ex.: Ocupada)" },
                "rotulo_off": { "type": "string", "description": "Texto do balão quando livre (ex.: Livre)" }
            }), &["camera_id", "nome", "forma", "pontos"]),
            "annotations": { "readOnlyHint": false, "destructiveHint": false }
        },
        {
            "name": "remover_regra",
            "description": "Apaga uma regra pelo id. Precisa de 'Permitir alterações' ligado na dashboard.",
            "inputSchema": obj(json!({ "regra_id": { "type": "integer" } }), &["regra_id"]),
            "annotations": { "readOnlyHint": false, "destructiveHint": true }
        },
        {
            "name": "ligar_modulo",
            "description": "Liga ou desliga um módulo de IA: movimento, cerca, saude, pessoas, contagem, permanencia, calor. Precisa de 'Permitir alterações' ligado na dashboard.",
            "inputSchema": obj(json!({
                "modulo": { "type": "string", "enum": ["movimento", "cerca", "saude", "pessoas", "contagem", "permanencia", "calor"] },
                "ligado": { "type": "boolean" }
            }), &["modulo", "ligado"]),
            "annotations": { "readOnlyHint": false, "destructiveHint": false }
        },
        {
            "name": "ver_alertas",
            "description": "Últimos eventos e alertas (regras, saúde das câmeras, comandos), do mais novo para o mais velho.",
            "inputSchema": obj(json!({ "limite": { "type": "integer", "minimum": 1, "maximum": 100 } }), &[]),
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "listar_botoes",
            "description": "Lista os botões de portão/relé cadastrados (só nome e ícone). Acionar NÃO é possível por aqui: exige PIN e confirmação de uma pessoa na dashboard ou no app.",
            "inputSchema": obj(json!({}), &[]),
            "annotations": { "readOnlyHint": true }
        }
    ])
}

fn eh_escrita(nome: &str) -> bool {
    matches!(nome, "criar_regra" | "remover_regra" | "ligar_modulo")
}

/// Executa uma ferramenta; `Err` vira resultado com `isError: true`.
fn chamar(app: &App, nome: &str, args: &Value) -> Result<Value, String> {
    if eh_escrita(nome) && !escrita(app) {
        return Err(
            "alterações pelo MCP estão bloqueadas; o técnico precisa ligar 'Permitir alterações' em Integrações → MCP".into(),
        );
    }
    let num = |k: &str| {
        args.get(k)
            .and_then(Value::as_i64)
            .ok_or(format!("informe {k} (número)"))
    };
    let r = match nome {
        "listar_cameras" => {
            let cams = app.store.cameras().map_err(|e| e.to_string())?;
            let v: Vec<Value> = cams
                .into_iter()
                .map(|c| {
                    let e = app
                        .video
                        .canal(c.id)
                        .map(|k| k.estado())
                        .unwrap_or_default();
                    json!({ "id": c.id, "nome": c.nome, "fonte": c.fonte, "ativa": c.ativa,
                            "ao_vivo": e.online, "fps": e.fps, "saude": e.saude })
                })
                .collect();
            json!({ "cameras": v })
        }
        "ver_estado" => {
            let cams = app.store.cameras().map_err(|e| e.to_string())?;
            let ao_vivo = cams
                .iter()
                .filter(|c| app.video.canal(c.id).is_some_and(|k| k.estado().online))
                .count();
            let m = crate::video::modulos(&app.store);
            let modulos: serde_json::Map<String, Value> = crate::video::MODULOS
                .iter()
                .map(|(id, _, _, _)| (id.to_string(), json!(m.ligado(id))))
                .collect();
            json!({
                "versao": cf_core::VERSION,
                "cameras": cams.len(),
                "ao_vivo": ao_vivo,
                "modulos": modulos,
                "ia_media_pronta": app.ia.pronto(),
                "alteracoes_permitidas": escrita(app),
            })
        }
        "listar_regras" => {
            let cam = num("camera_id")?;
            json!({ "regras": app.store.regras(cam).map_err(|e| e.to_string())? })
        }
        "criar_regra" => {
            let cam = num("camera_id")?;
            let r: crate::api::NovaRegra = serde_json::from_value(args.clone())
                .map_err(|e| format!("argumentos inválidos: {e}"))?;
            let id = crate::api::criar_regra(app, cam, &r).map_err(|(_, m)| m)?;
            registrar(
                app,
                &format!("criar_regra {} (id {id}) na câmera {cam}", r.nome),
            );
            json!({ "id": id })
        }
        "remover_regra" => {
            let id = num("regra_id")?;
            let cam = app
                .store
                .cameras()
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|c| {
                    app.store
                        .regras(c.id)
                        .unwrap_or_default()
                        .iter()
                        .any(|r| r.id == id)
                })
                .map(|c| c.id)
                .ok_or("regra não encontrada")?;
            app.store.remover_regra(id).map_err(|e| e.to_string())?;
            app.video.recarregar_regras(cam);
            registrar(app, &format!("remover_regra {id}"));
            json!({ "removida": true })
        }
        "ligar_modulo" => {
            let m = args
                .get("modulo")
                .and_then(Value::as_str)
                .ok_or("informe modulo")?;
            let l = args
                .get("ligado")
                .and_then(Value::as_bool)
                .ok_or("informe ligado (true/false)")?;
            let (id, bonito) = crate::video::definir_modulo(&app.store, m, l)?;
            registrar(app, &format!("ligar_modulo {id}={l}"));
            json!({ "modulo": id, "nome": bonito, "ligado": l })
        }
        "ver_alertas" => {
            let lim = args
                .get("limite")
                .and_then(Value::as_i64)
                .unwrap_or(20)
                .clamp(1, 100);
            json!({ "eventos": app.store.eventos(lim).map_err(|e| e.to_string())? })
        }
        "listar_botoes" => {
            let v: Vec<Value> = crate::botoes::listar(&app.store)
                .into_iter()
                .map(|b| json!({ "id": b.id, "nome": b.nome, "icone": b.icone }))
                .collect();
            json!({ "botoes": v, "aviso": "acionar exige PIN e confirmação na dashboard ou no app" })
        }
        _ => return Err(format!("ferramenta desconhecida: {nome}")),
    };
    Ok(r)
}

fn registrar(app: &App, detalhe: &str) {
    let _ = app.store.registrar_evento(0, None, "mcp", detalhe, agora());
}

// ---------------- JSON-RPC ----------------

fn erro_rpc(id: Value, codigo: i64, msg: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": codigo, "message": msg } })
}

/// Atende uma mensagem JSON-RPC. `None` = notificação (sem resposta).
pub fn atender(app: &App, msg: &Value) -> Option<Value> {
    if msg.is_array() {
        return Some(erro_rpc(Value::Null, -32600, "lotes não são aceitos"));
    }
    let metodo = msg.get("method").and_then(Value::as_str);
    let id = msg.get("id").cloned();
    let (Some(metodo), Some(id)) = (metodo, id) else {
        // Notificação (ex.: notifications/initialized) ou resposta: nada a dizer.
        return if msg.get("method").is_some() || msg.get("result").is_some() {
            None
        } else {
            Some(erro_rpc(Value::Null, -32600, "requisição inválida"))
        };
    };
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let resultado = match metodo {
        "initialize" => {
            let pedida = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(VERSOES[0]);
            let versao = VERSOES
                .iter()
                .find(|v| **v == pedida)
                .copied()
                .unwrap_or(VERSOES[0]);
            json!({
                "protocolVersion": versao,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "camfacil-ia", "title": "CamFácil IA", "version": cf_core::VERSION },
                "instructions": "Programa local de IA de vídeo para CFTV. Use listar_cameras e listar_regras antes de criar regras; pontos vão de 0 a 1. Alterações dependem de permissão do técnico. Portão e relé não são acionáveis por aqui."
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": ferramentas() }),
        "tools/call" => {
            let nome = params.get("name").and_then(Value::as_str).unwrap_or("");
            if !ferramentas()
                .as_array()
                .is_some_and(|l| l.iter().any(|f| f["name"] == nome))
            {
                return Some(erro_rpc(
                    id,
                    -32602,
                    &format!("ferramenta desconhecida: {nome}"),
                ));
            }
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match chamar(app, nome, &args) {
                Ok(v) => json!({
                    "content": [{ "type": "text", "text": v.to_string() }],
                    "structuredContent": v,
                    "isError": false
                }),
                Err(e) => json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
            }
        }
        _ => return Some(erro_rpc(id, -32601, "método não suportado")),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": resultado }))
}

/// Origem do navegador só de localhost (contra DNS rebinding); clientes MCP
/// normalmente não mandam `Origin`.
fn origem_ok(h: &HeaderMap) -> bool {
    match h.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        None => true,
        Some(o) => {
            let host = o
                .split("://")
                .nth(1)
                .unwrap_or("")
                .split(':')
                .next()
                .unwrap_or("");
            matches!(host, "localhost" | "127.0.0.1" | "[::1]")
        }
    }
}

/// `POST /mcp`.
pub async fn post_mcp(State(app): State<Shared>, headers: HeaderMap, corpo: String) -> Response {
    if !ligado(&app) {
        return bad(StatusCode::NOT_FOUND, "MCP desligado").into_response();
    }
    if !origem_ok(&headers) {
        return bad(StatusCode::FORBIDDEN, "origem não permitida").into_response();
    }
    let Ok(cofre) = Cofre::abrir() else {
        return bad(StatusCode::SERVICE_UNAVAILABLE, "cofre indisponível").into_response();
    };
    let dado = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
        .trim()
        .to_string();
    match token(&app.store, &cofre) {
        Some(t) if iguais(&t, &dado) => {}
        _ => return bad(StatusCode::UNAUTHORIZED, "token inválido").into_response(),
    }
    let msg: Value = match serde_json::from_str(&corpo) {
        Ok(v) => v,
        Err(_) => {
            return Json(erro_rpc(Value::Null, -32700, "JSON inválido")).into_response();
        }
    };
    let app2 = app.clone();
    // Ferramentas leem banco e estado: rápido, mas fora do laço do runtime.
    let r = tokio::task::spawn_blocking(move || atender(&app2, &msg))
        .await
        .unwrap_or_else(|_| Some(erro_rpc(Value::Null, -32603, "erro interno")));
    match r {
        Some(v) => Json(v).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}

/// `GET /mcp`: sem fluxo SSE neste servidor.
pub async fn get_mcp() -> Response {
    (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "POST")]).into_response()
}

// ---------------- Dashboard ----------------

fn comando() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "camfacil".into())
}

/// `GET /api/mcp`.
pub async fn get_config(State(app): State<Shared>) -> ApiResult {
    let lig = ligado(&app);
    let tok = if lig {
        Cofre::abrir().ok().and_then(|c| token(&app.store, &c))
    } else {
        None
    };
    let porta = app
        .store
        .get("port")
        .ok()
        .flatten()
        .unwrap_or_else(|| "7070".into());
    let url = format!("http://localhost:{porta}/mcp");
    let exemplo =
        json!({ "mcpServers": { "camfacil": { "command": comando(), "args": ["--mcp"] } } });
    Ok(Json(json!({
        "ligado": lig,
        "escrita": escrita(&app),
        "url": url,
        "token": tok,
        "exemplo": serde_json::to_string_pretty(&exemplo).unwrap_or_default(),
    })))
}

#[derive(Deserialize)]
pub struct ConfigReq {
    ligado: Option<bool>,
    escrita: Option<bool>,
}

/// `POST /api/mcp`: liga/desliga e permite alterações.
pub async fn post_config(State(app): State<Shared>, Json(c): Json<ConfigReq>) -> ApiResult {
    if c.ligado == Some(true) || c.escrita == Some(true) {
        crate::termos::exigir_tecnico(&app)?;
    }
    if let Some(l) = c.ligado {
        if l {
            let cofre = Cofre::abrir().map_err(internal)?;
            if token(&app.store, &cofre).is_none() {
                novo_token(&app.store, &cofre).map_err(internal)?;
            }
        }
        app.store
            .set(CHAVE_LIGADO, if l { "1" } else { "0" })
            .map_err(internal)?;
        if !l {
            // Desligar também tira a permissão de escrita.
            app.store.set(CHAVE_ESCRITA, "0").map_err(internal)?;
        }
        registrar(&app, if l { "MCP ligado" } else { "MCP desligado" });
    }
    if let Some(e) = c.escrita {
        if e && !ligado(&app) {
            return Err(bad(StatusCode::BAD_REQUEST, "ligue o MCP primeiro"));
        }
        app.store
            .set(CHAVE_ESCRITA, if e { "1" } else { "0" })
            .map_err(internal)?;
        registrar(
            &app,
            if e {
                "MCP: alterações permitidas"
            } else {
                "MCP: só leitura"
            },
        );
    }
    get_config(State(app)).await
}

/// `POST /api/mcp/token`: gera outro token (o antigo para de valer).
pub async fn post_token(State(app): State<Shared>) -> ApiResult {
    let cofre = Cofre::abrir().map_err(internal)?;
    novo_token(&app.store, &cofre).map_err(internal)?;
    registrar(&app, "MCP: token trocado");
    get_config(State(app)).await
}

// ---------------- Ponte stdio ----------------

/// `camfacil --mcp`: lê JSON-RPC por linha do stdin e repassa ao programa
/// aberto nesta máquina. Só escreve JSON-RPC no stdout; avisos vão ao stderr.
pub async fn ponte_stdio() -> Result<(), Box<dyn std::error::Error>> {
    let dir = cf_core::paths::data_dir();
    let store = cf_core::store::Store::open(&dir.join("camfacil.db"))?;
    let porta = store.get("port")?.unwrap_or_else(|| "7070".into());
    let tok = Cofre::abrir_em(&dir)
        .ok()
        .and_then(|c| token(&store, &c))
        .unwrap_or_default();
    drop(store);
    let url = format!("http://127.0.0.1:{porta}/mcp");
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;
    eprintln!("CamFácil IA MCP: ponte stdio para {url}");
    loop {
        let linha = tokio::task::spawn_blocking(|| {
            let mut s = String::new();
            std::io::stdin().read_line(&mut s).map(|n| (n, s))
        })
        .await??;
        if linha.0 == 0 {
            return Ok(());
        }
        let texto = linha.1.trim();
        if texto.is_empty() {
            continue;
        }
        let id = serde_json::from_str::<Value>(texto)
            .ok()
            .and_then(|v| v.get("id").cloned());
        let resp = http
            .post(&url)
            .header(header::CONTENT_TYPE.as_str(), "application/json")
            .header(header::AUTHORIZATION.as_str(), format!("Bearer {tok}"))
            .body(texto.to_string())
            .send()
            .await;
        let saida = match resp {
            Ok(r) if r.status() == reqwest::StatusCode::ACCEPTED => None,
            Ok(r) if r.status().is_success() => Some(r.text().await.unwrap_or_default()),
            Ok(r) => {
                let msg = match r.status().as_u16() {
                    404 => "MCP desligado: ligue em Integrações → MCP na dashboard",
                    401 => "token inválido: abra a dashboard e confira Integrações → MCP",
                    _ => "o CamFácil IA recusou o pedido",
                };
                id.map(|id| erro_rpc(id, -32000, msg).to_string())
            }
            Err(_) => id.map(|id| {
                erro_rpc(
                    id,
                    -32000,
                    "o CamFácil IA não está aberto nesta máquina; abra o programa e tente de novo",
                )
                .to_string()
            }),
        };
        if let Some(s) = saida {
            use std::io::Write;
            let mut out = std::io::stdout().lock();
            writeln!(out, "{}", s.replace('\n', ""))?;
            out.flush()?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origem_so_localhost() {
        let mut h = HeaderMap::new();
        assert!(origem_ok(&h));
        h.insert(header::ORIGIN, "http://localhost:7070".parse().unwrap());
        assert!(origem_ok(&h));
        h.insert(header::ORIGIN, "http://evil.example".parse().unwrap());
        assert!(!origem_ok(&h));
    }

    #[test]
    fn ferramentas_tem_nome_e_esquema() {
        let f = ferramentas();
        let l = f.as_array().unwrap();
        assert!(l.len() >= 8);
        for t in l {
            assert!(t["name"].is_string());
            assert_eq!(t["inputSchema"]["type"], "object");
        }
        assert!(!l
            .iter()
            .any(|t| t["name"].as_str().unwrap().contains("acionar")));
        assert!(eh_escrita("criar_regra") && !eh_escrita("ver_alertas"));
    }

    #[test]
    fn compara_token_tempo_constante() {
        assert!(iguais("abc", "abc"));
        assert!(!iguais("abc", "abd"));
        assert!(!iguais("abc", "ab"));
    }
}
