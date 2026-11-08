//! Acesso de fora pelo ngrok: temporário, só para testes (ADR 0015, SDD-05).
//!
//! O técnico cola o authtoken do ngrok (guardado cifrado pelo cofre) e o
//! programa sobe um túnel HTTPS com o **agente oficial** `ngrok` (achado no
//! PATH ou baixado de `bin.equinox.io` para `camfacil-dados/bin`). Sem SDK:
//! o agente é um processo à parte, então o binário do CamFácil IA não cresce
//! e a RAM só é gasta enquanto o túnel está ligado.
//!
//! - **Painel**: túnel para a dashboard com usuário e senha (ação `basic-auth`
//!   da Traffic Policy do ngrok, num arquivo 0600 que some ao desligar; o
//!   `--basic-auth` antigo está obsoleto e deixaria a senha visível no `ps`).
//!   A senha é aleatória e aparece na tela. Sem isso o painel ficaria aberto.
//! - **Só o MCP**: um servidor à parte em `127.0.0.1:<porta livre>` que só
//!   tem `/mcp` (o ngrok não filtra caminho); a proteção é o token do MCP.
//! - Ao fechar o programa o túnel cai junto (`parar` no encerramento).
//! - Pode desligar sozinho depois de 24 h, se o técnico marcar.
//! - Android: não disponível (o app não roda o agente do ngrok).

use crate::api::{bad, internal, ApiResult};
use crate::Shared;
use axum::{
    extract::{ConnectInfo, State},
    http::{Extensions, StatusCode},
    Json,
};
use cf_notify::cofre::{self, Cofre};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::BufRead;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

const CHAVE_AUTHTOKEN: &str = "ngrok_authtoken";
const CHAVE_AUTO24: &str = "ngrok_auto24";
/// Origem oficial do agente v3 (a mesma do instalador do site do ngrok).
const BASE_DOWNLOAD: &str = "https://bin.equinox.io/c/bNyj1mQVY4c/ngrok-v3-stable-";
/// Hosts aceitos no download (e em redirecionamentos).
const HOSTS_OFICIAIS: &[&str] = &["bin.equinox.io", "equinox.io", "bin.ngrok.com", "ngrok.com"];
pub const LINK_AUTHTOKEN: &str = "https://dashboard.ngrok.com/get-started/your-authtoken";
const LIMITE_DOWNLOAD: usize = 64 * 1024 * 1024;
const LIMITE_BINARIO: usize = 160 * 1024 * 1024;
const USUARIO: &str = "camfacil";
const DIA: i64 = 24 * 3600;

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Para onde o túnel aponta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alvo {
    Painel,
    Mcp,
}

impl Alvo {
    fn nome(self) -> &'static str {
        match self {
            Alvo::Painel => "painel",
            Alvo::Mcp => "mcp",
        }
    }
}

// ---------------- Peças puras (testadas) ----------------

/// Endereço oficial do agente para o sistema e a arquitetura.
pub fn url_download(os: &str, arch: &str) -> Option<String> {
    let a = match arch {
        "x86_64" => "amd64",
        "x86" => "386",
        "aarch64" => "arm64",
        "arm" => "arm",
        _ => return None,
    };
    let (o, ext) = match os {
        "linux" => ("linux", "tgz"),
        "windows" => ("windows", "zip"),
        "macos" => ("darwin", "zip"),
        "freebsd" => ("freebsd", "tgz"),
        _ => return None,
    };
    if o == "darwin" && !matches!(a, "amd64" | "arm64") {
        return None;
    }
    Some(format!("{BASE_DOWNLOAD}{o}-{a}.{ext}"))
}

fn host_oficial(host: &str) -> bool {
    HOSTS_OFICIAIS
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

/// Argumentos do agente (o authtoken vai pela variável `NGROK_AUTHTOKEN` e a
/// senha num arquivo de política, nunca na linha de comando, que outros
/// usuários da máquina enxergam).
pub fn argumentos(porta: u16, politica: Option<&str>) -> Vec<String> {
    let mut a = vec![
        "http".to_string(),
        format!("127.0.0.1:{porta}"),
        "--log=stdout".into(),
        "--log-format=json".into(),
    ];
    if let Some(p) = politica {
        a.push("--traffic-policy-file".into());
        a.push(p.to_string());
    }
    a
}

/// Traffic Policy do ngrok pedindo usuário e senha em todo pedido.
pub fn politica_basic_auth(usuario: &str, senha: &str) -> String {
    format!(
        "on_http_request:\n  - actions:\n      - type: basic-auth\n        config:\n          credentials:\n            - \"{usuario}:{senha}\"\n"
    )
}

fn arquivo_politica() -> PathBuf {
    cf_core::paths::data_dir().join("ngrok-politica.yml")
}

fn gravar_politica(conteudo: &str) -> Result<PathBuf, String> {
    let p = arquivo_politica();
    let _ = std::fs::remove_file(&p);
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    use std::io::Write;
    o.open(&p)
        .and_then(|mut f| f.write_all(conteudo.as_bytes()))
        .map_err(|e| format!("não deu para gravar a política do ngrok: {e}"))?;
    Ok(p)
}

/// URL pública HTTPS do túnel que aponta para `porta`, pela API local do
/// agente (`GET http://127.0.0.1:4040/api/tunnels`).
pub fn interpretar_tunnels(json: &str, porta: u16) -> Option<String> {
    let v: Value = serde_json::from_str(json).ok()?;
    let fim = format!(":{porta}");
    v["tunnels"].as_array()?.iter().find_map(|t| {
        let url = t["public_url"].as_str()?;
        let addr = t["config"]["addr"].as_str().unwrap_or("");
        (url.starts_with("https://") && addr.trim_end_matches('/').ends_with(&fim))
            .then(|| url.to_string())
    })
}

/// O que uma linha do log JSON do agente diz.
#[derive(Debug, PartialEq)]
pub enum Linha {
    Url(String),
    Erro(String),
    Nada,
}

pub fn interpretar_log(linha: &str) -> Linha {
    let Ok(v) = serde_json::from_str::<Value>(linha) else {
        return Linha::Nada;
    };
    if let Some(u) = v["url"].as_str().filter(|u| u.starts_with("https://")) {
        if v["msg"]
            .as_str()
            .is_some_and(|m| m.contains("started tunnel"))
        {
            return Linha::Url(u.to_string());
        }
    }
    let lvl = v["lvl"].as_str().unwrap_or("");
    if matches!(lvl, "eror" | "error" | "crit") {
        if let Some(e) = v["err"].as_str().or(v["msg"].as_str()) {
            return Linha::Erro(explicar_erro(e));
        }
    }
    Linha::Nada
}

/// Erros comuns do ngrok em palavras simples.
pub fn explicar_erro(e: &str) -> String {
    let b = e.to_lowercase();
    if b.contains("err_ngrok_4018")
        || b.contains("err_ngrok_105")
        || b.contains("authentication failed")
        || b.contains("authtoken")
    {
        format!("o ngrok recusou o authtoken. Copie de novo em {LINK_AUTHTOKEN}")
    } else if b.contains("err_ngrok_108") || b.contains("simultaneous") {
        "já existe outro ngrok ligado nesta conta (o plano grátis só permite um). Desligue o outro e tente de novo".into()
    } else {
        e.chars().take(240).collect()
    }
}

fn nome_do_binario() -> &'static str {
    if cfg!(windows) {
        "ngrok.exe"
    } else {
        "ngrok"
    }
}

fn basename(n: &str) -> &str {
    n.rsplit(['/', '\\']).next().unwrap_or(n)
}

/// Tira o executável de um `.tgz` (gzip + tar), sem crate nova.
pub fn extrair_tgz(dados: &[u8], nome: &str) -> Result<Vec<u8>, String> {
    if dados.len() < 18 || dados[0] != 0x1f || dados[1] != 0x8b || dados[2] != 8 {
        return Err("arquivo baixado não é gzip".into());
    }
    let flags = dados[3];
    let mut i = 10usize;
    let pular_zero = |i: &mut usize| {
        while *i < dados.len() && dados[*i] != 0 {
            *i += 1;
        }
        *i += 1;
    };
    if flags & 4 != 0 {
        let n = u16::from_le_bytes([dados[i], dados[i + 1]]) as usize;
        i += 2 + n;
    }
    if flags & 8 != 0 {
        pular_zero(&mut i);
    }
    if flags & 16 != 0 {
        pular_zero(&mut i);
    }
    if flags & 2 != 0 {
        i += 2;
    }
    if i + 8 > dados.len() {
        return Err("gzip cortado".into());
    }
    let tar = miniz_oxide::inflate::decompress_to_vec_with_limit(
        &dados[i..dados.len() - 8],
        LIMITE_BINARIO,
    )
    .map_err(|_| "gzip corrompido".to_string())?;
    let mut off = 0usize;
    while off + 512 <= tar.len() {
        let h = &tar[off..off + 512];
        if h.iter().all(|&b| b == 0) {
            break;
        }
        let campo = |a: usize, b: usize| {
            let s = &h[a..b];
            let fim = s.iter().position(|&c| c == 0).unwrap_or(s.len());
            String::from_utf8_lossy(&s[..fim]).into_owned()
        };
        let mut n = campo(0, 100);
        let prefixo = campo(345, 500);
        if !prefixo.is_empty() && &h[257..262] == b"ustar" {
            n = format!("{prefixo}/{n}");
        }
        let tamanho = usize::from_str_radix(campo(124, 136).trim(), 8)
            .map_err(|_| "tar inválido".to_string())?;
        let tipo = h[156];
        let ini = off + 512;
        if ini + tamanho > tar.len() {
            return Err("tar cortado".into());
        }
        if (tipo == b'0' || tipo == 0) && basename(&n) == nome {
            return Ok(tar[ini..ini + tamanho].to_vec());
        }
        off = ini + tamanho.div_ceil(512) * 512;
    }
    Err(format!("{nome} não está no arquivo baixado"))
}

/// Tira o executável de um `.zip` (diretório central + deflate), sem crate nova.
pub fn extrair_zip(dados: &[u8], nome: &str) -> Result<Vec<u8>, String> {
    let u16le = |i: usize| u16::from_le_bytes([dados[i], dados[i + 1]]) as usize;
    let u32le = |i: usize| {
        u32::from_le_bytes([dados[i], dados[i + 1], dados[i + 2], dados[i + 3]]) as usize
    };
    if dados.len() < 22 {
        return Err("zip cortado".into());
    }
    let fim = (0..=dados.len() - 22)
        .rev()
        .take(70_000)
        .find(|&i| dados[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .ok_or("arquivo baixado não é zip")?;
    let total = u16le(fim + 10);
    let mut p = u32le(fim + 16);
    for _ in 0..total {
        if p + 46 > dados.len() || dados[p..p + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err("zip inválido".into());
        }
        let metodo = u16le(p + 10);
        let comp = u32le(p + 20);
        let (ln, le, lc) = (u16le(p + 28), u16le(p + 30), u16le(p + 32));
        let local = u32le(p + 42);
        let n = String::from_utf8_lossy(&dados[p + 46..p + 46 + ln]).into_owned();
        if basename(&n) == nome {
            if local + 30 > dados.len() || dados[local..local + 4] != [0x50, 0x4b, 0x03, 0x04] {
                return Err("zip inválido".into());
            }
            let ini = local + 30 + u16le(local + 26) + u16le(local + 28);
            let bruto = dados.get(ini..ini + comp).ok_or("zip cortado")?;
            return match metodo {
                0 => Ok(bruto.to_vec()),
                8 => miniz_oxide::inflate::decompress_to_vec_with_limit(bruto, LIMITE_BINARIO)
                    .map_err(|_| "zip corrompido".to_string()),
                _ => Err("compressão do zip não suportada".into()),
            };
        }
        p += 46 + ln + le + lc;
    }
    Err(format!("{nome} não está no arquivo baixado"))
}

// ---------------- Binário ----------------

fn pasta_bin() -> PathBuf {
    cf_core::paths::data_dir().join("bin")
}

/// Agente `ngrok`: no PATH ou o que o programa baixou.
pub fn binario() -> Option<PathBuf> {
    let exe = nome_do_binario();
    let no_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(exe))
            .find(|p| p.is_file())
    });
    no_path.or_else(|| Some(pasta_bin().join(exe)).filter(|p| p.is_file()))
}

/// `ngrok version` precisa responder "ngrok version 3…".
fn conferir_binario(p: &std::path::Path) -> Result<String, String> {
    let mut cmd = std::process::Command::new(p);
    cmd.arg("version")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("não deu para rodar o ngrok: {e}"))?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.starts_with("ngrok version 3") {
        Ok(s)
    } else {
        Err("o arquivo baixado não é o agente do ngrok".into())
    }
}

/// Baixa o agente oficial para `camfacil-dados/bin`, confere e devolve o caminho.
async fn baixar(app: &Shared) -> Result<PathBuf, String> {
    let url = url_download(std::env::consts::OS, std::env::consts::ARCH)
        .ok_or("não há ngrok oficial para este sistema; instale à mão (ngrok.com/download)")?;
    let politica = reqwest::redirect::Policy::custom(|a| {
        let ok = a.url().scheme() == "https" && a.url().host_str().is_some_and(host_oficial);
        if !ok {
            a.error("redirecionado para fora do ngrok")
        } else if a.previous().len() > 4 {
            a.stop()
        } else {
            a.follow()
        }
    });
    let http = cf_notify::tls::raizes_do_sistema(reqwest::Client::builder())
        .redirect(politica)
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let mut r = http
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("não deu para baixar o ngrok: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("o site do ngrok respondeu {}", r.status()));
    }
    let mut dados = Vec::new();
    while let Some(c) = r
        .chunk()
        .await
        .map_err(|e| format!("download interrompido: {e}"))?
    {
        dados.extend_from_slice(&c);
        if dados.len() > LIMITE_DOWNLOAD {
            return Err("download grande demais; cancelado".into());
        }
    }
    let nome = nome_do_binario();
    let exe = tokio::task::spawn_blocking(move || {
        if url.ends_with(".zip") {
            extrair_zip(&dados, nome)
        } else {
            extrair_tgz(&dados, nome)
        }
    })
    .await
    .map_err(|_| "erro ao abrir o arquivo".to_string())??;
    let pasta = pasta_bin();
    std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
    let temp = pasta.join(format!("{nome}.part"));
    std::fs::write(&temp, &exe).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o755));
    }
    let t2 = temp.clone();
    let versao = tokio::task::spawn_blocking(move || conferir_binario(&t2))
        .await
        .map_err(|_| "erro ao conferir o ngrok".to_string())?;
    let versao = match versao {
        Ok(v) => v,
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            return Err(e);
        }
    };
    let final_ = pasta.join(nome);
    std::fs::rename(&temp, &final_).map_err(|e| e.to_string())?;
    use sha2::Digest;
    let hash: String = sha2::Sha256::digest(&exe)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    registrar(
        app,
        &format!("ngrok baixado de bin.equinox.io ({versao}, sha256 {hash})"),
    );
    Ok(final_)
}

// ---------------- Túnel ----------------

struct Tunel {
    id: u64,
    processo: std::process::Child,
    alvo: Alvo,
    url: Option<String>,
    erro: Option<String>,
    desde: i64,
    senha: Option<String>,
    servidor: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for Tunel {
    fn drop(&mut self) {
        let _ = self.processo.kill();
        let _ = self.processo.wait();
        if let Some(s) = self.servidor.take() {
            s.abort();
        }
        if self.senha.is_some() {
            let _ = std::fs::remove_file(arquivo_politica());
        }
    }
}

static TUNEL: Mutex<Option<Tunel>> = Mutex::new(None);
static ULTIMO_ERRO: Mutex<Option<String>> = Mutex::new(None);
static PROXIMO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn com_tunel<R>(id: u64, f: impl FnOnce(&mut Tunel) -> R) -> Option<R> {
    let mut g = TUNEL.lock().unwrap_or_else(|e| e.into_inner());
    g.as_mut().filter(|t| t.id == id).map(f)
}

fn registrar(app: &Shared, detalhe: &str) {
    let _ = app
        .store
        .registrar_evento(0, None, "acesso", detalhe, agora());
    println!("ngrok: {detalhe}");
}

/// Derruba o túnel (se houver). Chamado também ao fechar o programa.
pub fn parar() -> bool {
    let t = TUNEL.lock().unwrap_or_else(|e| e.into_inner()).take();
    t.is_some()
}

/// Desligar o MCP derruba o túnel que só servia o `/mcp`.
pub fn parar_se_mcp(app: &Shared) {
    let era_mcp = TUNEL
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|t| t.alvo == Alvo::Mcp);
    if era_mcp && parar() {
        registrar(app, "túnel do ngrok desligado junto com o MCP");
    }
}

fn authtoken(app: &Shared) -> Option<String> {
    let g = app.store.get(CHAVE_AUTHTOKEN).ok().flatten()?;
    Cofre::abrir().ok()?.decifrar(&g).filter(|t| !t.is_empty())
}

fn porta_local(app: &Shared) -> u16 {
    app.store
        .get("port")
        .ok()
        .flatten()
        .and_then(|p| p.parse().ok())
        .unwrap_or(7070)
}

fn ler_linhas(id: u64, fonte: impl std::io::Read + Send + 'static) {
    std::thread::spawn(move || {
        for linha in std::io::BufReader::new(fonte).lines().map_while(Result::ok) {
            let l = interpretar_log(&linha);
            let solta = linha.trim_start();
            if l == Linha::Nada
                && !solta.starts_with('{')
                && (linha.contains("ERR_NGROK") || solta.starts_with("ERROR"))
            {
                let e = explicar_erro(&linha);
                com_tunel(id, |t| t.erro = Some(e));
                continue;
            }
            match l {
                Linha::Url(u) => {
                    com_tunel(id, |t| t.url = Some(u));
                }
                Linha::Erro(e) => {
                    com_tunel(id, |t| t.erro = Some(e));
                }
                Linha::Nada => {}
            }
        }
    });
}

async fn url_pela_api(porta: u16) -> Option<String> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_millis(800))
        .no_proxy()
        .build()
        .ok()?;
    for p in 4040..4044 {
        let Ok(r) = http
            .get(format!("http://127.0.0.1:{p}/api/tunnels"))
            .send()
            .await
        else {
            continue;
        };
        if let Some(u) = interpretar_tunnels(&r.text().await.unwrap_or_default(), porta) {
            return Some(u);
        }
    }
    None
}

fn processo_vivo(id: u64) -> Option<bool> {
    com_tunel(id, |t| matches!(t.processo.try_wait(), Ok(None)))
}

async fn ligar(app: &Shared, alvo: Alvo, quem: &str) -> Result<(), String> {
    let token = authtoken(app).ok_or("cole o authtoken do ngrok primeiro")?;
    if alvo == Alvo::Mcp && !(crate::mcp::ligado(app) && crate::mcp::tem_token(app)) {
        return Err("ligue o MCP (com token) antes de abrir o túnel para ele".into());
    }
    parar();
    let bin = match binario() {
        Some(b) => b,
        None => baixar(app).await?,
    };
    // Alvo: a dashboard (com senha) ou um servidor só com /mcp.
    let (porta, servidor, senha) = match alvo {
        Alvo::Painel => {
            let s: String = cofre::aleatorio(12)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            (porta_local(app), None, Some(s))
        }
        Alvo::Mcp => {
            let l = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|e| e.to_string())?;
            let p = l.local_addr().map_err(|e| e.to_string())?.port();
            let r = crate::mcp::router_so_mcp(app.clone());
            let h = tokio::spawn(async move {
                let _ = axum::serve(
                    l,
                    r.into_make_service_with_connect_info::<std::net::SocketAddr>(),
                )
                .await;
            });
            (p, Some(h), None)
        }
    };
    let politica = match &senha {
        Some(s) => Some(gravar_politica(&politica_basic_auth(USUARIO, s))?),
        None => None,
    };
    let politica = politica.map(|p| p.display().to_string());
    let mut cmd = std::process::Command::new(&bin);
    cmd.args(argumentos(porta, politica.as_deref()))
        .env("NGROK_AUTHTOKEN", token)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut proc = match cmd.spawn() {
        Ok(p) => p,
        Err(e) => {
            if let Some(h) = servidor {
                h.abort();
            }
            return Err(format!("não deu para abrir o ngrok: {e}"));
        }
    };
    let id = PROXIMO.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let (out, err) = (proc.stdout.take(), proc.stderr.take());
    *TUNEL.lock().unwrap_or_else(|e| e.into_inner()) = Some(Tunel {
        id,
        processo: proc,
        alvo,
        url: None,
        erro: None,
        desde: agora(),
        senha,
        servidor,
    });
    if let Some(o) = out {
        ler_linhas(id, o);
    }
    if let Some(e) = err {
        ler_linhas(id, e);
    }
    // Espera a URL (log JSON; se não vier, pergunta à API local do agente).
    let mut url = None;
    for volta in 0..60 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let (u, e) = com_tunel(id, |t| (t.url.clone(), t.erro.clone())).unwrap_or_default();
        if u.is_some() {
            url = u;
            break;
        }
        if processo_vivo(id) != Some(true) {
            parar();
            return Err(e.unwrap_or_else(|| "o ngrok fechou sem abrir o túnel".into()));
        }
        if volta % 8 == 7 {
            if let Some(u) = url_pela_api(porta).await {
                com_tunel(id, |t| t.url = Some(u.clone()));
                url = Some(u);
                break;
            }
        }
    }
    let Some(url) = url else {
        let e = com_tunel(id, |t| t.erro.clone()).flatten();
        parar();
        return Err(e.unwrap_or_else(|| "o ngrok não abriu o túnel em 15 s".into()));
    };
    registrar(
        app,
        &format!("ngrok ligado ({}) por {quem}: {url}", alvo.nome()),
    );
    vigiar(app.clone(), id);
    Ok(())
}

/// Vigia o túnel: avisa se caiu e desliga sozinho após 24 h (se marcado).
fn vigiar(app: Shared, id: u64) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let Some((desde, vivo)) =
                com_tunel(id, |t| (t.desde, matches!(t.processo.try_wait(), Ok(None))))
            else {
                return;
            };
            if !vivo {
                let e = com_tunel(id, |t| t.erro.clone()).flatten();
                *ULTIMO_ERRO.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(e.unwrap_or_else(|| "o túnel do ngrok caiu".into()));
                parar();
                registrar(&app, "túnel do ngrok caiu");
                return;
            }
            let auto = app.store.get(CHAVE_AUTO24).ok().flatten().as_deref() == Some("1");
            if auto && agora() - desde >= DIA {
                parar();
                registrar(&app, "ngrok desligado sozinho depois de 24 h");
                return;
            }
        }
    });
}

// ---------------- API ----------------

fn quem(ext: &Extensions) -> String {
    ext.get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|c| {
            let ip = c.0.ip();
            if ip.is_loopback() {
                "este computador".to_string()
            } else {
                format!("o aparelho {ip}")
            }
        })
        .unwrap_or_else(|| "a dashboard".into())
}

fn situacao(app: &Shared) -> Value {
    let disponivel = !cfg!(target_os = "android");
    let mut r = json!({
        "disponivel": disponivel,
        "sistema": std::env::consts::OS,
        "instalado": binario().is_some(),
        "baixavel": url_download(std::env::consts::OS, std::env::consts::ARCH).is_some(),
        "tem_authtoken": authtoken(app).is_some(),
        "auto24": app.store.get(CHAVE_AUTO24).ok().flatten().as_deref() == Some("1"),
        "link_authtoken": LINK_AUTHTOKEN,
        "ligado": false,
        "erro": ULTIMO_ERRO.lock().unwrap_or_else(|e| e.into_inner()).clone(),
    });
    let g = TUNEL.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(t) = g.as_ref() {
        let url = t.url.clone();
        r["ligado"] = json!(url.is_some());
        r["alvo"] = json!(t.alvo.nome());
        r["url"] = json!(url.as_ref().map(|u| match t.alvo {
            Alvo::Mcp => format!("{u}/mcp"),
            Alvo::Painel => u.clone(),
        }));
        r["desde"] = json!(t.desde);
        r["ha_s"] = json!(agora() - t.desde);
        r["usuario"] = json!(t.senha.as_ref().map(|_| USUARIO));
        r["senha"] = json!(t.senha);
        if t.erro.is_some() {
            r["erro"] = json!(t.erro);
        }
    }
    r
}

/// `GET /api/ngrok`.
pub async fn get_ngrok(State(app): State<Shared>) -> Json<Value> {
    Json(situacao(&app))
}

#[derive(Deserialize)]
pub struct Pedido {
    authtoken: Option<String>,
    auto24: Option<bool>,
    acao: Option<String>,
    alvo: Option<String>,
    /// O técnico leu o aviso ("temporário, não é seguro").
    #[serde(default)]
    entendi: bool,
}

/// `POST /api/ngrok`: guarda o authtoken, liga, desliga, desligar após 24 h.
pub async fn post_ngrok(
    State(app): State<Shared>,
    ext: Extensions,
    Json(p): Json<Pedido>,
) -> ApiResult {
    if cfg!(target_os = "android") {
        return Err(bad(
            StatusCode::BAD_REQUEST,
            "o ngrok não está disponível no Android; use o Tailscale",
        ));
    }
    if let Some(t) = p.authtoken {
        let t = t.trim();
        if t.is_empty() {
            app.store.set(CHAVE_AUTHTOKEN, "").map_err(internal)?;
        } else {
            if t.len() > 200
                || !t
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Err(bad(StatusCode::BAD_REQUEST, "authtoken inválido"));
            }
            let c = Cofre::abrir().map_err(internal)?;
            app.store
                .set(CHAVE_AUTHTOKEN, &c.cifrar(t))
                .map_err(internal)?;
        }
    }
    if let Some(a) = p.auto24 {
        app.store
            .set(CHAVE_AUTO24, if a { "1" } else { "0" })
            .map_err(internal)?;
    }
    match p.acao.as_deref() {
        Some("ligar") => {
            if !p.entendi {
                return Err(bad(
                    StatusCode::BAD_REQUEST,
                    "leia e confirme o aviso do ngrok antes de ligar",
                ));
            }
            crate::termos::exigir_tecnico(&app)?;
            let alvo = match p.alvo.as_deref() {
                Some("mcp") => Alvo::Mcp,
                _ => Alvo::Painel,
            };
            *ULTIMO_ERRO.lock().unwrap_or_else(|e| e.into_inner()) = None;
            ligar(&app, alvo, &quem(&ext))
                .await
                .map_err(|e| bad(StatusCode::BAD_GATEWAY, e))?;
        }
        Some("desligar") => {
            if parar() {
                registrar(&app, &format!("ngrok desligado por {}", quem(&ext)));
            }
            *ULTIMO_ERRO.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        Some(_) => return Err(bad(StatusCode::BAD_REQUEST, "ação inválida")),
        None => {}
    }
    Ok(Json(situacao(&app)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_oficial_por_sistema() {
        assert_eq!(
            url_download("linux", "x86_64").unwrap(),
            "https://bin.equinox.io/c/bNyj1mQVY4c/ngrok-v3-stable-linux-amd64.tgz"
        );
        assert!(url_download("windows", "x86_64")
            .unwrap()
            .ends_with("windows-amd64.zip"));
        assert!(url_download("macos", "aarch64")
            .unwrap()
            .ends_with("darwin-arm64.zip"));
        assert!(url_download("linux", "aarch64")
            .unwrap()
            .ends_with("linux-arm64.tgz"));
        assert!(url_download("android", "aarch64").is_none());
        assert!(url_download("macos", "x86").is_none());
        assert!(host_oficial("bin.equinox.io") && host_oficial("bin.ngrok.com"));
        assert!(!host_oficial("equinox.io.evil.example") && !host_oficial("evilngrok.com"));
    }

    #[test]
    fn comando_do_agente() {
        assert_eq!(
            argumentos(7070, None).join(" "),
            "http 127.0.0.1:7070 --log=stdout --log-format=json"
        );
        let a = argumentos(7070, Some("/dados/ngrok-politica.yml"));
        assert_eq!(
            &a[4..],
            ["--traffic-policy-file", "/dados/ngrok-politica.yml"]
        );
        assert!(
            !a.iter().any(|x| x.contains("authtoken")),
            "authtoken nunca na linha de comando"
        );
        assert_eq!(
            politica_basic_auth("camfacil", "abc123"),
            "on_http_request:\n  - actions:\n      - type: basic-auth\n        config:\n          credentials:\n            - \"camfacil:abc123\"\n"
        );
    }

    #[test]
    fn le_api_do_agente() {
        let j = r#"{"tunnels":[
            {"name":"outro","public_url":"https://velho.ngrok-free.app","proto":"https","config":{"addr":"http://127.0.0.1:3000"}},
            {"name":"command_line","public_url":"https://ab12.ngrok-free.app","proto":"https","config":{"addr":"http://127.0.0.1:7070","inspect":true}}
        ],"uri":"/api/tunnels"}"#;
        assert_eq!(
            interpretar_tunnels(j, 7070).as_deref(),
            Some("https://ab12.ngrok-free.app")
        );
        assert_eq!(interpretar_tunnels(j, 9999), None);
        assert_eq!(interpretar_tunnels(r#"{"tunnels":[]}"#, 7070), None);
        assert_eq!(interpretar_tunnels("lixo", 7070), None);
    }

    #[test]
    fn le_log_json() {
        let l = r#"{"addr":"http://127.0.0.1:7070","lvl":"info","msg":"started tunnel","name":"command_line","obj":"tunnels","t":"2026-11-08T10:00:00-03:00","url":"https://ab12.ngrok-free.app"}"#;
        assert_eq!(
            interpretar_log(l),
            Linha::Url("https://ab12.ngrok-free.app".into())
        );
        let e = r#"{"lvl":"eror","msg":"session closing","obj":"tunnels.session","err":"authentication failed: The authtoken you specified is invalid. ERR_NGROK_107"}"#;
        assert!(matches!(interpretar_log(e), Linha::Erro(m) if m.contains("authtoken")));
        let e = r#"{"lvl":"crit","msg":"x","err":"ERR_NGROK_108 simultaneous sessions"}"#;
        assert!(matches!(interpretar_log(e), Linha::Erro(m) if m.contains("plano grátis")));
        assert_eq!(
            interpretar_log(r#"{"lvl":"info","msg":"open config file"}"#),
            Linha::Nada
        );
        assert_eq!(interpretar_log("texto solto"), Linha::Nada);
    }

    fn tar_com(nome: &str, conteudo: &[u8]) -> Vec<u8> {
        let mut h = vec![0u8; 512];
        h[..nome.len()].copy_from_slice(nome.as_bytes());
        let tam = format!("{:011o}\0", conteudo.len());
        h[124..136].copy_from_slice(tam.as_bytes());
        h[156] = b'0';
        let mut t = h;
        t.extend_from_slice(conteudo);
        t.resize(t.len().div_ceil(512) * 512, 0);
        t.extend(vec![0u8; 1024]);
        t
    }

    #[test]
    fn extrai_tgz() {
        let tar = [
            tar_com("LEIA.txt", b"oi")[..1024].to_vec(),
            tar_com("ngrok", b"\x7fELF binario"),
        ]
        .concat();
        let mut gz = vec![0x1f, 0x8b, 8, 8, 0, 0, 0, 0, 0, 3];
        gz.extend_from_slice(b"ngrok.tar\0");
        gz.extend(miniz_oxide::deflate::compress_to_vec(&tar, 6));
        gz.extend([0u8; 8]);
        assert_eq!(extrair_tgz(&gz, "ngrok").unwrap(), b"\x7fELF binario");
        assert!(extrair_tgz(&gz, "outro").is_err());
        assert!(extrair_tgz(b"nao e gzip nenhum aqui", "ngrok").is_err());
    }

    #[test]
    fn extrai_zip() {
        let conteudo = b"MZ binario do windows".repeat(20);
        let comp = miniz_oxide::deflate::compress_to_vec(&conteudo, 6);
        let nome = b"ngrok.exe";
        let mut z = Vec::new();
        // Cabeçalho local (tamanhos zerados, como o Go faz com descritor).
        z.extend([0x50, 0x4b, 0x03, 0x04, 20, 0, 8, 0, 8, 0]);
        z.extend([0u8; 16]);
        z.extend((nome.len() as u16).to_le_bytes());
        z.extend(0u16.to_le_bytes());
        z.extend(nome);
        z.extend(&comp);
        let cd = z.len() as u32;
        z.extend([0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 8, 0, 8, 0]);
        z.extend([0u8; 8]);
        z.extend((comp.len() as u32).to_le_bytes());
        z.extend((conteudo.len() as u32).to_le_bytes());
        z.extend((nome.len() as u16).to_le_bytes());
        z.extend([0u8; 12]);
        z.extend(0u32.to_le_bytes());
        z.extend(nome);
        let tam_cd = z.len() as u32 - cd;
        z.extend([0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 1, 0, 1, 0]);
        z.extend(tam_cd.to_le_bytes());
        z.extend(cd.to_le_bytes());
        z.extend(0u16.to_le_bytes());
        assert_eq!(extrair_zip(&z, "ngrok.exe").unwrap(), conteudo);
        assert!(extrair_zip(&z, "ngrok").is_err());
        assert!(extrair_zip(b"curto", "ngrok").is_err());
    }

    #[test]
    fn erros_em_portugues() {
        assert!(explicar_erro("ERR_NGROK_4018 authentication failed").contains(LINK_AUTHTOKEN));
        assert!(explicar_erro("ERR_NGROK_108").contains("outro ngrok"));
    }
}
