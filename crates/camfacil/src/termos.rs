//! Termos e LGPD dentro do programa (PRD §5.10, `docs/legal/`).
//!
//! - Termo de responsabilidade do técnico: texto embutido, aceite gravado na
//!   tabela `acceptances` com versão e data. **Sem ele nenhuma notificação
//!   (Telegram, WhatsApp, webhook) pode ser ligada** (`exigir_tecnico`).
//! - Autorização do cliente: dados do cliente gravados em `settings` e um
//!   `.md` preenchido para imprimir e assinar (sem crate de PDF).
//! - Placa "ambiente monitorado" preenchida para imprimir.
//! - "Apagar dados deste cliente": eventos, índice do arquivo e fotos, com
//!   evento `lgpd` no log.

use crate::api::{bad, internal, ApiResult};
use crate::Shared;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const TERMO_TECNICO: &str =
    include_str!("../../../docs/legal/TERMO-RESPONSABILIDADE-TECNICO.md");
pub const TERMO_CLIENTE: &str = include_str!("../../../docs/legal/TERMO-AUTORIZACAO-CLIENTE.md");
pub const PLACA: &str = include_str!("../../../docs/legal/PLACA-AMBIENTE-MONITORADO.md");

/// Versões dos textos (acompanham a linha "Versão do texto" de cada arquivo).
pub const VERSAO_TECNICO: &str = "1.0";
pub const VERSAO_CLIENTE: &str = "1.0";
pub(crate) const DOC_TECNICO: &str = "termo_tecnico";
const DOC_CLIENTE: &str = "autorizacao_cliente";
const CHAVE_CLIENTE: &str = "termos_cliente";

type Falha = (StatusCode, Json<Value>);

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Barreira: só deixa ligar uma notificação depois do aceite do termo do técnico.
pub fn exigir_tecnico(app: &Shared) -> Result<(), Falha> {
    let ok = app
        .store
        .accepted(DOC_TECNICO, VERSAO_TECNICO)
        .map_err(internal)?;
    if ok {
        Ok(())
    } else {
        Err(bad(
            StatusCode::PRECONDITION_FAILED,
            "Antes de ligar notificações, aceite o Termo de Responsabilidade do Técnico em Integrações → Termos e LGPD.",
        ))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Cliente {
    #[serde(default)]
    pub cliente: String,
    #[serde(default)]
    pub documento: String,
    #[serde(default)]
    pub tecnico: String,
    #[serde(default)]
    pub contato: String,
    #[serde(default)]
    pub local: String,
    #[serde(default)]
    pub prazo_dias: u32,
}

fn cliente_salvo(app: &Shared) -> Option<Cliente> {
    let v = app.store.get(CHAVE_CLIENTE).ok().flatten()?;
    serde_json::from_str(&v).ok()
}

/// Data local `DD/MM/AAAA` sem crate de datas (algoritmo civil dos dias julianos).
pub fn data_local(unix: i64, offset_s: i64) -> (u32, u32, i64) {
    let dias = (unix + offset_s).div_euclid(86_400);
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (d, m, if m <= 2 { y + 1 } else { y })
}

fn data_hoje(app: &Shared) -> (u32, u32, i64) {
    data_local(agora(), cf_notify::servico::offset_local_s(&app.store))
}

/// Substitui, em ordem, cada trecho de 3+ sublinhados pelo valor seguinte da
/// lista; valor vazio mantém a linha para preencher à mão.
pub fn preencher(modelo: &str, valores: &[&str]) -> String {
    let mut saida = String::with_capacity(modelo.len() + 128);
    let mut i = 0;
    let bytes = modelo.as_bytes();
    let mut p = 0;
    while p < bytes.len() {
        if bytes[p] == b'_' {
            let ini = p;
            while p < bytes.len() && bytes[p] == b'_' {
                p += 1;
            }
            if p - ini >= 3 {
                match valores.get(i).filter(|v| !v.is_empty()) {
                    Some(v) => saida.push_str(v),
                    None => saida.push_str(&modelo[ini..p]),
                }
                i += 1;
            } else {
                saida.push_str(&modelo[ini..p]);
            }
        } else {
            let ini = p;
            while p < bytes.len() && bytes[p] != b'_' {
                p += 1;
            }
            saida.push_str(&modelo[ini..p]);
        }
    }
    saida
}

/// Canais que estão configurados agora (para marcar `[x]` no termo).
fn canais(app: &Shared) -> Vec<&'static str> {
    let mut v = Vec::new();
    if app.telegram.get().map(|t| t.situacao().configurado) == Some(true) {
        v.push("Telegram");
    }
    if app.whatsapp.get().map(|w| w.situacao().configurado) == Some(true) {
        v.push("WhatsApp");
    }
    if app.webhook.get().map(|w| w.situacao().configurado) == Some(true) {
        v.push("webhook");
    }
    v
}

/// Termo do cliente preenchido com o que o técnico registrou.
pub fn termo_cliente_preenchido(app: &Shared, c: &Cliente) -> String {
    let (d, m, a) = data_hoje(app);
    let cams = app
        .store
        .cameras()
        .unwrap_or_default()
        .iter()
        .map(|c| c.nome.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let prazo = if c.prazo_dias > 0 {
        c.prazo_dias.to_string()
    } else {
        String::new()
    };
    let local_data = if c.local.is_empty() {
        format!("{d:02}/{m:02}/{a}")
    } else {
        format!("{}, {d:02}/{m:02}/{a}", c.local)
    };
    let dia = format!("{d:02}");
    let mes = format!("{m:02}");
    let ano = a.to_string();
    let mut texto = preencher(
        TERMO_CLIENTE,
        &[
            &c.cliente,
            &c.documento,
            &c.tecnico,
            &c.contato,
            &cams,
            &dia,
            &mes,
            &ano,
            "",
            "",
            "",
            "",
            &prazo,
            &c.contato,
            &local_data,
            "",
            "",
        ],
    );
    let ligados = canais(app);
    for (rotulo, marca) in [
        ("[ ] Telegram", "[x] Telegram"),
        ("[ ] WhatsApp", "[x] WhatsApp"),
    ] {
        if ligados.iter().any(|c| *c == &marca[4..]) {
            texto = texto.replace(rotulo, marca);
        }
    }
    texto.push_str(&format!(
        "\n\n---\nGerado pelo CamFácil IA {} em {d:02}/{m:02}/{a} (modelo v{VERSAO_CLIENTE}).\n",
        cf_core::VERSION
    ));
    texto
}

pub fn placa_preenchida(c: Option<&Cliente>) -> String {
    let (resp, cont) = match c {
        Some(c) if !c.cliente.is_empty() => (c.cliente.as_str(), c.contato.as_str()),
        _ => ("", ""),
    };
    let mut p = preencher(PLACA, &[resp, cont]);
    // A linha sobre PDF é promessa da fase 5; não entra no que se imprime.
    p = p
        .lines()
        .filter(|l| !l.starts_with("> A dashboard gerará"))
        .collect::<Vec<_>>()
        .join("\n");
    p.trim_end().to_string() + "\n"
}

fn termo_tecnico_com_recurso(app: &Shared) -> String {
    let c = canais(app);
    let recurso = if c.is_empty() {
        "alertas por Telegram, WhatsApp ou webhook".to_string()
    } else {
        format!("alertas por {}", c.join(", "))
    };
    TERMO_TECNICO.replace(
        "[recurso: arquivo de alertas no Telegram / WhatsApp / modo estúdio / envio de imagens a provedor de IA]",
        &recurso,
    )
}

// ---------- Rotas ----------

pub async fn get_termos(State(app): State<Shared>) -> ApiResult {
    let tecnico = app.store.ultimo_aceite(DOC_TECNICO).map_err(internal)?;
    let cliente_aceite = app.store.ultimo_aceite(DOC_CLIENTE).map_err(internal)?;
    let cliente = cliente_salvo(&app);
    let tec_ok = tecnico.as_ref().map(|(v, _)| v == VERSAO_TECNICO) == Some(true);
    Ok(Json(json!({
        "tecnico": {
            "aceito": tec_ok,
            "versao_atual": VERSAO_TECNICO,
            "versao_aceita": tecnico.as_ref().map(|(v, _)| v.clone()),
            "quando": tecnico.as_ref().map(|(_, q)| *q),
            "texto": termo_tecnico_com_recurso(&app),
        },
        "cliente": {
            "registrado": cliente.is_some(),
            "versao_atual": VERSAO_CLIENTE,
            "quando": cliente_aceite.as_ref().map(|(_, q)| *q),
            "dados": cliente,
            "canais": canais(&app),
        },
        "placa": placa_preenchida(cliente_salvo(&app).as_ref()),
        "notificacoes_liberadas": tec_ok,
    })))
}

pub async fn post_termos_tecnico(State(app): State<Shared>) -> ApiResult {
    let t = agora();
    app.store
        .accept(DOC_TECNICO, VERSAO_TECNICO, t)
        .map_err(internal)?;
    let _ = app.store.registrar_evento(
        0,
        None,
        "lgpd",
        &format!("termo do técnico v{VERSAO_TECNICO} aceito"),
        t,
    );
    Ok(Json(
        json!({ "aceito": true, "versao": VERSAO_TECNICO, "quando": t }),
    ))
}

pub async fn post_termos_cliente(State(app): State<Shared>, Json(c): Json<Cliente>) -> ApiResult {
    let c = Cliente {
        cliente: c.cliente.trim().to_string(),
        documento: c.documento.trim().to_string(),
        tecnico: c.tecnico.trim().to_string(),
        contato: c.contato.trim().to_string(),
        local: c.local.trim().to_string(),
        prazo_dias: c.prazo_dias.min(3650),
    };
    if c.cliente.is_empty() || c.tecnico.is_empty() {
        return Err(bad(
            StatusCode::UNPROCESSABLE_ENTITY,
            "informe pelo menos o nome do cliente e o do técnico responsável",
        ));
    }
    let t = agora();
    app.store
        .set(
            CHAVE_CLIENTE,
            &serde_json::to_string(&c).unwrap_or_default(),
        )
        .map_err(internal)?;
    app.store
        .accept(DOC_CLIENTE, VERSAO_CLIENTE, t)
        .map_err(internal)?;
    let _ = app.store.registrar_evento(
        0,
        None,
        "lgpd",
        &format!("autorização do cliente registrada: {}", c.cliente),
        t,
    );
    Ok(Json(json!({ "registrado": true, "quando": t })))
}

fn arquivo_md(nome: &str, corpo: String) -> Response {
    (
        [
            (
                header::CONTENT_TYPE,
                "text/markdown; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{nome}\""),
            ),
        ],
        corpo,
    )
        .into_response()
}

/// Termo do cliente preenchido, para imprimir e assinar.
pub async fn get_termos_cliente_md(State(app): State<Shared>) -> Response {
    let c = cliente_salvo(&app).unwrap_or_default();
    arquivo_md(
        "autorizacao-cliente-camfacil.md",
        termo_cliente_preenchido(&app, &c),
    )
}

pub async fn get_termos_placa_md(State(app): State<Shared>) -> Response {
    arquivo_md(
        "placa-ambiente-monitorado.md",
        placa_preenchida(cliente_salvo(&app).as_ref()),
    )
}

#[derive(Deserialize)]
pub struct ApagarReq {
    /// O técnico precisa digitar exatamente `APAGAR` (segunda confirmação).
    confirmacao: String,
}

/// Apaga eventos, índice do arquivo e fotos guardadas. Configurações, câmeras
/// e regras ficam. Registra o evento `lgpd` depois da limpeza.
pub async fn post_termos_apagar(State(app): State<Shared>, Json(r): Json<ApagarReq>) -> ApiResult {
    if r.confirmacao.trim() != "APAGAR" {
        return Err(bad(
            StatusCode::UNPROCESSABLE_ENTITY,
            "digite APAGAR para confirmar",
        ));
    }
    let (eventos, arquivados) = app.store.apagar_dados_cliente().map_err(internal)?;
    let fotos = apagar_fotos();
    let t = agora();
    let _ = app.store.registrar_evento(
        0,
        None,
        "lgpd",
        &format!("dados do cliente apagados: {eventos} eventos, {arquivados} alertas arquivados, {fotos} fotos"),
        t,
    );
    Ok(Json(json!({
        "eventos": eventos,
        "arquivados": arquivados,
        "fotos": fotos,
    })))
}

/// Pasta das fotos dos alertas (`data_dir/fotos/cam_<id>/…`), compartilhada
/// com o arquivo de alertas.
pub const PASTA_FOTOS: &str = "fotos";

/// Apaga todas as fotos guardadas; devolve quantos arquivos saíram.
pub fn apagar_fotos() -> usize {
    let raiz = cf_core::paths::data_dir().join(PASTA_FOTOS);
    let mut n = 0;
    if let Ok(cams) = std::fs::read_dir(&raiz) {
        for cam in cams.flatten() {
            if let Ok(fotos) = std::fs::read_dir(cam.path()) {
                n += fotos.flatten().filter(|f| f.path().is_file()).count();
            }
        }
    }
    let _ = std::fs::remove_dir_all(&raiz);
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preenche_sublinhados_em_ordem() {
        let m = "A: ______ B: ___ c_d ____.";
        assert_eq!(preencher(m, &["x", "", "z"]), "A: x B: ___ c_d z.");
        assert_eq!(preencher(m, &["x"]), "A: x B: ___ c_d ____.");
    }

    #[test]
    fn termo_do_cliente_tem_dezessete_lacunas() {
        let n = TERMO_CLIENTE
            .split(|c| c != '_')
            .filter(|s| s.len() >= 3)
            .count();
        // cliente, CNPJ, técnico, contato, câmeras, 2 datas (3 cada), provedor,
        // prazo, contato art. 18, local e data, 2 assinaturas.
        assert_eq!(n, 17);
        assert!(PLACA.split(|c| c != '_').filter(|s| s.len() >= 3).count() == 2);
    }

    #[test]
    fn data_civil() {
        assert_eq!(data_local(0, 0), (1, 1, 1970));
        assert_eq!(data_local(1_794_009_600, -3 * 3600), (6, 11, 2026)); // 2026-11-07 00:00 UTC
        assert_eq!(data_local(951_782_400, 0), (29, 2, 2000));
    }
}
