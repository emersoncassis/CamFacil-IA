//! Receita de módulo do modo estúdio (SDD-07, ADR 0013).
//!
//! A receita descreve uma inteligência criada pelo técnico: os objetos que o
//! modelo aprende (classes), as regras de cena (zonas, linhas, trajetórias,
//! contagens) e os metadados do estudo (fotos, épocas, métricas). É o que se
//! **compartilha** com outros técnicos: nunca leva fotos, só a descrição.
//!
//! O formato é JSON, descrito em `docs/esquemas/receita-v1.schema.json`; a
//! validação de verdade é esta, em Rust, com mensagens em pt-BR. Reconhecimento
//! facial e biometria são bloqueados (dado sensível, LGPD art. 11).

use crate::visao::Ponto;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const FORMATO: &str = "camfacil-receita";
pub const VERSAO: u32 = 1;

/// Aviso obrigatório em toda receita, notebook e modelo exportado.
pub const AVISO_ESTUDO: &str = "Somente para estudo e aprendizagem. Não use em produção: para uso real, procure soluções homologadas (câmeras e gravadores com IA de borda ou plataformas corporativas de IA).";

/// Esquema JSON (documentação e ferramentas externas; a regra vale aqui).
pub const ESQUEMA: &str = include_str!("../../../docs/esquemas/receita-v1.schema.json");

/// Tamanho máximo do JSON da receita (não cabe foto aqui dentro).
pub const TAMANHO_MAX: usize = 64 * 1024;
pub const MAX_CLASSES: usize = 20;
pub const MAX_REGRAS: usize = 32;
const MAX_PONTOS: usize = 32;
/// Palavra (sem espaço) mais longa aceita num texto livre: base64 de foto não passa.
const MAX_PALAVRA: usize = 120;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receita {
    pub formato: String,
    pub versao: u32,
    /// Identificador curto (`contar-caixas-banana`): pasta no Drive e nome do ONNX.
    pub id: String,
    pub nome: String,
    #[serde(default)]
    pub descricao: String,
    pub somente_estudo: bool,
    pub aviso: String,
    pub modelo: Modelo,
    pub objetos: Vec<Objeto>,
    #[serde(default)]
    pub zonas: Vec<Zona>,
    #[serde(default)]
    pub linhas: Vec<Linha>,
    #[serde(default)]
    pub trajetorias: Vec<Trajetoria>,
    #[serde(default)]
    pub contagens: Vec<Contagem>,
    pub estudo: Estudo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Base {
    Yolo11n,
    Yolov8n,
}

impl Base {
    /// Pesos iniciais do Ultralytics usados no Colab.
    pub fn pesos(self) -> &'static str {
        match self {
            Base::Yolo11n => "yolo11n.pt",
            Base::Yolov8n => "yolov8n.pt",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Modelo {
    pub base: Base,
    /// Lado da imagem de entrada (múltiplo de 32).
    #[serde(default = "lado_padrao")]
    pub lado: u32,
    /// Ids dos objetos na ordem dos índices do modelo (classe 0, 1, …).
    pub classes: Vec<String>,
    #[serde(default = "confianca_padrao")]
    pub confianca: f32,
}

fn lado_padrao() -> u32 {
    640
}
fn confianca_padrao() -> f32 {
    0.35
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Objeto {
    pub id: String,
    /// Nome mostrado no balão ("caixa de banana").
    pub nome: String,
    #[serde(default)]
    pub descricao: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Zona {
    pub id: String,
    pub nome: String,
    pub objetos: Vec<String>,
    /// Polígono em coordenadas normalizadas (0 a 1).
    pub pontos: Vec<Ponto>,
    /// Avisar se ficar mais que isto dentro da zona (permanência).
    #[serde(default)]
    pub permanencia_s: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Sentido {
    Entrada,
    Saida,
    #[default]
    Ambos,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Linha {
    pub id: String,
    pub nome: String,
    pub objetos: Vec<String>,
    /// Dois pontos normalizados.
    pub pontos: Vec<Ponto>,
    #[serde(default)]
    pub sentido: Sentido,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trajetoria {
    pub id: String,
    pub nome: String,
    pub objetos: Vec<String>,
    /// Caminho esperado (2 a 32 pontos normalizados).
    pub pontos: Vec<Ponto>,
    /// Distância máxima do caminho, em fração da imagem.
    #[serde(default = "tolerancia_padrao")]
    pub tolerancia: f32,
}

fn tolerancia_padrao() -> f32 {
    0.1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Janela {
    Hora,
    Turno,
    #[default]
    Dia,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contagem {
    pub id: String,
    pub nome: String,
    pub objetos: Vec<String>,
    /// Conta quem cruza esta linha…
    #[serde(default)]
    pub linha: Option<String>,
    /// …ou quem entra nesta zona (exatamente um dos dois).
    #[serde(default)]
    pub zona: Option<String>,
    #[serde(default)]
    pub janela: Janela,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Estudo {
    /// Apelido público de quem montou (opcional, nunca e-mail).
    #[serde(default)]
    pub autor: String,
    /// Tipo de lugar ("doca de carga"), sem endereço nem nome do cliente.
    #[serde(default)]
    pub local: String,
    /// Quantas fotos rotuladas foram usadas (só o número).
    #[serde(default)]
    pub fotos: u32,
    #[serde(default)]
    pub epocas: u32,
    #[serde(default)]
    pub metricas: Option<Metricas>,
    #[serde(default)]
    pub notas: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Metricas {
    #[serde(default)]
    pub map50: Option<f32>,
    #[serde(default)]
    pub map50_95: Option<f32>,
    #[serde(default)]
    pub precisao: Option<f32>,
    #[serde(default)]
    pub revocacao: Option<f32>,
}

/// Épocas usadas quando a receita não diz.
pub const EPOCAS_PADRAO: u32 = 60;

impl Receita {
    /// JSON canônico (ordem fixa dos campos): base do hash.
    pub fn canonico(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// SHA-256 (hex) do JSON canônico.
    pub fn sha256(&self) -> String {
        hex(&Sha256::digest(self.canonico().as_bytes()))
    }

    pub fn epocas(&self) -> u32 {
        if self.estudo.epocas == 0 {
            EPOCAS_PADRAO
        } else {
            self.estudo.epocas
        }
    }

    /// Nomes de exibição na ordem das classes do modelo.
    pub fn nomes_classes(&self) -> Vec<String> {
        self.modelo
            .classes
            .iter()
            .map(|c| {
                self.objetos
                    .iter()
                    .find(|o| &o.id == c)
                    .map(|o| o.nome.clone())
                    .unwrap_or_else(|| c.clone())
            })
            .collect()
    }
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Receita de exemplo (a do mockup): contar caixas de banana na porta do estoque.
pub fn exemplo() -> Receita {
    let p = |x, y| Ponto { x, y };
    Receita {
        formato: FORMATO.into(),
        versao: VERSAO,
        id: "contar-caixas-banana".into(),
        nome: "Contar caixas de banana".into(),
        descricao: "Conta as caixas de banana que entram no estoque pela porta dos fundos.".into(),
        somente_estudo: true,
        aviso: AVISO_ESTUDO.into(),
        modelo: Modelo {
            base: Base::Yolo11n,
            lado: 640,
            classes: vec!["caixa_banana".into()],
            confianca: 0.35,
        },
        objetos: vec![Objeto {
            id: "caixa_banana".into(),
            nome: "caixa de banana".into(),
            descricao: "Caixa plástica ou de papelão com cachos de banana.".into(),
        }],
        zonas: vec![Zona {
            id: "estoque".into(),
            nome: "Área do estoque".into(),
            objetos: vec!["caixa_banana".into()],
            pontos: vec![p(0.05, 0.4), p(0.6, 0.4), p(0.6, 0.95), p(0.05, 0.95)],
            permanencia_s: None,
        }],
        linhas: vec![Linha {
            id: "porta".into(),
            nome: "Porta dos fundos".into(),
            objetos: vec!["caixa_banana".into()],
            pontos: vec![p(0.7, 0.3), p(0.7, 0.9)],
            sentido: Sentido::Entrada,
        }],
        trajetorias: vec![],
        contagens: vec![Contagem {
            id: "entradas".into(),
            nome: "Caixas que entraram".into(),
            objetos: vec!["caixa_banana".into()],
            linha: Some("porta".into()),
            zona: None,
            janela: Janela::Dia,
        }],
        estudo: Estudo {
            autor: String::new(),
            local: "estoque de hortifrúti".into(),
            fotos: 0,
            epocas: 0,
            metricas: None,
            notas: String::new(),
        },
    }
}

// ---------------------------------------------------------------------------
// Leitura e validação

/// Lê e valida uma receita a partir do texto JSON.
pub fn ler(texto: &str) -> Result<Receita, Vec<String>> {
    if texto.len() > TAMANHO_MAX {
        return Err(vec![format!(
            "a receita passa de {} KB; ela leva só a descrição, nunca fotos",
            TAMANHO_MAX / 1024
        )]);
    }
    let r: Receita = serde_json::from_str(texto).map_err(|e| vec![traduzir_serde(&e)])?;
    validar(&r)?;
    Ok(r)
}

/// Lê e valida uma receita a partir de um valor JSON já decodificado.
pub fn de_valor(v: serde_json::Value) -> Result<Receita, Vec<String>> {
    ler(&v.to_string())
}

/// Erros do serde em pt-BR (os mais comuns; o resto vai com o texto original).
fn traduzir_serde(e: &serde_json::Error) -> String {
    let m = e.to_string();
    let onde = format!("(linha {}, coluna {})", e.line(), e.column());
    let crase = |s: &str| s.split('`').nth(1).unwrap_or("").to_string();
    let base = if m.starts_with("missing field") {
        format!("falta o campo `{}`", crase(&m))
    } else if m.starts_with("unknown field") {
        format!(
            "campo desconhecido `{}` (a receita só aceita os campos do formato; fotos não entram)",
            crase(&m)
        )
    } else if m.starts_with("unknown variant") {
        format!("valor não permitido `{}`", crase(&m))
    } else if m.starts_with("invalid type") {
        "tipo de valor errado (ex.: texto onde devia ser número)".to_string()
    } else if m.starts_with("invalid value") {
        "valor inválido".to_string()
    } else if m.contains("EOF") {
        "JSON incompleto".to_string()
    } else {
        format!("JSON inválido: {m}")
    };
    format!("{base} {onde}")
}

fn id_ok(s: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&s.len())
        && s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Valida a receita. Devolve todas as mensagens de uma vez, em pt-BR.
pub fn validar(r: &Receita) -> Result<(), Vec<String>> {
    let mut e: Vec<String> = Vec::new();

    if r.formato != FORMATO {
        e.push(format!("formato: precisa ser \"{FORMATO}\""));
    }
    if r.versao != VERSAO {
        e.push(format!(
            "versao: esta versão do CamFácil IA lê receitas da versão {VERSAO}"
        ));
    }
    if !id_ok(&r.id, 3, 60) || r.id.contains('_') {
        e.push("id: use de 3 a 60 letras minúsculas, números e hífen (ex.: contar-pallets)".into());
    }
    texto(&mut e, "nome", &r.nome, 3, 80);
    texto(&mut e, "descricao", &r.descricao, 0, 1000);
    if !r.somente_estudo {
        e.push("somente_estudo: precisa ser true; o modo estúdio é só para estudo".into());
    }
    if r.aviso != AVISO_ESTUDO {
        e.push("aviso: use o texto padrão de \"somente estudo\" sem alterar".into());
    }

    // Modelo e objetos
    let m = &r.modelo;
    if !(320..=1280).contains(&m.lado) || !m.lado.is_multiple_of(32) {
        e.push("modelo.lado: use um múltiplo de 32 entre 320 e 1280 (padrão 640)".into());
    }
    if !(0.05..=0.95).contains(&m.confianca) {
        e.push("modelo.confianca: use um valor entre 0,05 e 0,95".into());
    }
    if r.objetos.is_empty() {
        e.push("objetos: descreva ao menos um objeto".into());
    }
    if r.objetos.len() > MAX_CLASSES {
        e.push(format!(
            "objetos: no máximo {MAX_CLASSES} (um modelo nano aprende melhor poucas classes)"
        ));
    }
    let mut ids_obj = HashSet::new();
    for (i, o) in r.objetos.iter().enumerate() {
        if !id_ok(&o.id, 1, 40) {
            e.push(format!(
                "objetos[{i}].id: use letras minúsculas, números, _ ou - (ex.: caixa_banana)"
            ));
        }
        if !ids_obj.insert(o.id.as_str()) {
            e.push(format!("objetos[{i}].id: \"{}\" repetido", o.id));
        }
        texto(&mut e, &format!("objetos[{i}].nome"), &o.nome, 1, 40);
        texto(
            &mut e,
            &format!("objetos[{i}].descricao"),
            &o.descricao,
            0,
            200,
        );
    }
    let mut vistas = HashSet::new();
    for (i, c) in m.classes.iter().enumerate() {
        if !ids_obj.contains(c.as_str()) {
            e.push(format!(
                "modelo.classes[{i}]: \"{c}\" não está na lista de objetos"
            ));
        }
        if !vistas.insert(c.as_str()) {
            e.push(format!("modelo.classes[{i}]: \"{c}\" repetida"));
        }
    }
    for o in &r.objetos {
        if !vistas.contains(o.id.as_str()) {
            e.push(format!(
                "modelo.classes: falta o objeto \"{}\" (cada objeto é uma classe do modelo)",
                o.id
            ));
        }
    }

    // Regras
    let total = r.zonas.len() + r.linhas.len() + r.trajetorias.len() + r.contagens.len();
    if total > MAX_REGRAS {
        e.push(format!("regras: no máximo {MAX_REGRAS} no total"));
    }
    let mut ids_regra = HashSet::new();
    let mut regra = |e: &mut Vec<String>, onde: String, id: &str, nome: &str, objs: &[String]| {
        if !id_ok(id, 1, 40) {
            e.push(format!("{onde}.id: use letras minúsculas, números, _ ou -"));
        } else if !ids_regra.insert(id.to_string()) {
            e.push(format!("{onde}.id: \"{id}\" repetido entre as regras"));
        }
        texto(e, &format!("{onde}.nome"), nome, 1, 60);
        if objs.is_empty() {
            e.push(format!(
                "{onde}.objetos: diga quais objetos a regra observa"
            ));
        }
        for o in objs {
            if !ids_obj.contains(o.as_str()) {
                e.push(format!(
                    "{onde}.objetos: \"{o}\" não está na lista de objetos"
                ));
            }
        }
    };
    for (i, z) in r.zonas.iter().enumerate() {
        let onde = format!("zonas[{i}]");
        regra(&mut e, onde.clone(), &z.id, &z.nome, &z.objetos);
        pontos(&mut e, &onde, &z.pontos, 3, MAX_PONTOS);
        if z.pontos.len() >= 3 && area(&z.pontos) < 0.0005 {
            e.push(format!(
                "{onde}.pontos: a zona está achatada (área quase zero)"
            ));
        }
        if let Some(s) = z.permanencia_s {
            if !(1..=86_400).contains(&s) {
                e.push(format!("{onde}.permanencia_s: use de 1 a 86400 segundos"));
            }
        }
    }
    for (i, l) in r.linhas.iter().enumerate() {
        let onde = format!("linhas[{i}]");
        regra(&mut e, onde.clone(), &l.id, &l.nome, &l.objetos);
        pontos(&mut e, &onde, &l.pontos, 2, 2);
        if l.pontos.len() == 2 && dist(l.pontos[0], l.pontos[1]) < 0.02 {
            e.push(format!("{onde}.pontos: a linha é curta demais"));
        }
    }
    for (i, t) in r.trajetorias.iter().enumerate() {
        let onde = format!("trajetorias[{i}]");
        regra(&mut e, onde.clone(), &t.id, &t.nome, &t.objetos);
        pontos(&mut e, &onde, &t.pontos, 2, MAX_PONTOS);
        if !(0.01..=0.5).contains(&t.tolerancia) {
            e.push(format!("{onde}.tolerancia: use de 0,01 a 0,5"));
        }
    }
    let zonas: HashSet<&str> = r.zonas.iter().map(|z| z.id.as_str()).collect();
    let linhas: HashSet<&str> = r.linhas.iter().map(|l| l.id.as_str()).collect();
    for (i, c) in r.contagens.iter().enumerate() {
        let onde = format!("contagens[{i}]");
        regra(&mut e, onde.clone(), &c.id, &c.nome, &c.objetos);
        match (&c.linha, &c.zona) {
            (Some(l), None) if !linhas.contains(l.as_str()) => {
                e.push(format!("{onde}.linha: não existe linha \"{l}\""))
            }
            (None, Some(z)) if !zonas.contains(z.as_str()) => {
                e.push(format!("{onde}.zona: não existe zona \"{z}\""))
            }
            (Some(_), None) | (None, Some(_)) => {}
            _ => e.push(format!(
                "{onde}: informe uma linha ou uma zona para contar (só uma das duas)"
            )),
        }
    }

    // Estudo
    let s = &r.estudo;
    texto(&mut e, "estudo.autor", &s.autor, 0, 40);
    if s.autor.contains('@') {
        e.push("estudo.autor: use um apelido, não um e-mail".into());
    }
    texto(&mut e, "estudo.local", &s.local, 0, 80);
    texto(&mut e, "estudo.notas", &s.notas, 0, 2000);
    if s.fotos > 100_000 {
        e.push("estudo.fotos: número de fotos fora do esperado".into());
    }
    if s.epocas > 300 {
        e.push("estudo.epocas: no máximo 300 (o Colab gratuito não aguenta mais)".into());
    }
    if let Some(m) = &s.metricas {
        for (nome, v) in [
            ("map50", m.map50),
            ("map50_95", m.map50_95),
            ("precisao", m.precisao),
            ("revocacao", m.revocacao),
        ] {
            if let Some(v) = v {
                if !(0.0..=1.0).contains(&v) {
                    e.push(format!("estudo.metricas.{nome}: use de 0 a 1"));
                }
            }
        }
    }

    // Bloqueio: reconhecimento facial e biometria (LGPD art. 11).
    if let Some((campo, termo)) = proibido_na_receita(r) {
        e.push(format!(
            "{campo}: menciona \"{termo}\". Reconhecimento facial, biometria e inferência de dados sensíveis (LGPD art. 11) são bloqueados no modo estúdio"
        ));
    }

    if e.is_empty() {
        Ok(())
    } else {
        Err(e)
    }
}

fn texto(e: &mut Vec<String>, campo: &str, v: &str, min: usize, max: usize) {
    let n = v.chars().count();
    if n < min || n > max {
        if min == 0 {
            e.push(format!("{campo}: no máximo {max} caracteres"));
        } else {
            e.push(format!("{campo}: use de {min} a {max} caracteres"));
        }
    }
    if v.chars().any(|c| c.is_control() && c != '\n') {
        e.push(format!("{campo}: tem caracteres de controle"));
    }
    if v.split_whitespace()
        .any(|p| p.chars().count() > MAX_PALAVRA)
        || v.to_ascii_lowercase().contains("data:image")
    {
        e.push(format!(
            "{campo}: parece um arquivo embutido; a receita não leva fotos"
        ));
    }
}

fn pontos(e: &mut Vec<String>, onde: &str, p: &[Ponto], min: usize, max: usize) {
    if p.len() < min || p.len() > max {
        if min == max {
            e.push(format!("{onde}.pontos: precisa de exatamente {min} pontos"));
        } else {
            e.push(format!("{onde}.pontos: use de {min} a {max} pontos"));
        }
    }
    if p.iter()
        .any(|q| !(0.0..=1.0).contains(&q.x) || !(0.0..=1.0).contains(&q.y))
    {
        e.push(format!(
            "{onde}.pontos: coordenadas vão de 0 a 1 (fração da imagem)"
        ));
    }
}

fn area(p: &[Ponto]) -> f32 {
    let n = p.len();
    let s: f32 = (0..n)
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % n]);
            a.x * b.y - b.x * a.y
        })
        .sum();
    (s / 2.0).abs()
}

fn dist(a: Ponto, b: Ponto) -> f32 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
}

// ---------------------------------------------------------------------------
// Bloqueio de biometria

/// Termos bloqueados, já sem acento e em minúsculas. `*` no fim = prefixo.
const PROIBIDOS: &[&[&str]] = &[
    &["rosto*"],
    &["face"],
    &["faces"],
    &["facia*"],
    &["biometr*"],
    &["iris"],
    &["retina*"],
    &["impressao", "digital"],
    &["impressoes", "digitais"],
    &["fingerprint*"],
    &["palmar"],
    &["reconhecimento", "de", "voz"],
    &["emoca*"],
    &["emocoes"],
    &["emotion*"],
    &["etni*"],
    &["ethnic*"],
    &["raca"],
    &["racial"],
    &["cor", "da", "pele"],
    &["genero"],
    &["gender"],
    &["sexo"],
    &["gait"],
    &["identificar", "pessoa*"],
    &["reconhecer", "pessoa*"],
    &["reconhecimento", "de", "pessoa*"],
    &["identificacao", "de", "pessoa*"],
    &["cpf"],
];

fn sem_acento(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        c => c,
    }
}

fn palavras(t: &str) -> Vec<String> {
    t.to_lowercase()
        .chars()
        .map(sem_acento)
        .collect::<String>()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect()
}

fn casa(palavra: &str, termo: &str) -> bool {
    match termo.strip_suffix('*') {
        Some(prefixo) => palavra.starts_with(prefixo),
        None => palavra == termo,
    }
}

/// Primeiro termo bloqueado encontrado no texto (para a mensagem).
pub fn termo_proibido(t: &str) -> Option<String> {
    let ps = palavras(t);
    for termo in PROIBIDOS {
        if ps.len() < termo.len() {
            continue;
        }
        for janela in ps.windows(termo.len()) {
            if janela.iter().zip(termo.iter()).all(|(p, t)| casa(p, t)) {
                return Some(janela.join(" "));
            }
        }
    }
    None
}

fn proibido_na_receita(r: &Receita) -> Option<(String, String)> {
    let mut campos: Vec<(String, &str)> = vec![
        ("id".into(), &r.id),
        ("nome".into(), &r.nome),
        ("descricao".into(), &r.descricao),
        ("estudo.local".into(), &r.estudo.local),
        ("estudo.notas".into(), &r.estudo.notas),
    ];
    for (i, o) in r.objetos.iter().enumerate() {
        campos.push((format!("objetos[{i}].id"), &o.id));
        campos.push((format!("objetos[{i}].nome"), &o.nome));
        campos.push((format!("objetos[{i}].descricao"), &o.descricao));
    }
    for (i, c) in r.modelo.classes.iter().enumerate() {
        campos.push((format!("modelo.classes[{i}]"), c));
    }
    let regras = r
        .zonas
        .iter()
        .map(|z| (&z.id, &z.nome))
        .chain(r.linhas.iter().map(|l| (&l.id, &l.nome)))
        .chain(r.trajetorias.iter().map(|t| (&t.id, &t.nome)))
        .chain(r.contagens.iter().map(|c| (&c.id, &c.nome)));
    for (id, nome) in regras {
        campos.push((format!("regra \"{id}\""), id));
        campos.push((format!("regra \"{id}\""), nome));
    }
    campos
        .into_iter()
        .find_map(|(c, t)| termo_proibido(t).map(|p| (c, p)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn erros(r: &Receita) -> Vec<String> {
        validar(r).err().unwrap_or_default()
    }

    #[test]
    fn exemplo_e_valido_e_volta_igual() {
        let r = exemplo();
        assert_eq!(validar(&r), Ok(()));
        let lida = ler(&r.canonico()).unwrap();
        assert_eq!(lida, r);
        assert_eq!(lida.sha256(), r.sha256());
        assert_eq!(r.sha256().len(), 64);
        assert_eq!(r.nomes_classes(), vec!["caixa de banana"]);
        assert_eq!(r.epocas(), EPOCAS_PADRAO);
    }

    #[test]
    fn padroes_preenchidos_ao_ler() {
        let v = json!({
            "formato": FORMATO, "versao": 1, "id": "contar-pallets", "nome": "Contar pallets",
            "somente_estudo": true, "aviso": AVISO_ESTUDO,
            "modelo": {"base": "yolo11n", "classes": ["pallet"]},
            "objetos": [{"id": "pallet", "nome": "pallet"}],
            "estudo": {}
        });
        let r = de_valor(v).unwrap();
        assert_eq!(r.modelo.lado, 640);
        assert_eq!(r.modelo.confianca, 0.35);
        assert!(r.zonas.is_empty() && r.contagens.is_empty());
    }

    #[test]
    fn campo_desconhecido_como_fotos_e_recusado_em_portugues() {
        let mut v: Value = serde_json::from_str(&exemplo().canonico()).unwrap();
        v["fotos"] = json!(["base64..."]);
        let e = de_valor(v).unwrap_err();
        assert!(e[0].contains("campo desconhecido `fotos`"), "{e:?}");
        let e = ler("{\"formato\": \"camfacil-receita\"").unwrap_err();
        assert!(e[0].contains("JSON incompleto"), "{e:?}");
        let mut v: Value = serde_json::from_str(&exemplo().canonico()).unwrap();
        v.as_object_mut().unwrap().remove("nome");
        assert!(de_valor(v).unwrap_err()[0].contains("falta o campo `nome`"));
    }

    #[test]
    fn tamanho_e_arquivo_embutido() {
        let mut r = exemplo();
        r.descricao = "x".repeat(TAMANHO_MAX);
        assert!(ler(&r.canonico()).unwrap_err()[0].contains("KB"));
        let mut r = exemplo();
        r.estudo.notas = format!("veja {}", "A".repeat(500));
        assert!(erros(&r).iter().any(|m| m.contains("arquivo embutido")));
        let mut r = exemplo();
        r.estudo.notas = "data:image/jpeg;base64,abc".into();
        assert!(erros(&r).iter().any(|m| m.contains("arquivo embutido")));
    }

    #[test]
    fn aviso_e_somente_estudo_obrigatorios() {
        let mut r = exemplo();
        r.somente_estudo = false;
        r.aviso = "pode usar em produção".into();
        let e = erros(&r);
        assert!(e.iter().any(|m| m.starts_with("somente_estudo")));
        assert!(e.iter().any(|m| m.starts_with("aviso")));
    }

    #[test]
    fn classes_precisam_casar_com_objetos() {
        let mut r = exemplo();
        r.modelo.classes = vec![
            "caixa_banana".into(),
            "caixa_banana".into(),
            "pallet".into(),
        ];
        let e = erros(&r);
        assert!(e.iter().any(|m| m.contains("repetida")));
        assert!(e.iter().any(|m| m.contains("\"pallet\" não está")));
        let mut r = exemplo();
        r.modelo.classes.clear();
        assert!(erros(&r).iter().any(|m| m.contains("falta o objeto")));
        let mut r = exemplo();
        r.objetos = (0..21)
            .map(|i| Objeto {
                id: format!("o{i}"),
                nome: format!("objeto {i}"),
                descricao: String::new(),
            })
            .collect();
        r.modelo.classes = r.objetos.iter().map(|o| o.id.clone()).collect();
        r.zonas.clear();
        r.linhas.clear();
        r.contagens.clear();
        assert!(erros(&r).iter().any(|m| m.contains("no máximo 20")));
    }

    #[test]
    fn regras_geometria_e_referencias() {
        let mut r = exemplo();
        r.zonas[0].pontos.truncate(2);
        r.linhas[0].pontos[1] = Ponto { x: 1.5, y: 0.2 };
        r.contagens[0].linha = Some("janela".into());
        r.contagens.push(Contagem {
            id: "dupla".into(),
            nome: "Dupla".into(),
            objetos: vec!["caixa_banana".into()],
            linha: Some("porta".into()),
            zona: Some("estoque".into()),
            janela: Janela::Hora,
        });
        r.trajetorias.push(Trajetoria {
            id: "porta".into(),
            nome: "Caminho".into(),
            objetos: vec!["empilhadeira".into()],
            pontos: vec![Ponto { x: 0.1, y: 0.1 }, Ponto { x: 0.9, y: 0.9 }],
            tolerancia: 0.9,
        });
        let e = erros(&r);
        let tem = |s: &str| e.iter().any(|m| m.contains(s));
        assert!(tem("zonas[0].pontos: use de 3 a 32"), "{e:?}");
        assert!(tem("linhas[0].pontos: coordenadas vão de 0 a 1"));
        assert!(tem("não existe linha \"janela\""));
        assert!(tem("contagens[1]: informe uma linha ou uma zona"));
        assert!(tem("trajetorias[0].id: \"porta\" repetido"));
        assert!(tem("\"empilhadeira\" não está na lista"));
        assert!(tem("trajetorias[0].tolerancia"));
        let mut r = exemplo();
        r.zonas[0].pontos = vec![
            Ponto { x: 0.1, y: 0.1 },
            Ponto { x: 0.2, y: 0.2 },
            Ponto { x: 0.3, y: 0.3 },
        ];
        assert!(erros(&r).iter().any(|m| m.contains("achatada")));
    }

    #[test]
    fn ids_e_metadados() {
        let mut r = exemplo();
        r.id = "Contar Bananas".into();
        r.estudo.autor = "fulano@empresa.com".into();
        r.estudo.epocas = 1000;
        r.estudo.metricas = Some(Metricas {
            map50: Some(1.2),
            ..Default::default()
        });
        r.modelo.lado = 650;
        let e = erros(&r);
        assert!(e.iter().any(|m| m.starts_with("id:")));
        assert!(e.iter().any(|m| m.contains("não um e-mail")));
        assert!(e.iter().any(|m| m.starts_with("estudo.epocas")));
        assert!(e.iter().any(|m| m.starts_with("estudo.metricas.map50")));
        assert!(e.iter().any(|m| m.starts_with("modelo.lado")));
    }

    #[test]
    fn bloqueia_reconhecimento_facial_e_biometria() {
        for (campo, texto) in [
            ("nome", "Reconhecimento facial na portaria"),
            ("descricao", "Identificar pessoas pelo rosto"),
            ("descricao", "Ler a ÍRIS do funcionário"),
            ("descricao", "controle por impressão digital"),
            ("descricao", "detectar emoções dos clientes"),
            ("descricao", "classificar por etnia"),
            ("nome", "Face ID da loja"),
            ("descricao", "biometria de acesso"),
        ] {
            let mut r = exemplo();
            match campo {
                "nome" => r.nome = texto.into(),
                _ => r.descricao = texto.into(),
            }
            let e = erros(&r);
            assert!(
                e.iter().any(|m| m.contains("LGPD art. 11")),
                "não bloqueou {texto:?}: {e:?}"
            );
        }
        // Classe com nome de rosto, mesmo escondida no id do objeto.
        let mut r = exemplo();
        r.objetos[0].id = "rosto_cliente".into();
        r.modelo.classes = vec!["rosto_cliente".into()];
        for z in &mut r.zonas {
            z.objetos = vec!["rosto_cliente".into()];
        }
        for l in &mut r.linhas {
            l.objetos = vec!["rosto_cliente".into()];
        }
        for c in &mut r.contagens {
            c.objetos = vec!["rosto_cliente".into()];
        }
        let e = erros(&r);
        assert_eq!(e.len(), 1, "{e:?}");
        assert!(e[0].starts_with("objetos[0].id: menciona \"rosto\""));
    }

    #[test]
    fn nao_bloqueia_o_que_nao_e_biometria() {
        for t in [
            "Contar pessoas na fila",
            "Carro na vaga de carga e descarga",
            "Empilhadeira na faixa de pedestre",
            "Moto na calçada da loja",
            "Caixa caramelo",
            "Prateleira vazia no corredor",
        ] {
            assert_eq!(termo_proibido(t), None, "{t}");
        }
    }

    #[test]
    fn esquema_documenta_os_mesmos_campos() {
        let esquema: Value = serde_json::from_str(ESQUEMA).expect("esquema é JSON válido");
        let props = esquema["properties"].as_object().unwrap();
        let receita: Value = serde_json::from_str(&exemplo().canonico()).unwrap();
        let campos = receita.as_object().unwrap();
        let mut a: Vec<&String> = props.keys().collect();
        let mut b: Vec<&String> = campos.keys().collect();
        a.sort();
        b.sort();
        assert_eq!(a, b, "esquema e struct Receita divergem");
        assert_eq!(esquema["properties"]["aviso"]["const"], AVISO_ESTUDO);
        assert_eq!(esquema["properties"]["formato"]["const"], FORMATO);
        for sub in ["modelo", "estudo"] {
            let mut a: Vec<&String> = esquema["$defs"][sub]["properties"]
                .as_object()
                .unwrap()
                .keys()
                .collect();
            let mut b: Vec<&String> = receita[sub].as_object().unwrap().keys().collect();
            a.sort();
            b.sort();
            assert_eq!(a, b, "esquema de {sub} diverge");
        }
    }
}
