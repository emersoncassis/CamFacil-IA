//! Modo estúdio no programa local (SDD-07): **coletar fotos** de uma câmera
//! para rotular no site e importar o **modelo de estudo** treinado no Google
//! Colab (ONNX + receita + manifesto).
//!
//! O modelo fica em `data_dir/estudio/<id>/`, marcado "somente estudo" e
//! **desligado**. O técnico liga um por vez, à mão: o detector troca o YOLO
//! do COCO pelo ONNX de estudo e o balão da regra mostra o nome da classe da
//! receita (nunca caixas para o cliente).
//!
//! As fotos coletadas ficam em `data_dir/estudio/<receita>/fotos/` e não vão a
//! lugar nenhum: o técnico abre essa pasta na ferramenta de rotular do site,
//! que roda inteira no navegador.
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

/// Como o modelo de estudo roda.
pub const COMO_EXECUTA: &str = "Ligue um modelo de estudo por vez. Enquanto ligado, o detector usa o seu ONNX no lugar do modelo padrão (pessoa, carro, moto…) e o balão da regra mostra o nome da classe. Desligue para voltar ao modelo padrão.";

/// Limites da coleta de fotos.
pub const COLETA_MAX: u32 = 500;
pub const INTERVALO_MIN_S: u32 = 2;
pub const INTERVALO_MAX_S: u32 = 3600;

/// Modelo de estudo ligado (o que o detector precisa para rodar).
#[derive(Debug, Clone, PartialEq)]
pub struct ModeloEstudo {
    pub id: String,
    pub nome: String,
    pub onnx: PathBuf,
    /// Nomes de exibição na ordem das classes do modelo.
    pub nomes: Vec<String>,
    pub lado: usize,
    pub confianca: f32,
}

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

    /// Liga (só um por vez) ou desliga um modelo de estudo pronto.
    pub fn ligar(&self, id: &str, ligar: bool) -> Result<Info, Falha> {
        let pasta = self.pasta(id)?;
        let mut info = self.ler_info(&pasta)?;
        if ligar && info.situacao != Situacao::Pronto {
            return Err(Falha::Invalido(vec![
                "importe o arquivo .onnx antes de ligar o modelo de estudo".into(),
            ]));
        }
        if ligar {
            for mut outro in self.listar() {
                if outro.id != info.id && outro.ligado {
                    outro.ligado = false;
                    if let Ok(p) = self.pasta(&outro.id) {
                        self.gravar_info(&p, &outro)?;
                    }
                }
            }
        }
        info.ligado = ligar;
        self.gravar_info(&pasta, &info)?;
        Ok(info)
    }

    /// O modelo de estudo ligado, se houver (lê a receita guardada).
    pub fn ativo(&self) -> Option<ModeloEstudo> {
        let info = self
            .listar()
            .into_iter()
            .find(|i| i.ligado && i.situacao == Situacao::Pronto)?;
        let pasta = self.pasta(&info.id).ok()?;
        let r = receita::ler(&std::fs::read_to_string(pasta.join("receita.json")).ok()?).ok()?;
        let onnx = pasta.join("modelo.onnx");
        onnx.is_file().then(|| ModeloEstudo {
            id: info.id.clone(),
            nome: r.nome.clone(),
            onnx,
            nomes: r.nomes_classes(),
            lado: r.modelo.lado as usize,
            confianca: r.modelo.confianca,
        })
    }

    /// Pasta das fotos coletadas para uma receita (`estudio/<receita>/fotos`).
    pub fn pasta_fotos(&self, receita_id: &str) -> Result<PathBuf, Falha> {
        if !id_receita_valido(receita_id) {
            return Err(Falha::Invalido(vec![
                "nome da receita: use o identificador curto da receita (letras minúsculas, números e hífen, como contar-caixas-banana)".into(),
            ]));
        }
        Ok(self.dir.join(receita_id).join("fotos"))
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

/// Id da receita (igual ao do `cf_core::receita`): 3 a 60, começa com letra ou número.
fn id_receita_valido(id: &str) -> bool {
    (3..=60).contains(&id.len())
        && id
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

// ---------------------------------------------------------------------------
// Coleta de fotos (uma por vez, em segundo plano)

#[derive(Debug, Clone, Serialize, Default)]
pub struct EstadoColeta {
    pub ativa: bool,
    pub receita: String,
    pub camera_id: i64,
    pub intervalo_s: u32,
    pub total: u32,
    pub salvas: u32,
    pub pasta: String,
    pub erro: Option<String>,
}

#[derive(Default)]
pub struct Coleta {
    estado: std::sync::Mutex<EstadoColeta>,
    parar: std::sync::atomic::AtomicBool,
}

impl Coleta {
    pub fn estado(&self) -> EstadoColeta {
        self.estado.lock().unwrap().clone()
    }

    pub fn parar(&self) {
        self.parar.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Nome do arquivo da n-ésima foto (ordem e horário no nome, sem dado do cliente).
fn nome_foto(receita: &str, quando: i64, n: u32) -> String {
    format!("{receita}-{quando}-{n:04}.jpg")
}

/// Laço da coleta: guarda o último quadro da câmera a cada intervalo, sem
/// repetir o mesmo quadro, até juntar `total` ou alguém parar.
fn coletar(
    coleta: std::sync::Arc<Coleta>,
    video: std::sync::Arc<crate::video::Video>,
    pasta: PathBuf,
) {
    use std::sync::atomic::Ordering;
    let (camera, intervalo, total, receita) = {
        let e = coleta.estado.lock().unwrap();
        (e.camera_id, e.intervalo_s, e.total, e.receita.clone())
    };
    let mut anterior: Option<std::sync::Arc<Vec<u8>>> = None;
    let mut salvas = 0u32;
    let fim = |erro: Option<String>| {
        let mut e = coleta.estado.lock().unwrap();
        e.ativa = false;
        e.erro = erro;
    };
    while salvas < total && !coleta.parar.load(Ordering::SeqCst) {
        let quadro = video
            .canal(camera)
            .and_then(|c| c.ultimo.lock().unwrap().clone());
        match quadro {
            Some(q)
                if !anterior
                    .as_ref()
                    .is_some_and(|a| std::sync::Arc::ptr_eq(a, &q)) =>
            {
                salvas += 1;
                let arq = pasta.join(nome_foto(&receita, agora(), salvas));
                if let Err(e) = Estudio::gravar(&arq, &q) {
                    fim(Some(format!("não gravou a foto: {e:?}")));
                    return;
                }
                {
                    let mut e = coleta.estado.lock().unwrap();
                    e.salvas = salvas;
                    e.erro = None;
                }
                anterior = Some(q);
            }
            _ => {
                coleta.estado.lock().unwrap().erro =
                    Some("a câmera não mandou imagem nova; confira se ela está online".into());
            }
        }
        // Dorme em passos curtos para "parar" responder logo.
        let mut resta = intervalo as u64 * 1000;
        while resta > 0 && !coleta.parar.load(Ordering::SeqCst) {
            let passo = resta.min(200);
            std::thread::sleep(std::time::Duration::from_millis(passo));
            resta -= passo;
        }
    }
    fim(None);
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
    v["executa"] = json!(info.ligado && info.situacao == Situacao::Pronto);
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
            "Antes de usar o modo estúdio (coletar fotos ou importar modelo), aceite o Termo de Responsabilidade do Técnico em Integrações → Termos e LGPD.",
        ))
    }
}

/// Avisa o detector de qual modelo de estudo está ligado agora.
pub fn sincronizar(app: &Shared) {
    *app.ia.estudo.lock().unwrap() = app.estudio.ativo().map(std::sync::Arc::new);
}

/// GET /api/estudio/modelos
pub async fn get_modelos(State(app): State<Shared>) -> Json<Value> {
    let lista: Vec<Value> = app.estudio.listar().iter().map(com_aviso).collect();
    let ativo = app.ia.estudo.lock().unwrap().as_ref().map(|m| m.id.clone());
    Json(json!({
        "modelos": lista,
        "aviso": AVISO_ESTUDO,
        "ativo": ativo,
        "como": COMO_EXECUTA,
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
    sincronizar(&app);
    Ok(Json(json!({
        "modelo": com_aviso(&info),
        "como": COMO_EXECUTA,
    })))
}

#[derive(Deserialize)]
pub struct Ligar {
    ligado: bool,
}

/// POST /api/estudio/modelos/{id}/ligar `{ligado}` — liga um (desliga os outros).
pub async fn post_ligar(
    State(app): State<Shared>,
    Path(id): Path<String>,
    Json(p): Json<Ligar>,
) -> ApiResult {
    if p.ligado {
        exigir_termo(&app)?;
    }
    let info = app.estudio.ligar(&id, p.ligado).map_err(Falha::http)?;
    sincronizar(&app);
    Ok(Json(
        json!({ "modelo": com_aviso(&info), "como": COMO_EXECUTA }),
    ))
}

/// DELETE /api/estudio/modelos/{id}
pub async fn delete_modelo(State(app): State<Shared>, Path(id): Path<String>) -> ApiResult {
    app.estudio.apagar(&id).map_err(Falha::http)?;
    sincronizar(&app);
    Ok(Json(json!({ "apagado": true })))
}

#[derive(Deserialize)]
pub struct PedidoColeta {
    camera_id: i64,
    receita: String,
    #[serde(default = "intervalo_padrao")]
    intervalo_s: u32,
    #[serde(default = "total_padrao")]
    total: u32,
}

fn intervalo_padrao() -> u32 {
    30
}
fn total_padrao() -> u32 {
    150
}

fn estado_coleta(app: &Shared) -> Value {
    let e = app.coleta.estado();
    let fotos = app
        .estudio
        .pasta_fotos(&e.receita)
        .ok()
        .and_then(|p| std::fs::read_dir(p).ok())
        .map(|it| it.flatten().count())
        .unwrap_or(0);
    let mut v = serde_json::to_value(&e).unwrap_or(Value::Null);
    v["fotos_na_pasta"] = json!(fotos);
    v["aviso"] = json!(AVISO_ESTUDO);
    v
}

/// GET /api/estudio/coleta — situação da coleta atual (ou da última).
pub async fn get_coleta(State(app): State<Shared>) -> Json<Value> {
    Json(estado_coleta(&app))
}

/// POST /api/estudio/coleta — começa a guardar fotos de uma câmera numa pasta
/// local (`estudio/<receita>/fotos`). Nada é enviado a lugar nenhum.
pub async fn post_coleta(State(app): State<Shared>, Json(p): Json<PedidoColeta>) -> ApiResult {
    exigir_termo(&app)?;
    if app.coleta.estado().ativa {
        return Err(bad(
            StatusCode::CONFLICT,
            "já há uma coleta em andamento; pare a atual antes de começar outra",
        ));
    }
    let mut erros = Vec::new();
    if !(1..=COLETA_MAX).contains(&p.total) {
        erros.push(format!("quantidade: de 1 a {COLETA_MAX} fotos"));
    }
    if !(INTERVALO_MIN_S..=INTERVALO_MAX_S).contains(&p.intervalo_s) {
        erros.push(format!(
            "intervalo: de {INTERVALO_MIN_S} a {INTERVALO_MAX_S} segundos"
        ));
    }
    let canal = app.video.canal(p.camera_id).filter(|c| !c.ensaio);
    if canal.is_none() {
        erros.push("câmera não encontrada ou desligada".into());
    }
    let pasta = match app.estudio.pasta_fotos(&p.receita) {
        Ok(p) => Some(p),
        Err(Falha::Invalido(e)) => {
            erros.extend(e);
            None
        }
        Err(e) => return Err(e.http()),
    };
    if !erros.is_empty() {
        return Err(Falha::Invalido(erros).http());
    }
    let pasta = pasta.unwrap_or_default();
    std::fs::create_dir_all(&pasta).map_err(internal)?;
    {
        let mut e = app.coleta.estado.lock().unwrap();
        *e = EstadoColeta {
            ativa: true,
            receita: p.receita,
            camera_id: p.camera_id,
            intervalo_s: p.intervalo_s,
            total: p.total,
            salvas: 0,
            pasta: pasta.display().to_string(),
            erro: None,
        };
    }
    app.coleta
        .parar
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let (coleta, video) = (app.coleta.clone(), app.video.clone());
    std::thread::Builder::new()
        .name("estudio-coleta".into())
        .spawn(move || coletar(coleta, video, pasta))
        .map_err(internal)?;
    Ok(Json(estado_coleta(&app)))
}

/// POST /api/estudio/coleta/parar
pub async fn post_coleta_parar(State(app): State<Shared>) -> Json<Value> {
    app.coleta.parar();
    Json(estado_coleta(&app))
}

/// DELETE /api/estudio/coleta/{receita} — "apagar tudo": as fotos coletadas.
pub async fn delete_coleta(State(app): State<Shared>, Path(receita): Path<String>) -> ApiResult {
    let e = app.coleta.estado();
    if e.ativa && e.receita == receita {
        return Err(bad(
            StatusCode::CONFLICT,
            "pare a coleta antes de apagar as fotos",
        ));
    }
    let fotos = app.estudio.pasta_fotos(&receita).map_err(Falha::http)?;
    if let Some(dir) = fotos.parent() {
        // Só a pasta da coleta (sem info.json: não é modelo importado).
        if dir.join("info.json").exists() {
            return Err(Falha::NaoEncontrado.http());
        }
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(internal)?;
        }
    }
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
        // Ligado, vira o modelo ativo; ligar outro desliga este.
        assert!(e.ativo().is_none());
        let ligado = e.ligar(&info.id, true).unwrap();
        assert!(ligado.ligado);
        let m = e.ativo().expect("modelo ativo");
        assert_eq!(m.nomes, vec!["caixa de banana"]);
        assert_eq!(m.lado, 640);
        assert!(m.onnx.ends_with("modelo.onnx"));
        let outro = e.importar(receita(), None).unwrap();
        assert!(matches!(e.ligar(&outro.id, true), Err(Falha::Invalido(_))));
        e.gravar_onnx(&outro.id, &onnx_falso()).unwrap();
        e.ligar(&outro.id, true).unwrap();
        assert_eq!(e.ativo().unwrap().id, outro.id);
        assert!(e.listar().iter().filter(|i| i.ligado).count() == 1);
        e.ligar(&outro.id, false).unwrap();
        assert!(e.ativo().is_none());
        e.apagar(&outro.id).unwrap();
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
    fn pasta_de_fotos_so_aceita_id_de_receita() {
        let d = pasta_teste("fotos");
        let e = Estudio::new(&d);
        let p = e.pasta_fotos("contar-caixas-banana").unwrap();
        assert!(p.ends_with("estudio/contar-caixas-banana/fotos"));
        for ruim in ["../x", "ab", "A-b", "a/b", "-abc", "a_b_c"] {
            assert!(e.pasta_fotos(ruim).is_err(), "{ruim}");
        }
        assert_eq!(nome_foto("abc", 1700000000, 7), "abc-1700000000-0007.jpg");
        // A pasta da coleta não aparece como modelo importado.
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(p.join("x.jpg"), b"jpg").unwrap();
        assert!(e.listar().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn nome_do_objeto_no_balao() {
        use crate::video::nome_objeto;
        use cf_core::deteccao::CLASSE_ESTUDO;
        let nomes = vec!["caixa de banana".to_string(), "pallet".to_string()];
        assert_eq!(nome_objeto(CLASSE_ESTUDO + 1, Some(&nomes)), "pallet");
        assert_eq!(nome_objeto(CLASSE_ESTUDO + 5, Some(&nomes)), "objeto");
        assert_eq!(nome_objeto(0, Some(&nomes)), "pessoa");
        assert_eq!(nome_objeto(CLASSE_ESTUDO, None), "objeto");
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
