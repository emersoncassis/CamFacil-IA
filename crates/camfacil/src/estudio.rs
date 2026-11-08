//! Modo estúdio no programa local (SDD-07): importar um **modelo de estudo**
//! treinado no Google Colab (ONNX + receita + manifesto).
//!
//! O modelo fica em `data_dir/estudio/<id>/`, marcado "somente estudo" e
//! **desligado**. O detector atual só entende as classes do COCO
//! (`cf_core::deteccao::CLASSES`); rodar classes próprias vem na próxima etapa,
//! então aqui o modelo é guardado, conferido e listado, sem executar.
//!
//! Importação em dois passos para não carregar o ONNX inteiro num JSON:
//! 1. `POST /api/estudio/modelos` com `{receita, manifesto?}`;
//! 2. `PUT /api/estudio/modelos/{id}/onnx` com o arquivo (corpo cru).

use crate::api::{bad, internal, ApiResult};
use crate::Shared;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use cf_core::receita::{self, Metricas, Receita, AVISO_ESTUDO};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path as Caminho, PathBuf};

/// Maior ONNX aceito (um YOLO nano tem ~10 MB).
pub const ONNX_MAX: usize = 64 * 1024 * 1024;

/// Por que o modelo de estudo ainda não roda.
pub const MOTIVO_NAO_EXECUTA: &str = "O detector atual só reconhece as classes padrão (pessoa, carro, moto…). Rodar as classes do seu modelo de estudo vem numa próxima versão; por enquanto ele fica guardado e conferido.";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Situacao {
    AguardandoOnnx,
    Pronto,
}

/// O que fica gravado em `info.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    pub id: String,
    pub receita_id: String,
    pub nome: String,
    pub classes: Vec<String>,
    pub situacao: Situacao,
    pub ligado: bool,
    pub somente_estudo: bool,
    pub importado_em: i64,
    pub receita_sha256: String,
    pub onnx_sha256: Option<String>,
    pub onnx_bytes: Option<u64>,
    /// SHA-256 esperado (do manifesto do Colab), se veio.
    pub esperado_sha256: Option<String>,
    pub metricas: Option<Metricas>,
}

/// Parte do manifesto que o notebook grava e que conferimos.
#[derive(Debug, Deserialize)]
struct Manifesto {
    formato: String,
    #[serde(default)]
    somente_estudo: bool,
    receita_id: String,
    onnx_sha256: String,
    classes: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum Falha {
    Invalido(Vec<String>),
    NaoEncontrado,
    Interno(String),
}

impl Falha {
    fn http(self) -> (StatusCode, Json<Value>) {
        match self {
            Falha::Invalido(e) => (
                StatusCode::BAD_REQUEST,
                Json(json!({ "erro": e.join("; "), "erros": e })),
            ),
            Falha::NaoEncontrado => bad(StatusCode::NOT_FOUND, "modelo de estudo não encontrado"),
            Falha::Interno(e) => internal(e),
        }
    }
}

fn io(e: std::io::Error) -> Falha {
    Falha::Interno(e.to_string())
}

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn id_valido(id: &str) -> bool {
    (1..=80).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub struct Estudio {
    dir: PathBuf,
}

impl Estudio {
    pub fn new(data_dir: &Caminho) -> Self {
        Estudio {
            dir: data_dir.join("estudio"),
        }
    }

    fn pasta(&self, id: &str) -> Result<PathBuf, Falha> {
        if !id_valido(id) {
            return Err(Falha::NaoEncontrado);
        }
        let p = self.dir.join(id);
        if p.join("info.json").is_file() {
            Ok(p)
        } else {
            Err(Falha::NaoEncontrado)
        }
    }

    fn ler_info(&self, pasta: &Caminho) -> Result<Info, Falha> {
        let t = std::fs::read_to_string(pasta.join("info.json")).map_err(io)?;
        serde_json::from_str(&t).map_err(|e| Falha::Interno(e.to_string()))
    }

    fn gravar(caminho: &Caminho, dados: &[u8]) -> Result<(), Falha> {
        let tmp = caminho.with_extension("tmp");
        std::fs::write(&tmp, dados).map_err(io)?;
        std::fs::rename(&tmp, caminho).map_err(io)
    }

    fn gravar_info(&self, pasta: &Caminho, info: &Info) -> Result<(), Falha> {
        let t = serde_json::to_vec_pretty(info).map_err(|e| Falha::Interno(e.to_string()))?;
        Self::gravar(&pasta.join("info.json"), &t)
    }

    /// Passo 1: valida a receita (e o manifesto, se veio) e reserva a pasta.
    pub fn importar(&self, receita: Value, manifesto: Option<Value>) -> Result<Info, Falha> {
        let r: Receita = receita::de_valor(receita).map_err(Falha::Invalido)?;
        let esperado = match manifesto {
            None | Some(Value::Null) => None,
            Some(m) => Some(conferir_manifesto(&r, m)?),
        };
        std::fs::create_dir_all(&self.dir).map_err(io)?;
        let mut sufixo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() ^ d.as_secs() as u32)
            .unwrap_or(0);
        let pasta = loop {
            let id = format!("{}-{:08x}", r.id, sufixo);
            let p = self.dir.join(&id);
            if std::fs::create_dir(&p).is_ok() {
                break p;
            }
            sufixo = sufixo.wrapping_add(1);
        };
        let id = pasta
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        Self::gravar(&pasta.join("receita.json"), r.canonico().as_bytes())?;
        let info = Info {
            id,
            receita_id: r.id.clone(),
            nome: r.nome.clone(),
            classes: r.nomes_classes(),
            situacao: Situacao::AguardandoOnnx,
            ligado: false,
            somente_estudo: true,
            importado_em: agora(),
            receita_sha256: r.sha256(),
            onnx_sha256: None,
            onnx_bytes: None,
            esperado_sha256: esperado,
            metricas: r.estudo.metricas,
        };
        self.gravar_info(&pasta, &info)?;
        Ok(info)
    }

    /// Passo 2: grava o ONNX, conferindo o SHA-256 do manifesto quando houver.
    pub fn gravar_onnx(&self, id: &str, dados: &[u8]) -> Result<Info, Falha> {
        let pasta = self.pasta(id)?;
        let mut info = self.ler_info(&pasta)?;
        if dados.is_empty() || dados.len() > ONNX_MAX {
            return Err(Falha::Invalido(vec![format!(
                "o arquivo ONNX precisa ter entre 1 byte e {} MB",
                ONNX_MAX / 1024 / 1024
            )]));
        }
        // ModelProto do ONNX começa pelo campo 1 (ir_version, varint): byte 0x08.
        if dados[0] != 0x08 {
            return Err(Falha::Invalido(vec![
                "isto não parece um modelo ONNX (use o .onnx da pasta exportado/ do Colab)".into(),
            ]));
        }
        let sha = receita::hex(&Sha256::digest(dados));
        if let Some(esp) = &info.esperado_sha256 {
            if *esp != sha {
                return Err(Falha::Invalido(vec![
                    "o SHA-256 do ONNX não confere com o manifesto: o arquivo mudou ou é de outro treino".into(),
                ]));
            }
        }
        Self::gravar(&pasta.join("modelo.onnx"), dados)?;
        info.onnx_sha256 = Some(sha);
        info.onnx_bytes = Some(dados.len() as u64);
        info.situacao = Situacao::Pronto;
        info.ligado = false;
        self.gravar_info(&pasta, &info)?;
        Ok(info)
    }

    pub fn listar(&self) -> Vec<Info> {
        let Ok(it) = std::fs::read_dir(&self.dir) else {
            return vec![];
        };
        let mut v: Vec<Info> = it
            .flatten()
            .filter(|e| e.path().join("info.json").is_file())
            .filter_map(|e| self.ler_info(&e.path()).ok())
            .collect();
        v.sort_by(|a, b| b.importado_em.cmp(&a.importado_em).then(a.id.cmp(&b.id)));
        v
    }

    /// "Apagar tudo" de um modelo de estudo (receita, manifesto e ONNX).
    pub fn apagar(&self, id: &str) -> Result<(), Falha> {
        let pasta = self.pasta(id)?;
        std::fs::remove_dir_all(pasta).map_err(io)
    }
}

fn conferir_manifesto(r: &Receita, m: Value) -> Result<String, Falha> {
    let m: Manifesto = serde_json::from_value(m)
        .map_err(|_| Falha::Invalido(vec!["manifesto.json com formato desconhecido".into()]))?;
    let mut e = Vec::new();
    if m.formato != "camfacil-manifesto" || !m.somente_estudo {
        e.push("manifesto: não é um manifesto de modelo de estudo do CamFácil IA".to_string());
    }
    if m.receita_id != r.id {
        e.push(format!(
            "manifesto: é da receita \"{}\", não de \"{}\"",
            m.receita_id, r.id
        ));
    }
    if m.classes != r.modelo.classes {
        e.push("manifesto: as classes não batem com as da receita".into());
    }
    let sha = m.onnx_sha256.to_ascii_lowercase();
    if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        e.push("manifesto: onnx_sha256 inválido".into());
    }
    if e.is_empty() {
        Ok(sha)
    } else {
        Err(Falha::Invalido(e))
    }
}

// ---------------------------------------------------------------------------
// Rotas

fn com_aviso(info: &Info) -> Value {
    let mut v = serde_json::to_value(info).unwrap_or(Value::Null);
    v["aviso"] = json!(AVISO_ESTUDO);
    v["executa"] = json!(false);
    v
}

fn exigir_termo(app: &Shared) -> Result<(), (StatusCode, Json<Value>)> {
    let ok = app
        .store
        .accepted(crate::termos::DOC_TECNICO, crate::termos::VERSAO_TECNICO)
        .map_err(internal)?;
    if ok {
        Ok(())
    } else {
        Err(bad(
            StatusCode::PRECONDITION_FAILED,
            "Antes de importar um modelo de estudo, aceite o Termo de Responsabilidade do Técnico em Integrações → Termos e LGPD.",
        ))
    }
}

/// GET /api/estudio/modelos
pub async fn get_modelos(State(app): State<Shared>) -> Json<Value> {
    let lista: Vec<Value> = app.estudio.listar().iter().map(com_aviso).collect();
    Json(json!({
        "modelos": lista,
        "aviso": AVISO_ESTUDO,
        "executa": false,
        "motivo": MOTIVO_NAO_EXECUTA,
    }))
}

#[derive(Deserialize)]
pub struct Importar {
    receita: Value,
    #[serde(default)]
    manifesto: Option<Value>,
}

/// POST /api/estudio/modelos — passo 1 (receita + manifesto).
pub async fn post_modelo(State(app): State<Shared>, Json(p): Json<Importar>) -> ApiResult {
    exigir_termo(&app)?;
    let info = app
        .estudio
        .importar(p.receita, p.manifesto)
        .map_err(Falha::http)?;
    Ok(Json(json!({
        "modelo": com_aviso(&info),
        "proximo": format!("/api/estudio/modelos/{}/onnx", info.id),
    })))
}

/// PUT /api/estudio/modelos/{id}/onnx — passo 2 (arquivo ONNX no corpo).
pub async fn put_onnx(
    State(app): State<Shared>,
    Path(id): Path<String>,
    corpo: Bytes,
) -> ApiResult {
    exigir_termo(&app)?;
    let app2 = app.clone();
    let info = tokio::task::spawn_blocking(move || app2.estudio.gravar_onnx(&id, &corpo))
        .await
        .map_err(internal)?
        .map_err(Falha::http)?;
    Ok(Json(json!({
        "modelo": com_aviso(&info),
        "motivo": MOTIVO_NAO_EXECUTA,
    })))
}

/// DELETE /api/estudio/modelos/{id}
pub async fn delete_modelo(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult {
    app.estudio.apagar(&id).map_err(Falha::http)?;
    Ok(Json(json!({ "apagado": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasta_teste(nome: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cf-estudio-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn receita() -> Value {
        serde_json::from_str(&receita::exemplo().canonico()).unwrap()
    }

    fn onnx_falso() -> Vec<u8> {
        let mut v = vec![0x08, 0x07, 0x12, 0x07];
        v.extend_from_slice(b"pytorch");
        v
    }

    fn manifesto(sha: &str) -> Value {
        json!({
            "formato": "camfacil-manifesto", "versao": 1, "somente_estudo": true,
            "aviso": AVISO_ESTUDO, "receita_id": "contar-caixas-banana",
            "onnx_sha256": sha, "classes": ["caixa_banana"], "lado": 640,
        })
    }

    #[test]
    fn importa_em_dois_passos_e_fica_desligado() {
        let d = pasta_teste("ok");
        let e = Estudio::new(&d);
        let sha = receita::hex(&Sha256::digest(onnx_falso()));
        let info = e.importar(receita(), Some(manifesto(&sha))).unwrap();
        assert_eq!(info.situacao, Situacao::AguardandoOnnx);
        assert!(info.id.starts_with("contar-caixas-banana-"));
        assert!(!info.ligado && info.somente_estudo);
        assert_eq!(info.classes, vec!["caixa de banana"]);

        let info = e.gravar_onnx(&info.id, &onnx_falso()).unwrap();
        assert_eq!(info.situacao, Situacao::Pronto);
        assert!(!info.ligado);
        assert_eq!(info.onnx_sha256.as_deref(), Some(sha.as_str()));
        let pasta = d.join("estudio").join(&info.id);
        assert!(pasta.join("modelo.onnx").is_file());
        assert!(pasta.join("receita.json").is_file());

        let lista = e.listar();
        assert_eq!(lista.len(), 1);
        let v = com_aviso(&lista[0]);
        assert_eq!(v["aviso"], AVISO_ESTUDO);
        assert_eq!(v["executa"], false);
        assert_eq!(v["ligado"], false);

        e.apagar(&info.id).unwrap();
        assert!(!pasta.exists());
        assert!(e.listar().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn sem_manifesto_aceita_qualquer_onnx_valido() {
        let d = pasta_teste("sem");
        let e = Estudio::new(&d);
        let info = e.importar(receita(), None).unwrap();
        assert!(e.gravar_onnx(&info.id, &onnx_falso()).is_ok());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn recusa_hash_diferente_e_arquivo_que_nao_e_onnx() {
        let d = pasta_teste("hash");
        let e = Estudio::new(&d);
        let info = e
            .importar(receita(), Some(manifesto(&"a".repeat(64))))
            .unwrap();
        let Err(Falha::Invalido(m)) = e.gravar_onnx(&info.id, &onnx_falso()) else {
            panic!("devia recusar")
        };
        assert!(m[0].contains("não confere"));
        let Err(Falha::Invalido(m)) = e.gravar_onnx(&info.id, b"\x89PNG") else {
            panic!("devia recusar")
        };
        assert!(m[0].contains("não parece um modelo ONNX"));
        assert!(matches!(
            e.gravar_onnx(&info.id, &[]),
            Err(Falha::Invalido(_))
        ));
        assert_eq!(e.listar()[0].situacao, Situacao::AguardandoOnnx);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn manifesto_de_outra_receita_e_biometria_sao_recusados() {
        let d = pasta_teste("outra");
        let e = Estudio::new(&d);
        let mut m = manifesto(&"b".repeat(64));
        m["receita_id"] = json!("contar-pallets");
        m["classes"] = json!(["pallet"]);
        let Err(Falha::Invalido(erros)) = e.importar(receita(), Some(m)) else {
            panic!("devia recusar")
        };
        assert_eq!(erros.len(), 2, "{erros:?}");

        let mut r = receita();
        r["nome"] = json!("Reconhecimento facial na entrada");
        let Err(Falha::Invalido(erros)) = e.importar(r, None) else {
            panic!("devia recusar")
        };
        assert!(erros[0].contains("LGPD art. 11"));
        assert!(e.listar().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn ids_estranhos_nao_saem_da_pasta() {
        let d = pasta_teste("ids");
        let e = Estudio::new(&d);
        for id in ["../etc", "", "a/b", "A", "..", "x".repeat(81).as_str()] {
            assert_eq!(e.apagar(id), Err(Falha::NaoEncontrado), "{id}");
            assert!(matches!(
                e.gravar_onnx(id, &onnx_falso()),
                Err(Falha::NaoEncontrado)
            ));
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
