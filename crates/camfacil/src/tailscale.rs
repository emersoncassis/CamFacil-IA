//! Acesso de fora pelo Tailscale (ADR-0005, SDD-05).
//!
//! Não embute VPN nem abre porta no roteador: se o cliente Tailscale já está
//! instalado e logado na máquina, o CamFácil IA passa a escutar também no IP
//! dele (100.x.y.z), e o técnico abre a dashboard pelo celular na mesma rede
//! Tailscale. O vídeo vai direto, criptografado, sem passar pela nuvem.
//! Sem Tailscale, nada muda: só localhost (ou a LAN, com `--lan`).

use crate::Shared;
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

/// Tempo máximo esperando o comando `tailscale` (nunca trava a UI).
const TEMPO: Duration = Duration::from_millis(2500);
/// Intervalo da vigília que liga a escuta no IP Tailscale sem reiniciar.
const VIGIA: Duration = Duration::from_secs(60);

/// Estado compartilhado: em qual IP Tailscale estamos escutando.
#[derive(Default)]
pub struct Estado {
    escutando: Mutex<Option<Ipv4Addr>>,
    porta: Mutex<u16>,
}

impl Estado {
    pub fn escutando(&self) -> Option<Ipv4Addr> {
        *self.escutando.lock().unwrap()
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Situacao {
    pub instalado: bool,
    pub logado: bool,
    pub ip: Option<Ipv4Addr>,
    /// Nome MagicDNS (ex.: `loja-pc.tail1234.ts.net`), quando houver.
    pub nome: Option<String>,
    pub erro: Option<String>,
}

/// Onde o cliente Tailscale costuma estar instalado em cada sistema.
pub fn binario() -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "tailscale.exe"
    } else {
        "tailscale"
    };
    let no_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(exe))
            .find(|p| p.is_file())
    });
    if no_path.is_some() {
        return no_path;
    }
    let fixos: &[&str] = if cfg!(windows) {
        &[r"C:\Program Files\Tailscale\tailscale.exe"]
    } else if cfg!(target_os = "macos") {
        &[
            "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
            "/usr/local/bin/tailscale",
            "/opt/homebrew/bin/tailscale",
        ]
    } else {
        &[
            "/usr/bin/tailscale",
            "/usr/sbin/tailscale",
            "/usr/local/bin/tailscale",
        ]
    };
    fixos.iter().map(PathBuf::from).find(|p| p.is_file())
}

/// Comando de instalação sugerido para este sistema.
pub fn comando_instalar() -> &'static str {
    if cfg!(windows) {
        "winget install tailscale.tailscale"
    } else if cfg!(target_os = "macos") {
        "brew install --cask tailscale"
    } else {
        "curl -fsSL https://tailscale.com/install.sh | sh && sudo tailscale up"
    }
}

async fn rodar(args: &[&str]) -> Result<String, String> {
    let bin = binario().ok_or_else(|| "tailscale não instalado".to_string())?;
    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000); // sem janela de console
    }
    let out = tokio::time::timeout(TEMPO, cmd.output())
        .await
        .map_err(|_| "o tailscale demorou para responder".to_string())?
        .map_err(|e| format!("não foi possível rodar o tailscale: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let e = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if e.is_empty() {
            "o tailscale devolveu erro".into()
        } else {
            e.chars().take(200).collect()
        })
    }
}

/// Interpreta `tailscale status --json`.
pub fn interpretar_status(json: &str) -> Situacao {
    let v: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => {
            return Situacao {
                instalado: true,
                erro: Some("resposta do tailscale não entendida".into()),
                ..Default::default()
            }
        }
    };
    let logado = v["BackendState"].as_str() == Some("Running");
    let ip = v["Self"]["TailscaleIPs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str()?.parse::<Ipv4Addr>().ok())
        .next();
    let nome = v["Self"]["DNSName"]
        .as_str()
        .map(|n| n.trim_end_matches('.').to_string())
        .filter(|n| !n.is_empty());
    Situacao {
        instalado: true,
        logado: logado && ip.is_some(),
        ip: if logado { ip } else { None },
        nome: if logado { nome } else { None },
        erro: (!logado).then(|| {
            format!(
                "Tailscale instalado, mas não conectado ({})",
                v["BackendState"].as_str().unwrap_or("?")
            )
        }),
    }
}

/// Situação atual, sem travar: dois comandos curtos com timeout.
pub async fn situacao() -> Situacao {
    if binario().is_none() {
        return Situacao::default();
    }
    match rodar(&["status", "--json"]).await {
        Ok(s) => interpretar_status(&s),
        Err(e) => {
            // Versões antigas podem não ter `--json`; tenta só o IP.
            match ip().await {
                Some(ip) => Situacao {
                    instalado: true,
                    logado: true,
                    ip: Some(ip),
                    nome: None,
                    erro: None,
                },
                None => Situacao {
                    instalado: true,
                    erro: Some(e),
                    ..Default::default()
                },
            }
        }
    }
}

/// Só o IPv4 Tailscale desta máquina (`tailscale ip -4`), se logado.
pub async fn ip() -> Option<Ipv4Addr> {
    rodar(&["ip", "-4"])
        .await
        .ok()?
        .lines()
        .find_map(|l| l.trim().parse().ok())
}

/// Passa a escutar no IP Tailscale se ainda não escuta. Devolve se ligou agora.
async fn escutar(app: &Shared, router: &axum::Router, ip: Ipv4Addr) -> bool {
    let porta = *app.tailscale.porta.lock().unwrap();
    if app.tailscale.escutando() == Some(ip) {
        return false;
    }
    let addr = SocketAddr::new(IpAddr::V4(ip), porta);
    match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => {
            *app.tailscale.escutando.lock().unwrap() = Some(ip);
            println!("Acesso de fora (Tailscale): http://{ip}:{porta}");
            let r = router.clone();
            tokio::spawn(async move {
                let _ = axum::serve(l, r).await;
            });
            true
        }
        Err(e) => {
            eprintln!("Tailscale: não deu para escutar em {addr}: {e}");
            false
        }
    }
}

/// Vigília leve: confere o Tailscale agora e a cada minuto; quando ele estiver
/// logado, liga a escuta no IP dele sem precisar reiniciar. Com `--lan` o
/// servidor já escuta em todas as interfaces e não há nada a fazer.
pub fn vigiar(app: Shared, router: axum::Router, porta: u16, lan: bool) {
    *app.tailscale.porta.lock().unwrap() = porta;
    if lan {
        return;
    }
    tokio::spawn(async move {
        loop {
            if binario().is_some() {
                if let Some(ip) = ip().await {
                    escutar(&app, &router, ip).await;
                }
            }
            tokio::time::sleep(VIGIA).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpreta_status_logado() {
        let j = r#"{"BackendState":"Running","Self":{"DNSName":"loja-pc.tail1234.ts.net.","TailscaleIPs":["100.101.102.103","fd7a:115c::1"]}}"#;
        let s = interpretar_status(j);
        assert!(s.instalado && s.logado);
        assert_eq!(s.ip, Some("100.101.102.103".parse().unwrap()));
        assert_eq!(s.nome.as_deref(), Some("loja-pc.tail1234.ts.net"));
        assert!(s.erro.is_none());
    }

    #[test]
    fn interpreta_status_parado() {
        let s = interpretar_status(r#"{"BackendState":"NeedsLogin","Self":{"TailscaleIPs":[]}}"#);
        assert!(s.instalado && !s.logado && s.ip.is_none());
        assert!(s.erro.unwrap().contains("NeedsLogin"));
        assert!(interpretar_status("lixo").erro.is_some());
    }

    #[test]
    fn comando_por_sistema() {
        assert!(comando_instalar().contains("tailscale"));
    }
}
