//! Gerador do notebook do Google Colab do modo estúdio (SDD-07).
//!
//! A partir de uma receita válida, monta um `.ipynb` (JSON, nbformat 4) que:
//! monta o Google Drive do técnico, recebe as fotos rotuladas no formato YOLO,
//! treina um YOLO nano (Ultralytics) salvando os checkpoints no Drive (a sessão
//! gratuita do Colab cai; o treino continua de onde parou), exporta ONNX e
//! grava um manifesto com o SHA-256 do modelo. Python só no Colab: nada disso
//! roda na máquina do cliente.

use crate::receita::{self, Receita, AVISO_ESTUDO};
use serde_json::{json, Value};

/// Faixa do Ultralytics aceita (YOLO11 entrou na 8.3).
pub const ULTRALYTICS: &str = "ultralytics>=8.3.0,<9";
/// Pasta do estudo dentro do "Meu Drive".
pub const PASTA_DRIVE: &str = "/content/drive/MyDrive/CamFacil-IA/estudio/";
/// Mínimo de fotos rotuladas para treinar.
pub const MIN_FOTOS: u32 = 20;

/// Nome sugerido para o arquivo baixado.
pub fn nome_arquivo(r: &Receita) -> String {
    format!("camfacil-estudio-{}.ipynb", r.id)
}

/// Gera o notebook (JSON formatado). Valida a receita antes.
pub fn gerar(r: &Receita) -> Result<String, Vec<String>> {
    receita::validar(r)?;
    let nb = notebook(r);
    Ok(serde_json::to_string_pretty(&nb).unwrap_or_default())
}

/// Literal de texto Python: um texto JSON também é um literal válido em Python 3.
fn py(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

fn linhas(fonte: &str) -> Value {
    let partes: Vec<&str> = fonte.split_inclusive('\n').collect();
    let mut v: Vec<String> = partes.iter().map(|s| s.to_string()).collect();
    if let Some(u) = v.last_mut() {
        if u.ends_with('\n') {
            u.pop();
        }
    }
    json!(v)
}

fn md(fonte: &str) -> Value {
    json!({"cell_type": "markdown", "metadata": {}, "source": linhas(fonte)})
}

fn codigo(fonte: &str) -> Value {
    json!({
        "cell_type": "code",
        "execution_count": null,
        "metadata": {},
        "outputs": [],
        "source": linhas(fonte),
    })
}

fn notebook(r: &Receita) -> Value {
    let canonico = r.canonico();
    let classes: Vec<String> = r
        .modelo
        .classes
        .iter()
        .enumerate()
        .zip(r.nomes_classes())
        .map(|((i, id), nome)| format!("| {i} | `{id}` | {nome} |"))
        .collect();

    let topo = format!(
        r#"# CamFácil IA · Modo estúdio · {nome}

> **Somente para estudo e aprendizagem.** {aviso}
> Reconhecimento facial e biometria são bloqueados no modo estúdio.

Receita `{id}` · modelo base **{base}** · imagens de {lado} px · {epocas} épocas

| Classe | Id | Nome |
|---|---|---|
{classes}

**Como usar**
1. Ative a GPU: *Ambiente de execução → Alterar o tipo de ambiente de execução → GPU T4*.
2. Rode as células **na ordem**. Se a sessão cair, rode todas de novo: o treino continua de onde parou.
3. As fotos ficam no **seu** Google Drive. Nada passa pelo servidor do CamFácil IA.
4. Use só fotos de clientes que assinaram a autorização (veja `docs/legal/` no repositório)."#,
        nome = r.nome,
        aviso = AVISO_ESTUDO,
        id = r.id,
        base = r.modelo.base.pesos().trim_end_matches(".pt"),
        lado = r.modelo.lado,
        epocas = r.epocas(),
        classes = classes.join("\n"),
    );

    let config = format!(
        r#"# Somente para estudo e aprendizagem. Não use este modelo em produção.
AVISO = {aviso}
RECEITA_JSON = {receita}
RECEITA_SHA256 = {sha}
EPOCAS = {epocas}
LADO = {lado}
BASE = {base}
MIN_FOTOS = {min}

import hashlib
import json

RECEITA = json.loads(RECEITA_JSON)
assert hashlib.sha256(RECEITA_JSON.encode("utf-8")).hexdigest() == RECEITA_SHA256, "A receita foi alterada: gere o notebook de novo no CamFácil IA."
CLASSES = RECEITA["modelo"]["classes"]
print(AVISO)
print("Receita:", RECEITA["nome"], "| classes:", ", ".join(CLASSES))"#,
        aviso = py(AVISO_ESTUDO),
        receita = py(&canonico),
        sha = py(&r.sha256()),
        epocas = r.epocas(),
        lado = r.modelo.lado,
        base = py(r.modelo.base.pesos()),
        min = MIN_FOTOS,
    );

    let gpu = r#"import subprocess

r = subprocess.run(["nvidia-smi", "-L"], capture_output=True, text=True)
if r.returncode != 0:
    print("Sem GPU. Vá em Ambiente de execução > Alterar o tipo de ambiente de execução > GPU (T4) e rode de novo.")
else:
    print(r.stdout)"#;

    let drive = format!(
        r#"import os
from google.colab import drive

drive.mount("/content/drive")
PASTA = {pasta} + RECEITA["id"]
for sub in ("fotos", "treino", "exportado"):
    os.makedirs(os.path.join(PASTA, sub), exist_ok=True)
with open(os.path.join(PASTA, "receita.json"), "w", encoding="utf-8") as f:
    f.write(RECEITA_JSON)
print("Pasta do estudo no seu Drive:", PASTA)"#,
        pasta = py(PASTA_DRIVE),
    );

    let como_fotos = format!(
        r#"## Fotos rotuladas (formato YOLO)

Coloque um ou mais `.zip` em `Meu Drive/CamFacil-IA/estudio/{id}/fotos/` (ou envie quando a próxima célula pedir). Dentro do zip:

- as fotos (`.jpg`, `.png`…), por exemplo em `images/`;
- um `.txt` com o **mesmo nome** de cada foto, por exemplo em `labels/`, com uma linha por objeto: `classe x_centro y_centro largura altura` (valores de 0 a 1).

A classe é o número da tabela do topo. Junte de 100 a 300 fotos em horários diferentes (mínimo {min})."#,
        id = r.id,
        min = MIN_FOTOS,
    );

    let fotos = r#"import glob
import random
import shutil
import zipfile

DADOS = "/content/dados"
shutil.rmtree(DADOS, ignore_errors=True)
os.makedirs(DADOS)
zips = sorted(glob.glob(os.path.join(PASTA, "fotos", "*.zip")))
if not zips:
    from google.colab import files
    print("Nenhum .zip em", os.path.join(PASTA, "fotos"), "- envie agora o arquivo com as fotos e os rótulos.")
    for nome, conteudo in files.upload().items():
        destino = os.path.join(PASTA, "fotos", os.path.basename(nome))
        with open(destino, "wb") as f:
            f.write(conteudo)
        zips.append(destino)
for z in zips:
    with zipfile.ZipFile(z) as arq:
        arq.extractall(os.path.join(DADOS, "bruto"))

EXT = (".jpg", ".jpeg", ".png", ".bmp", ".webp")
imagens, rotulos = {}, {}
for raiz, _, nomes in os.walk(os.path.join(DADOS, "bruto")):
    for n in nomes:
        base, ext = os.path.splitext(n)
        if ext.lower() in EXT:
            imagens[base] = os.path.join(raiz, n)
        elif ext.lower() == ".txt" and n != "classes.txt":
            rotulos[base] = os.path.join(raiz, n)
pares = sorted(b for b in imagens if b in rotulos)
problemas = []
for b in pares:
    with open(rotulos[b], encoding="utf-8") as f:
        for i, linha in enumerate(f, 1):
            partes = linha.split()
            if not partes:
                continue
            if len(partes) != 5:
                problemas.append(f"{b}.txt linha {i}: esperado 5 valores (classe x y largura altura)")
                continue
            try:
                c = int(partes[0])
                v = [float(x) for x in partes[1:]]
            except ValueError:
                problemas.append(f"{b}.txt linha {i}: valores não numéricos")
                continue
            if not 0 <= c < len(CLASSES):
                problemas.append(f"{b}.txt linha {i}: classe {c} não existe (use de 0 a {len(CLASSES) - 1})")
            if any(x < 0 or x > 1 for x in v):
                problemas.append(f"{b}.txt linha {i}: coordenadas fora de 0 a 1")
if problemas:
    print("\n".join(problemas[:20]))
    raise SystemExit("Corrija os rótulos acima e rode esta célula de novo.")
if len(pares) < MIN_FOTOS:
    raise SystemExit(f"Só {len(pares)} fotos rotuladas. Junte pelo menos {MIN_FOTOS} (o ideal é de 100 a 300).")
random.Random(0).shuffle(pares)
corte = max(1, int(len(pares) * 0.2))
grupos = {"val": pares[:corte], "train": pares[corte:]}
for grupo, lista in grupos.items():
    for sub in ("images", "labels"):
        os.makedirs(os.path.join(DADOS, sub, grupo), exist_ok=True)
    for b in lista:
        ext = os.path.splitext(imagens[b])[1].lower()
        shutil.copy(imagens[b], os.path.join(DADOS, "images", grupo, b + ext))
        shutil.copy(rotulos[b], os.path.join(DADOS, "labels", grupo, b + ".txt"))
YAML = os.path.join(DADOS, "dados.yaml")
with open(YAML, "w", encoding="utf-8") as f:
    f.write("path: " + json.dumps(DADOS) + "\n")
    f.write("train: images/train\nval: images/val\n")
    f.write("names: " + json.dumps(CLASSES, ensure_ascii=False) + "\n")
FOTOS = len(pares)
print(f"{FOTOS} fotos rotuladas ({len(grupos['train'])} treino, {len(grupos['val'])} validação); {len(imagens) - FOTOS} sem rótulo ignoradas.")"#;

    let instalar = format!("!pip install -q \"{ULTRALYTICS}\"");

    let treino = r#"import ultralytics
from ultralytics import YOLO

TREINO = os.path.join(PASTA, "treino")
ULTIMO = os.path.join(TREINO, "execucao", "weights", "last.pt")
if os.path.exists(ULTIMO):
    print("Continuando de onde parou:", ULTIMO)
    try:
        YOLO(ULTIMO).train(resume=True)
    except Exception as e:
        if "nothing to resume" not in str(e):
            raise
        print("O treino já tinha terminado.")
else:
    # Checkpoints vão direto para o Drive (last.pt a cada época, cópia a cada 5).
    YOLO(BASE).train(data=YAML, epochs=EPOCAS, imgsz=LADO, batch=16, patience=20, seed=0,
                     project=TREINO, name="execucao", exist_ok=True, save_period=5)"#;

    let validar = r#"MELHOR = os.path.join(TREINO, "execucao", "weights", "best.pt")
final = YOLO(MELHOR)
m = final.val(data=YAML, imgsz=LADO, plots=False)
METRICAS = {
    "map50": round(float(m.box.map50), 4),
    "map50_95": round(float(m.box.map), 4),
    "precisao": round(float(m.box.mp), 4),
    "revocacao": round(float(m.box.mr), 4),
}
print("Métricas na validação:", METRICAS)"#;

    let exportar = r#"import datetime

onnx_tmp = final.export(format="onnx", imgsz=LADO, opset=12, simplify=False, dynamic=False)
DESTINO = os.path.join(PASTA, "exportado", RECEITA["id"] + ".onnx")
shutil.copy(onnx_tmp, DESTINO)

def sha256_arquivo(caminho):
    h = hashlib.sha256()
    with open(caminho, "rb") as f:
        for bloco in iter(lambda: f.read(1 << 20), b""):
            h.update(bloco)
    return h.hexdigest()

receita_final = json.loads(RECEITA_JSON)
receita_final["estudo"]["fotos"] = FOTOS
receita_final["estudo"]["epocas"] = EPOCAS
receita_final["estudo"]["metricas"] = METRICAS
with open(os.path.join(PASTA, "exportado", "receita.json"), "w", encoding="utf-8") as f:
    json.dump(receita_final, f, ensure_ascii=False, indent=2)

MANIFESTO = {
    "formato": "camfacil-manifesto",
    "versao": 1,
    "somente_estudo": True,
    "aviso": AVISO,
    "receita_id": RECEITA["id"],
    "receita_sha256": RECEITA_SHA256,
    "onnx": os.path.basename(DESTINO),
    "onnx_sha256": sha256_arquivo(DESTINO),
    "onnx_bytes": os.path.getsize(DESTINO),
    "classes": CLASSES,
    "lado": LADO,
    "base": RECEITA["modelo"]["base"],
    "metricas": METRICAS,
    "fotos": FOTOS,
    "epocas": EPOCAS,
    "ultralytics": ultralytics.__version__,
    "gerado_em": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
}
with open(os.path.join(PASTA, "exportado", "manifesto.json"), "w", encoding="utf-8") as f:
    json.dump(MANIFESTO, f, ensure_ascii=False, indent=2)
print(AVISO)
print("Pronto. Arquivos em", os.path.join(PASTA, "exportado"))
print("SHA-256 do modelo:", MANIFESTO["onnx_sha256"])"#;

    let depois = format!(
        r#"## Depois do treino

Na pasta `Meu Drive/CamFacil-IA/estudio/{id}/exportado/` ficam três arquivos:

- `{id}.onnx`: o modelo de estudo;
- `receita.json`: a receita com as métricas do treino;
- `manifesto.json`: classes, métricas e o SHA-256 do modelo.

**Importar no CamFácil IA:** o programa local confere o SHA-256 do manifesto e guarda o modelo marcado como *modelo de estudo*, desligado.

**Compartilhar com outros técnicos:** publique só a `receita.json` no site do CamFácil IA. Fotos de clientes **nunca** são compartilhadas.

> {aviso}"#,
        id = r.id,
        aviso = AVISO_ESTUDO,
    );

    let apagar = r#"# Apagar tudo deste estudo (fotos, rótulos, treino e modelo) do seu Drive.
APAGAR_TUDO = False  # mude para True e rode esta célula
if APAGAR_TUDO:
    shutil.rmtree(PASTA, ignore_errors=True)
    shutil.rmtree("/content/dados", ignore_errors=True)
    print("Apagado:", PASTA)
else:
    print("Nada foi apagado. Para apagar o estudo, mude APAGAR_TUDO para True.")"#;

    json!({
        "nbformat": 4,
        "nbformat_minor": 0,
        "metadata": {
            "colab": {"name": nome_arquivo(r), "provenance": []},
            "kernelspec": {"name": "python3", "display_name": "Python 3"},
            "language_info": {"name": "python"},
            "accelerator": "GPU",
            "camfacil": {
                "somente_estudo": true,
                "aviso": AVISO_ESTUDO,
                "receita_id": r.id,
                "receita_sha256": r.sha256(),
                "gerador": format!("cf-core {}", crate::VERSION),
            },
        },
        "cells": [
            md(&topo),
            codigo(&config),
            md("## 1. Conferir a GPU"),
            codigo(gpu),
            md("## 2. Montar o seu Google Drive"),
            codigo(&drive),
            md(&como_fotos),
            codigo(fotos),
            md("## 3. Instalar o Ultralytics"),
            codigo(&instalar),
            md("## 4. Treinar (salva no Drive; se cair, rode de novo e ele continua)"),
            codigo(treino),
            md("## 5. Medir o resultado"),
            codigo(validar),
            md("## 6. Exportar ONNX e manifesto"),
            codigo(exportar),
            md(&depois),
            md("## Apagar tudo"),
            codigo(apagar),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::receita::exemplo;

    fn gerado() -> Value {
        serde_json::from_str(&gerar(&exemplo()).unwrap()).unwrap()
    }

    fn fonte(c: &Value) -> String {
        c["source"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect()
    }

    fn codigos(nb: &Value) -> Vec<String> {
        nb["cells"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["cell_type"] == "code")
            .map(fonte)
            .collect()
    }

    #[test]
    fn estrutura_nbformat_4() {
        let nb = gerado();
        assert_eq!(nb["nbformat"], 4);
        assert_eq!(nb["metadata"]["kernelspec"]["name"], "python3");
        assert_eq!(nb["metadata"]["accelerator"], "GPU");
        let cells = nb["cells"].as_array().unwrap();
        assert!(cells.len() >= 10);
        for c in cells {
            let tipo = c["cell_type"].as_str().unwrap();
            assert!(tipo == "markdown" || tipo == "code");
            assert!(c["metadata"].is_object());
            let src = c["source"].as_array().unwrap();
            assert!(!src.is_empty());
            // Toda linha termina com \n, menos a última (formato do Jupyter).
            for (i, l) in src.iter().enumerate() {
                let l = l.as_str().unwrap();
                assert_eq!(l.ends_with('\n'), i + 1 < src.len(), "{l:?}");
            }
            if tipo == "code" {
                assert!(c["outputs"].as_array().unwrap().is_empty());
                assert!(c["execution_count"].is_null());
            }
        }
    }

    #[test]
    fn aviso_de_estudo_no_topo_e_nos_metadados() {
        let nb = gerado();
        let primeira = &nb["cells"][0];
        assert_eq!(primeira["cell_type"], "markdown");
        let t = fonte(primeira);
        assert!(t.contains("Somente para estudo e aprendizagem"));
        assert!(t.contains(AVISO_ESTUDO));
        assert!(t.contains("| 0 | `caixa_banana` | caixa de banana |"));
        assert_eq!(nb["metadata"]["camfacil"]["somente_estudo"], true);
        assert!(fonte(&nb["cells"][1]).starts_with("# Somente para estudo"));
    }

    #[test]
    fn passos_drive_treino_onnx_e_manifesto() {
        let todo = codigos(&gerado()).join("\n");
        for trecho in [
            "drive.mount(\"/content/drive\")",
            "/content/drive/MyDrive/CamFacil-IA/estudio/",
            "zipfile.ZipFile",
            "!pip install -q \"ultralytics>=8.3.0,<9\"",
            "BASE = \"yolo11n.pt\"",
            "resume=True",
            "save_period=5",
            "project=TREINO",
            "format=\"onnx\"",
            "\"onnx_sha256\": sha256_arquivo(DESTINO)",
            "manifesto.json",
            "APAGAR_TUDO = False",
            "EPOCAS = 60",
            "LADO = 640",
        ] {
            assert!(todo.contains(trecho), "faltou {trecho}");
        }
    }

    #[test]
    fn receita_embutida_confere_com_hash() {
        let r = exemplo();
        let cfg = &codigos(&gerado())[0];
        let linha = cfg
            .lines()
            .find(|l| l.starts_with("RECEITA_JSON = "))
            .unwrap();
        // O literal Python é um texto JSON: decodifica e volta à mesma receita.
        let texto: String = serde_json::from_str(&linha["RECEITA_JSON = ".len()..]).unwrap();
        assert_eq!(texto, r.canonico());
        assert_eq!(crate::receita::ler(&texto).unwrap(), r);
        assert!(cfg.contains(&format!("RECEITA_SHA256 = \"{}\"", r.sha256())));
    }

    #[test]
    fn receita_invalida_nao_gera() {
        let mut r = exemplo();
        r.nome = "Reconhecimento facial".into();
        assert!(gerar(&r).is_err());
    }

    #[test]
    fn aspas_e_barras_no_nome_nao_quebram_o_python() {
        let mut r = exemplo();
        r.nome = "Caixa \"grande\" \\ teste ''' fim".into();
        let nb: Value = serde_json::from_str(&gerar(&r).unwrap()).unwrap();
        let cfg = &codigos(&nb)[0];
        let linha = cfg
            .lines()
            .find(|l| l.starts_with("RECEITA_JSON = "))
            .unwrap();
        let texto: String = serde_json::from_str(&linha["RECEITA_JSON = ".len()..]).unwrap();
        assert_eq!(crate::receita::ler(&texto).unwrap().nome, r.nome);
    }

    /// Confere a sintaxe do Python com `python3`, se existir na máquina de
    /// desenvolvimento (não é dependência do projeto; sem ele, o teste passa).
    #[test]
    fn python_das_celulas_compila() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut fonte = String::new();
        for c in codigos(&gerado()) {
            for l in c.lines() {
                // Comandos de shell do Colab (`!pip`) não são Python.
                if !l.trim_start().starts_with('!') {
                    fonte.push_str(l);
                }
                fonte.push('\n');
            }
        }
        let filho = Command::new("python3")
            .args(["-I", "-c", "import ast,sys; ast.parse(sys.stdin.read())"])
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let Ok(mut filho) = filho else {
            eprintln!("python3 ausente: sintaxe não conferida");
            return;
        };
        filho
            .stdin
            .take()
            .unwrap()
            .write_all(fonte.as_bytes())
            .unwrap();
        let saida = filho.wait_with_output().unwrap();
        assert!(
            saida.status.success(),
            "{}",
            String::from_utf8_lossy(&saida.stderr)
        );
    }
}
