//! Nível médio de IA (SDD-02): ONNX Runtime carregado em tempo de execução e
//! modelo YOLO baixados sob demanda para `data_dir/modelos`.
//!
//! O binário continua pequeno: `ort` com `load-dynamic` só sabe abrir a
//! biblioteca; ela e o modelo (~10 MB cada) vêm quando o técnico liga o
//! primeiro módulo médio, com barra de progresso e cancelar. Os dois são
//! conferidos por SHA-256 e ficam na pasta de dados (offline depois disso).

pub mod arquivos;
pub mod detector;

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// Versão do ONNX Runtime baixada (releases oficiais de microsoft/onnxruntime).
pub const ORT_VERSAO: &str = "1.20.1";

/// Modelo: YOLO11n exportado pelo Ultralytics em ONNX (640×640, opset 12,
/// sem NMS embutido), licença AGPL-3.0 como o projeto. Publicado na release
/// `modelos-v1` deste repositório; cópia em `modelos/` no repositório.
pub const MODELO_NOME: &str = "yolo11n.onnx";
pub const MODELO_URLS: &[&str] = &[
    "https://github.com/emersoncassis/CamFacil-IA/releases/download/modelos-v1/yolo11n.onnx",
    "https://raw.githubusercontent.com/emersoncassis/CamFacil-IA/main/modelos/yolo11n.onnx",
];
pub const MODELO_SHA256: &str = include_str!("../../../../modelos/yolo11n.onnx.sha256");

/// Pacote do ONNX Runtime para esta máquina.
pub struct Pacote {
    pub url: String,
    pub sha256: &'static str,
    /// Nome (sufixo) da biblioteca dentro do pacote.
    pub lib: &'static str,
    pub zip: bool,
}

pub fn pacote_ort() -> Option<Pacote> {
    let (nome, sha, lib, zip) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => (
            "linux-x64",
            "67db4dc1561f1e3fd42e619575c82c601ef89849afc7ea85a003abbac1a1a105",
            "libonnxruntime.so.1.20.1",
            false,
        ),
        ("linux", "aarch64") => (
            "linux-aarch64",
            "ae4fedbdc8c18d688c01306b4b50c63de3445cdf2dbd720e01a2fa3810b8106a",
            "libonnxruntime.so.1.20.1",
            false,
        ),
        ("windows", "x86_64") => (
            "win-x64",
            "78d447051e48bd2e1e778bba378bec4ece11191c9e538cf7b2c4a4565e8f5581",
            "onnxruntime.dll",
            true,
        ),
        ("windows", "aarch64") => (
            "win-arm64",
            "21ded41cf725dec14bf138f891438afd06f5363fe8b2477049e7262bd7238ce2",
            "onnxruntime.dll",
            true,
        ),
        ("macos", "aarch64") => (
            "osx-arm64",
            "b678fc3c2354c771fea4fba420edeccfba205140088334df801e7fc40e83a57a",
            "libonnxruntime.1.20.1.dylib",
            false,
        ),
        ("macos", "x86_64") => (
            "osx-x86_64",
            "0f73006813af2a1a5d1723ed7dfb694fc629d15037124081bb61b7bf7d99fc78",
            "libonnxruntime.1.20.1.dylib",
            false,
        ),
        _ => return None,
    };
    let ext = if zip { "zip" } else { "tgz" };
    Some(Pacote {
        url: format!(
            "https://github.com/microsoft/onnxruntime/releases/download/v{ORT_VERSAO}/onnxruntime-{nome}-{ORT_VERSAO}.{ext}"
        ),
        sha256: sha,
        lib,
        zip,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fase {
    Parado,
    Onnxruntime,
    Modelo,
    Extraindo,
    Pronto,
    Erro,
    Cancelado,
}

#[derive(Debug, Clone, Serialize)]
pub struct Andamento {
    pub fase: Fase,
    pub baixado: u64,
    pub total: u64,
    pub erro: Option<String>,
}

#[derive(Default, Debug, Clone, Serialize)]
pub struct EstadoDetector {
    pub carregado: bool,
    pub inferencias: u64,
    pub ultimo_ms: u64,
    pub erro: Option<String>,
}

pub struct Ia {
    dir: PathBuf,
    andamento: Mutex<Andamento>,
    baixando: AtomicBool,
    cancelar: AtomicBool,
    pub detector: Mutex<EstadoDetector>,
}

impl Ia {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("modelos"),
            andamento: Mutex::new(Andamento {
                fase: Fase::Parado,
                baixado: 0,
                total: 0,
                erro: None,
            }),
            baixando: AtomicBool::new(false),
            cancelar: AtomicBool::new(false),
            detector: Mutex::new(EstadoDetector::default()),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Caminho da biblioteca do ONNX Runtime, se já baixada (ou apontada por
    /// `CAMFACIL_ORT_LIB`, para quem já tem o runtime instalado).
    /// Há como rodar o nível médio aqui? Pacote oficial para baixar, ou a
    /// biblioteca já embutida (app Android: vem no APK, CAMFACIL_ORT_LIB).
    pub fn suportado(&self) -> bool {
        pacote_ort().is_some() || self.lib_ort().is_some()
    }

    pub fn lib_ort(&self) -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("CAMFACIL_ORT_LIB") {
            let p = PathBuf::from(p);
            if p.is_file() {
                return Some(p);
            }
        }
        let p = self.dir.join("onnxruntime").join(pacote_ort()?.lib);
        p.is_file().then_some(p)
    }

    pub fn modelo(&self) -> Option<PathBuf> {
        let p = self.dir.join(MODELO_NOME);
        p.is_file().then_some(p)
    }

    pub fn pronto(&self) -> bool {
        self.lib_ort().is_some() && self.modelo().is_some()
    }

    pub fn andamento(&self) -> Andamento {
        self.andamento.lock().unwrap().clone()
    }

    pub fn baixando(&self) -> bool {
        self.baixando.load(Ordering::Relaxed)
    }

    pub fn cancelar(&self) {
        if self.baixando() {
            self.cancelar.store(true, Ordering::Relaxed);
        }
    }

    fn marcar(&self, fase: Fase, baixado: u64, total: u64) {
        let mut a = self.andamento.lock().unwrap();
        a.fase = fase;
        a.baixado = baixado;
        a.total = total;
    }

    fn falhar(&self, erro: String) {
        let mut a = self.andamento.lock().unwrap();
        a.fase = if erro == "cancelado" {
            Fase::Cancelado
        } else {
            Fase::Erro
        };
        a.erro = Some(erro);
    }

    /// Baixa o que falta (runtime e/ou modelo). Uma por vez; devolve `false`
    /// se já havia um download em andamento.
    pub async fn baixar(&self) -> bool {
        if self
            .baixando
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return false;
        }
        self.cancelar.store(false, Ordering::Relaxed);
        self.andamento.lock().unwrap().erro = None;
        let r = self.baixar_tudo().await;
        match r {
            Ok(()) => self.marcar(Fase::Pronto, 0, 0),
            Err(e) => self.falhar(e),
        }
        self.baixando.store(false, Ordering::Relaxed);
        true
    }

    async fn baixar_tudo(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let cliente = cf_notify::tls::raizes_do_sistema(reqwest::Client::builder())
            .connect_timeout(Duration::from_secs(20))
            .user_agent(concat!("CamFacilIA/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| e.to_string())?;
        if self.lib_ort().is_none() {
            let p = pacote_ort().ok_or_else(|| {
                format!(
                    "não há ONNX Runtime oficial para {} {}",
                    std::env::consts::OS,
                    std::env::consts::ARCH
                )
            })?;
            let tmp = self.dir.join("onnxruntime.pacote");
            self.baixar_arquivo(
                &cliente,
                std::slice::from_ref(&p.url),
                &tmp,
                p.sha256,
                Fase::Onnxruntime,
            )
            .await?;
            self.marcar(Fase::Extraindo, 0, 0);
            let destino = self.dir.join("onnxruntime").join(p.lib);
            let lib = p.lib;
            let zip = p.zip;
            let tmp2 = tmp.clone();
            let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
                let dados = std::fs::read(&tmp2).map_err(|e| e.to_string())?;
                let achado = if zip {
                    arquivos::zip_extrair(&dados, lib)
                } else {
                    arquivos::tar_extrair(&arquivos::gunzip(&dados)?, lib)
                };
                achado.ok_or_else(|| format!("{lib} não está no pacote"))
            })
            .await
            .map_err(|e| e.to_string())??;
            std::fs::create_dir_all(destino.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&destino, bytes).map_err(|e| e.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&destino, std::fs::Permissions::from_mode(0o755));
            }
            let _ = std::fs::remove_file(&tmp);
        }
        if self.modelo().is_none() {
            let tmp = self.dir.join(format!("{MODELO_NOME}.parte"));
            let urls: Vec<String> = MODELO_URLS.iter().map(|s| s.to_string()).collect();
            self.baixar_arquivo(&cliente, &urls, &tmp, MODELO_SHA256.trim(), Fase::Modelo)
                .await?;
            std::fs::rename(&tmp, self.dir.join(MODELO_NOME)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Baixa de uma das URLs (na ordem) para `destino`, conferindo o SHA-256.
    async fn baixar_arquivo(
        &self,
        cliente: &reqwest::Client,
        urls: &[String],
        destino: &Path,
        sha_esperado: &str,
        fase: Fase,
    ) -> Result<(), String> {
        let mut ultimo_erro = String::from("sem URL");
        for url in urls {
            match self
                .baixar_de(cliente, url, destino, sha_esperado, fase)
                .await
            {
                Ok(()) => return Ok(()),
                Err(e) if e == "cancelado" => {
                    let _ = std::fs::remove_file(destino);
                    return Err(e);
                }
                Err(e) => {
                    let _ = std::fs::remove_file(destino);
                    ultimo_erro = e;
                }
            }
        }
        Err(ultimo_erro)
    }

    async fn baixar_de(
        &self,
        cliente: &reqwest::Client,
        url: &str,
        destino: &Path,
        sha_esperado: &str,
        fase: Fase,
    ) -> Result<(), String> {
        use std::io::Write;
        let mut resp = cliente.get(url).send().await.map_err(|e| traduzir(&e))?;
        if !resp.status().is_success() {
            return Err(format!("servidor respondeu {} em {url}", resp.status()));
        }
        let total = resp.content_length().unwrap_or(0);
        self.marcar(fase, 0, total);
        let mut f = std::fs::File::create(destino).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut baixado = 0u64;
        loop {
            if self.cancelar.load(Ordering::Relaxed) {
                return Err("cancelado".into());
            }
            let pedaco = tokio::time::timeout(Duration::from_secs(60), resp.chunk())
                .await
                .map_err(|_| "o download parou de responder".to_string())?
                .map_err(|e| traduzir(&e))?;
            let Some(b) = pedaco else { break };
            hash.update(&b);
            f.write_all(&b).map_err(|e| e.to_string())?;
            baixado += b.len() as u64;
            self.marcar(fase, baixado, total);
        }
        f.flush().map_err(|e| e.to_string())?;
        drop(f);
        let sha: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if !sha_esperado.is_empty() && sha != sha_esperado {
            return Err(format!(
                "arquivo baixado não bate com a assinatura esperada (SHA-256 {}… ≠ {}…)",
                &sha[..12],
                &sha_esperado[..sha_esperado.len().min(12)]
            ));
        }
        Ok(())
    }

    /// Apaga runtime e modelo (libera ~25 MB em disco).
    pub fn apagar(&self) -> Result<(), String> {
        if self.baixando() {
            return Err("há um download em andamento".into());
        }
        let _ = std::fs::remove_dir_all(self.dir.join("onnxruntime"));
        let _ = std::fs::remove_file(self.dir.join(MODELO_NOME));
        self.marcar(Fase::Parado, 0, 0);
        Ok(())
    }
}

fn traduzir(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "sem resposta (internet?)".into()
    } else if e.is_connect() {
        "sem conexão com a internet".into()
    } else {
        format!("falha de rede: {e}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacote_desta_plataforma_tem_url_oficial() {
        if let Some(p) = pacote_ort() {
            assert!(p
                .url
                .starts_with("https://github.com/microsoft/onnxruntime/releases/download/"));
            assert_eq!(p.sha256.len(), 64);
            assert!(p.lib.contains("onnxruntime"));
        }
        assert_eq!(MODELO_SHA256.trim().len(), 64);
    }

    #[test]
    fn caminhos_e_estado_inicial() {
        let dir = std::env::temp_dir().join(format!("cf-ia-{}", std::process::id()));
        let ia = Ia::new(&dir);
        assert!(!ia.pronto());
        assert!(ia.modelo().is_none());
        assert_eq!(ia.andamento().fase, Fase::Parado);
        assert!(!ia.baixando());
    }
}
