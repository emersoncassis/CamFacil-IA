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
    } else if cfg!(target_os = "android") {
        "instale o app Tailscale (tailscale.com) pela loja e entre na mesma conta do celular"
    } else if cfg!(target_os = "macos") {
        "brew install --cask tailscale"
    } else {
        "curl -fsSL https://tailscale.com/install.sh | sh && sudo tailscale up"
    }
}

async fn rodar(args: &[&str]) -> Result<String, String> {
    rodar_por(args, TEMPO).await
}

async fn rodar_por(args: &[&str], tempo: Duration) -> Result<String, String> {
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
    let out = tokio::time::timeout(tempo, cmd.output())
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

/// Há como saber do Tailscale aqui? No Android o app oficial não expõe o
/// comando `tailscale`; o IP vem da rota (ver `ip_pela_rota`).
fn disponivel() -> bool {
    cfg!(target_os = "android") || binario().is_some()
}

/// IP Tailscale pela tabela de rotas, sem rodar comando nem mandar pacote:
/// um `connect` UDP só escolhe a interface de saída. Se o caminho até o DNS do
/// Tailscale (100.100.100.100) sai por um IP 100.64.0.0/10 diferente do usado
/// para a internet, é a VPN do Tailscale (e não o CGNAT da operadora).
pub fn ip_pela_rota() -> Option<Ipv4Addr> {
    fn origem(destino: &str) -> Option<Ipv4Addr> {
        let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
        s.connect(destino).ok()?;
        match s.local_addr().ok()?.ip() {
            IpAddr::V4(ip) if !ip.is_unspecified() => Some(ip),
            _ => None,
        }
    }
    let ts = origem("100.100.100.100:53")?;
    let o = ts.octets();
    let cgnat = o[0] == 100 && (64..128).contains(&o[1]);
    (cgnat && origem("8.8.8.8:53") != Some(ts)).then_some(ts)
}

/// Situação atual, sem travar: dois comandos curtos com timeout.
pub async fn situacao() -> Situacao {
    if cfg!(target_os = "android") {
        let ip = ip_pela_rota();
        return Situacao {
            instalado: ip.is_some(),
            logado: ip.is_some(),
            ip,
            nome: None,
            erro: None,
        };
    }
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
    if cfg!(target_os = "android") {
        return ip_pela_rota();
    }
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
                let _ = axum::serve(l, r.into_make_service_with_connect_info::<SocketAddr>()).await;
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
            if disponivel() {
                if let Some(ip) = ip().await {
                    escutar(&app, &router, ip).await;
                }
            }
            tokio::time::sleep(VIGIA).await;
        }
    });
}

// ---------------- HTTPS para o MCP (Tailscale Serve / Funnel) ----------------
//
// O Claude (conectores remotos) só aceita HTTPS com certificado válido. O
// Tailscale emite esse certificado para `<máquina>.<tailnet>.ts.net` e faz o
// proxy reverso até `127.0.0.1:<porta>/mcp`. Só o caminho `/mcp` é publicado:
// a dashboard continua fora. Serve = só aparelhos da rede Tailscale; Funnel =
// internet (necessário para o claude.ai, que chama a partir da nuvem).

/// Versão mínima com a sintaxe atual (`serve --bg --https=443 --set-path`).
pub const VERSAO_MINIMA: (u32, u32) = (1, 52);
/// Portas que o Funnel aceita (o Serve também usa estas, na mesma ordem).
pub const PORTAS_HTTPS: [u16; 3] = [443, 8443, 10000];
/// Caminho publicado.
pub const CAMINHO: &str = "/mcp";
/// Painel do Tailscale: DNS (MagicDNS e certificados HTTPS).
pub const PAINEL_DNS: &str = "https://login.tailscale.com/admin/dns";
/// Painel do Tailscale: política de acesso (atributo `funnel`).
pub const PAINEL_FUNNEL: &str = "https://login.tailscale.com/admin/acls/file";
/// Tempo máximo de `serve`/`funnel` (o primeiro uso pode demorar um pouco).
const TEMPO_SERVE: Duration = Duration::from_secs(20);

/// Serve (só a rede Tailscale) ou Funnel (internet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Modo {
    Serve,
    Funnel,
}

impl Modo {
    pub fn cmd(self) -> &'static str {
        match self {
            Modo::Serve => "serve",
            Modo::Funnel => "funnel",
        }
    }
}

/// O que o tailnet permite para esta máquina (de `tailscale status --json`).
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Recursos {
    /// Nome MagicDNS desta máquina, sem o ponto final.
    pub nome: Option<String>,
    /// Certificados HTTPS habilitados no tailnet (`Self.CertDomains`).
    pub https: bool,
    /// Política do tailnet dá o atributo `funnel` a esta máquina.
    pub funnel: bool,
    /// Portas liberadas para o Funnel.
    pub portas_funnel: Vec<u16>,
}

fn tem_cap(v: &serde_json::Value, nome: &str) -> bool {
    let lista = v["Self"]["Capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c.as_str());
    let mapa = v["Self"]["CapMap"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.keys().map(String::as_str));
    lista.chain(mapa).any(|c| c == nome)
}

fn caps(v: &serde_json::Value) -> Vec<String> {
    let lista = v["Self"]["Capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c.as_str().map(String::from));
    let mapa = v["Self"]["CapMap"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.keys().cloned());
    lista.chain(mapa).collect()
}

/// Interpreta nome, HTTPS e Funnel de `tailscale status --json`.
pub fn interpretar_recursos(json: &str) -> Recursos {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return Recursos::default();
    };
    let nome = v["Self"]["DNSName"]
        .as_str()
        .map(|n| n.trim_end_matches('.').to_string())
        .filter(|n| !n.is_empty());
    let https = v["Self"]["CertDomains"]
        .as_array()
        .is_some_and(|l| l.iter().any(|d| d.as_str().is_some_and(|d| !d.is_empty())))
        || v["CertDomains"].as_array().is_some_and(|l| !l.is_empty());
    let funnel = tem_cap(&v, "funnel") || tem_cap(&v, "https://tailscale.com/cap/funnel");
    // "https://tailscale.com/cap/funnel-ports?ports=443,8443,10000"
    let mut portas: Vec<u16> = caps(&v)
        .iter()
        .filter_map(|c| c.strip_prefix("https://tailscale.com/cap/funnel-ports?ports="))
        .flat_map(|p| {
            p.split(',')
                .filter_map(|n| n.trim().parse().ok())
                .collect::<Vec<u16>>()
        })
        .filter(|p| PORTAS_HTTPS.contains(p))
        .collect();
    if funnel && portas.is_empty() {
        portas = PORTAS_HTTPS.to_vec();
    }
    portas.sort_unstable();
    portas.dedup();
    Recursos {
        nome,
        https,
        funnel,
        portas_funnel: if funnel { portas } else { Vec::new() },
    }
}

/// Versão do cliente (`tailscale version`, primeira linha `1.76.1`).
pub fn interpretar_versao(saida: &str) -> Option<(u32, u32, u32)> {
    let l = saida.lines().next()?.trim();
    let base = l.split(['-', ' ']).next()?;
    let mut p = base.split('.').map(|n| n.parse::<u32>().ok());
    Some((p.next()??, p.next()??, p.next().flatten().unwrap_or(0)))
}

pub fn versao_ok(v: (u32, u32, u32)) -> bool {
    (v.0, v.1) >= VERSAO_MINIMA
}

/// Uma porta HTTPS na configuração de `tailscale serve status --json`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PortaServe {
    pub porta: u16,
    /// Caminhos publicados nessa porta (ex.: `/`, `/mcp`).
    pub caminhos: Vec<String>,
    pub funnel: bool,
    /// Porta usada como encaminhamento TCP cru (não dá para pôr HTTPS nela).
    pub tcp_cru: bool,
}

/// Configuração atual do Serve/Funnel (só o que importa aqui).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigServe {
    pub portas: Vec<PortaServe>,
}

impl ConfigServe {
    fn porta(&self, p: u16) -> Option<&PortaServe> {
        self.portas.iter().find(|x| x.porta == p)
    }

    /// Onde o `/mcp` já está publicado: (porta, é Funnel?).
    pub fn mcp(&self) -> Option<(u16, bool)> {
        self.portas
            .iter()
            .find(|p| p.caminhos.iter().any(|c| c == CAMINHO))
            .map(|p| (p.porta, p.funnel))
    }

    /// Escolhe a porta HTTPS para publicar o `/mcp` **sem mexer** no que o
    /// usuário já tem: no Funnel, a liberação vale para a porta inteira, então
    /// não usa porta com outros caminhos (eles iriam para a internet junto);
    /// no Serve, não usa porta com Funnel (o `/mcp` ficaria público).
    pub fn escolher_porta(&self, modo: Modo, portas_funnel: &[u16]) -> Option<u16> {
        PORTAS_HTTPS
            .iter()
            .copied()
            .filter(|p| modo == Modo::Serve || portas_funnel.contains(p))
            .find(|&p| match self.porta(p) {
                None => true,
                Some(x) if x.tcp_cru => false,
                Some(x) => {
                    let outros = x.caminhos.iter().any(|c| c != CAMINHO);
                    match modo {
                        Modo::Funnel => !outros,
                        Modo::Serve => !x.funnel || !outros,
                    }
                }
            })
    }
}

/// Interpreta `tailscale serve status --json` (vazio ou `{}` = nada publicado).
pub fn interpretar_serve(json: &str) -> ConfigServe {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return ConfigServe::default();
    };
    let porta_de = |hp: &str| hp.rsplit(':').next().and_then(|p| p.parse::<u16>().ok());
    let mut portas: Vec<PortaServe> = Vec::new();
    fn pegar(p: u16, portas: &mut Vec<PortaServe>) -> usize {
        if let Some(i) = portas.iter().position(|x| x.porta == p) {
            return i;
        }
        portas.push(PortaServe {
            porta: p,
            ..Default::default()
        });
        portas.len() - 1
    }
    if let Some(tcp) = v["TCP"].as_object() {
        for (p, cfg) in tcp {
            let Ok(p) = p.parse::<u16>() else { continue };
            let i = pegar(p, &mut portas);
            portas[i].tcp_cru = cfg["HTTPS"].as_bool() != Some(true)
                && cfg["HTTP"].as_bool() != Some(true)
                && cfg.get("TCPForward").is_some();
        }
    }
    if let Some(web) = v["Web"].as_object() {
        for (hp, cfg) in web {
            let Some(p) = porta_de(hp) else { continue };
            let i = pegar(p, &mut portas);
            if let Some(h) = cfg["Handlers"].as_object() {
                portas[i].caminhos.extend(h.keys().cloned());
            }
        }
    }
    if let Some(f) = v["AllowFunnel"].as_object() {
        for (hp, on) in f {
            if on.as_bool() != Some(true) {
                continue;
            }
            let Some(p) = porta_de(hp) else { continue };
            let i = pegar(p, &mut portas);
            portas[i].funnel = true;
        }
    }
    portas.sort_by_key(|p| p.porta);
    ConfigServe { portas }
}

/// Argumentos para publicar `/mcp` (sem executar).
pub fn args_publicar(modo: Modo, porta_https: u16, porta_local: u16) -> Vec<String> {
    vec![
        modo.cmd().into(),
        "--bg".into(),
        format!("--https={porta_https}"),
        format!("--set-path={CAMINHO}"),
        format!("http://127.0.0.1:{porta_local}{CAMINHO}"),
    ]
}

/// Argumentos para tirar só o `/mcp` (o resto da configuração fica).
pub fn args_despublicar(modo: Modo, porta_https: u16) -> Vec<String> {
    vec![
        modo.cmd().into(),
        format!("--https={porta_https}"),
        format!("--set-path={CAMINHO}"),
        "off".into(),
    ]
}

/// Endereço HTTPS do MCP para o nome MagicDNS e a porta.
pub fn url_https(nome: &str, porta: u16) -> String {
    if porta == 443 {
        format!("https://{nome}{CAMINHO}")
    } else {
        format!("https://{nome}:{porta}{CAMINHO}")
    }
}

/// Traduz o erro do `tailscale` para algo que o técnico entende.
pub fn explicar_erro(e: &str) -> String {
    let b = e.to_lowercase();
    if b.contains("access denied") || b.contains("operator") || b.contains("permission denied") {
        "o Tailscale pediu permissão de administrador. No Linux, rode uma vez: sudo tailscale set --operator=$USER (e tente de novo)".into()
    } else if b.contains("flag provided but not defined") || b.contains("unknown flag") {
        "este Tailscale é antigo e não entende os comandos atuais. Atualize o Tailscale (1.52 ou mais novo)".into()
    } else if b.contains("funnel")
        && (b.contains("not enabled") || b.contains("not available") || b.contains("nodeattr"))
    {
        format!("o Funnel não está liberado no seu tailnet. Libere na política de acesso: {PAINEL_FUNNEL}")
    } else if b.contains("https") && (b.contains("not enabled") || b.contains("certificate")) {
        format!("os certificados HTTPS não estão ligados no seu tailnet. Ligue em {PAINEL_DNS} (HTTPS Certificates)")
    } else if b.contains("demorou") {
        format!("o Tailscale não respondeu. Talvez ele esteja esperando você liberar HTTPS no painel: {PAINEL_DNS}")
    } else {
        e.to_string()
    }
}

/// Versão do cliente instalado.
pub async fn versao() -> Option<(u32, u32, u32)> {
    interpretar_versao(&rodar(&["version"]).await.ok()?)
}

/// Recursos do tailnet para esta máquina.
pub async fn recursos() -> Result<Recursos, String> {
    rodar(&["status", "--json"])
        .await
        .map(|s| interpretar_recursos(&s))
}

/// Configuração atual do Serve/Funnel.
pub async fn config_serve() -> Result<ConfigServe, String> {
    rodar(&["serve", "status", "--json"])
        .await
        .map(|s| interpretar_serve(&s))
}

/// Roda um `serve`/`funnel` montado por `args_publicar`/`args_despublicar`.
pub async fn aplicar(args: &[String]) -> Result<(), String> {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    rodar_por(&a, TEMPO_SERVE)
        .await
        .map(|_| ())
        .map_err(|e| explicar_erro(&e))
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
    fn ip_pela_rota_so_devolve_cgnat() {
        // Sem Tailscale na máquina de teste dá None; com ele, só 100.64.0.0/10.
        if let Some(ip) = ip_pela_rota() {
            let o = ip.octets();
            assert!(o[0] == 100 && (64..128).contains(&o[1]));
        }
    }

    const STATUS_FUNNEL: &str = r#"{"BackendState":"Running","MagicDNSSuffix":"tail1234.ts.net",
        "Self":{"DNSName":"loja-pc.tail1234.ts.net.","TailscaleIPs":["100.101.102.103"],
        "CertDomains":["loja-pc.tail1234.ts.net"],
        "Capabilities":["HTTPS","https://tailscale.com/cap/funnel","https://tailscale.com/cap/funnel-ports?ports=443,8443"],
        "CapMap":{"https":null,"funnel":null,"https://tailscale.com/cap/funnel-ports?ports=443,8443":null}}}"#;

    #[test]
    fn recursos_com_https_e_funnel() {
        let r = interpretar_recursos(STATUS_FUNNEL);
        assert_eq!(r.nome.as_deref(), Some("loja-pc.tail1234.ts.net"));
        assert!(r.https && r.funnel);
        assert_eq!(r.portas_funnel, vec![443, 8443]);
    }

    #[test]
    fn recursos_so_capmap_e_sem_https() {
        // Versões novas só têm CapMap; sem CertDomains = HTTPS desligado.
        let j = r#"{"Self":{"DNSName":"pc.t.ts.net.","CapMap":{"funnel":null}}}"#;
        let r = interpretar_recursos(j);
        assert!(!r.https && r.funnel);
        assert_eq!(r.portas_funnel, PORTAS_HTTPS.to_vec());
        let r = interpretar_recursos(r#"{"Self":{"DNSName":"pc.t.ts.net.","CertDomains":null}}"#);
        assert!(!r.https && !r.funnel && r.portas_funnel.is_empty());
        assert_eq!(interpretar_recursos("lixo"), Recursos::default());
    }

    #[test]
    fn versao_do_cliente() {
        assert_eq!(
            interpretar_versao("1.76.1\n  tailscale commit: abc\n"),
            Some((1, 76, 1))
        );
        assert_eq!(interpretar_versao("1.50.0-t1234"), Some((1, 50, 0)));
        assert!(versao_ok((1, 52, 0)) && versao_ok((2, 0, 0)));
        assert!(!versao_ok((1, 50, 9)));
        assert_eq!(interpretar_versao(""), None);
    }

    #[test]
    fn le_serve_status() {
        let j = r#"{"TCP":{"443":{"HTTPS":true},"22":{"TCPForward":"127.0.0.1:22"}},
            "Web":{"loja-pc.tail1234.ts.net:443":{"Handlers":{"/":{"Proxy":"http://127.0.0.1:3000"},"/mcp":{"Proxy":"http://127.0.0.1:7070/mcp"}}}},
            "AllowFunnel":{"loja-pc.tail1234.ts.net:443":false}}"#;
        let c = interpretar_serve(j);
        assert_eq!(c.mcp(), Some((443, false)));
        let p = c.porta(443).unwrap();
        assert!(!p.funnel && !p.tcp_cru && p.caminhos.len() == 2);
        assert!(c.porta(22).unwrap().tcp_cru);
        assert_eq!(interpretar_serve("{}"), ConfigServe::default());
        assert_eq!(interpretar_serve(""), ConfigServe::default());
    }

    #[test]
    fn porta_nao_derruba_config_do_usuario() {
        let vazio = ConfigServe::default();
        assert_eq!(vazio.escolher_porta(Modo::Serve, &[]), Some(443));
        assert_eq!(
            vazio.escolher_porta(Modo::Funnel, &[8443, 10000]),
            Some(8443)
        );
        assert_eq!(vazio.escolher_porta(Modo::Funnel, &[]), None);
        // Usuário já serve "/" na 443 (só tailnet): Serve divide a porta,
        // Funnel não (publicaria o "/" dele na internet).
        let c = interpretar_serve(r#"{"Web":{"pc:443":{"Handlers":{"/":{}}}}}"#);
        assert_eq!(c.escolher_porta(Modo::Serve, &[]), Some(443));
        assert_eq!(c.escolher_porta(Modo::Funnel, &PORTAS_HTTPS), Some(8443));
        // Usuário tem Funnel na 443 com "/": Serve do /mcp vai para outra porta.
        let c = interpretar_serve(
            r#"{"Web":{"pc:443":{"Handlers":{"/":{}}}},"AllowFunnel":{"pc:443":true}}"#,
        );
        assert_eq!(c.escolher_porta(Modo::Serve, &[]), Some(8443));
        // Nosso próprio /mcp com Funnel: dá para reaproveitar a porta.
        let c = interpretar_serve(
            r#"{"Web":{"pc:443":{"Handlers":{"/mcp":{}}}},"AllowFunnel":{"pc:443":true}}"#,
        );
        assert_eq!(c.escolher_porta(Modo::Funnel, &PORTAS_HTTPS), Some(443));
        assert_eq!(c.mcp(), Some((443, true)));
    }

    #[test]
    fn comandos_montados() {
        assert_eq!(
            args_publicar(Modo::Serve, 443, 7070).join(" "),
            "serve --bg --https=443 --set-path=/mcp http://127.0.0.1:7070/mcp"
        );
        assert_eq!(
            args_publicar(Modo::Funnel, 8443, 7071).join(" "),
            "funnel --bg --https=8443 --set-path=/mcp http://127.0.0.1:7071/mcp"
        );
        assert_eq!(
            args_despublicar(Modo::Funnel, 443).join(" "),
            "funnel --https=443 --set-path=/mcp off"
        );
        assert_eq!(url_https("pc.t.ts.net", 443), "https://pc.t.ts.net/mcp");
        assert_eq!(
            url_https("pc.t.ts.net", 8443),
            "https://pc.t.ts.net:8443/mcp"
        );
    }

    #[test]
    fn erros_explicados() {
        assert!(explicar_erro("Access denied: serve config denied").contains("--operator"));
        assert!(explicar_erro("flag provided but not defined: -bg").contains("1.52"));
        assert!(
            explicar_erro("Funnel not available; nodeAttr funnel missing").contains(PAINEL_FUNNEL)
        );
        assert!(explicar_erro("HTTPS certificates not enabled").contains(PAINEL_DNS));
        assert_eq!(explicar_erro("outra coisa"), "outra coisa");
    }

    #[test]
    fn comando_por_sistema() {
        assert!(comando_instalar().contains("tailscale"));
    }
}
