//! Diagnóstico de rede (SDD-05, fase 4): inventário, "foto da rede", diferença
//! entre fotos e diagnóstico das câmeras cadastradas.
//!
//! Tudo aqui roda só sob demanda (ou na vigília leve, a cada 5 min) e nunca
//! trava a interface: a varredura é assíncrona com timeout por conexão. Sem
//! dependência nova: TCP, UDP (ONVIF) e leitura da tabela ARP do sistema.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;

/// Portas sondadas na foto da rede: web, RTSP, Hikvision (SDK) e Dahua/Intelbras (SDK).
pub const PORTAS: [u16; 4] = [80, 554, 8000, 37777];

/// Quantas conexões a varredura faz por host (uma por porta) — para a barra de progresso.
pub fn total_sondas() -> usize {
    254 * PORTAS.len()
}

// ---------------- Fabricante pelo prefixo do MAC ----------------

/// Tabela pequena embutida (prefixos OUI dos fabricantes mais comuns no CFTV
/// brasileiro). Não é a lista inteira do IEEE: quem não está aqui fica sem marca.
const OUI: &[(&str, &[&str])] = &[
    (
        "Intelbras",
        &[
            "001A3F", "58108C", "982A0A", "808FE8", "180D2C", "808544", "AC1EA9", "54BAD9",
            "CC8130", "B87EE5", "D8778B", "98E55B", "D8365F", "4851CF", "443B32", "24FD0D",
            "30E1F1", "546CAC",
        ],
    ),
    (
        "Dahua",
        &[
            "3CEF8C", "4C11BF", "A0BD1D", "E0508B", "9C1463", "38AF29", "BC325F", "B44C3B",
            "E4246C", "6C1C71", "FC5F49", "74C929", "08EDED", "E02EFE", "407AA4", "5CF51A",
            "24526A", "9002A9", "C4AAC4", "F8CE07", "4C99E8", "202C05", "8CE9B4", "14A78B",
            "64FD29", "A8CA87", "98F9CC", "D4430E", "F4B1C2", "3CE36B", "FCB69D", "C0395A",
            "30DDAA",
        ],
    ),
    (
        "Hikvision",
        &[
            "4419B6", "C056E3", "2857BE", "54C415", "BCAD28", "1868CB", "4CBD8F", "A41437",
            "C42F90", "5803FB", "98DF82", "2CA59C", "0C75D2", "548C81", "244845", "ECC89C",
            "8CE748", "2428FD", "ACB92F", "D4E853", "240F9B", "C06DED", "2432AE", "E0BAAD",
            "E0CA3C", "DC07F8", "64DB8B", "94E1AC", "4447CC", "80F5AE", "A42902", "849459",
            "00BC99", "04EECD", "188025", "08CC81", "F84DFC", "849A40", "C0517E", "40ACBF",
            "98F112", "989DE5", "3C1BF8", "4C1F86", "CC13F3", "B0FF0D", "5850ED", "1012FB",
            "4CF5DC", "988B0A", "807C62", "BC9B5E", "BCBAC2", "ACCB51", "80BEAF", "040312",
            "ECA971", "E4D58B", "C8A702", "A44BD9", "88DE39", "083BC1", "48785B", "4C62DF",
        ],
    ),
    (
        "TP-Link",
        &[
            "50C7BF", "F4F26D", "14CC20", "30B5C2", "60E327", "C04A00", "98DAC4", "B04E26",
            "1C3BF3", "54AF97", "003192", "34F716", "54A703", "B0BE76", "34E894", "94D9B3",
            "B0958E", "C025E9", "002719", "40169F", "F4EC38", "808917", "90F652", "14CF92",
            "20DCE6", "54C80F", "E4D332", "C4E984", "282CB2", "E8DE27", "BCD177", "F8CE21",
            "F8C903", "40ED00", "788CB5", "202351", "D84489", "6083E7", "A86E84", "1C61B4",
            "9CA2F4", "CC68B6", "F0A731", "48C381", "CCBABD", "3C7895", "B8FBB3", "186945",
            "D807B6", "646E97", "D03745", "603A7C", "68DDB7", "14D864", "AC84C6", "6CB158",
            "246968", "2C79BE", "54D299", "D02791", "34B126", "F42A7D", "C0C9E3", "F88C21",
            "CC08FB", "D46E0E", "001478", "30FC68", "0019E0", "002586", "54E6FC", "74EA3A",
            "F81A67", "EC172F", "14E6E4", "A8574E", "E894F6", "1CFA68", "306893", "EC750C",
        ],
    ),
    (
        "Ubiquiti",
        &[
            "F09FC2", "802AA8", "788A20", "7483C2", "E063DA", "245A4C", "602232", "E43883",
            "0CEA14", "2CE5BD", "784558", "AC8BA9", "9C05D6", "28704E", "847848", "58D61F",
            "6C63F8", "A89C6C", "74F92C", "0418D6", "24A43C", "44D9E7", "D021F9", "70A741",
            "8CEDE1", "9041B2", "682E3C", "F4E2C6", "D8B370", "B4FBE4", "687251", "FCECDA",
            "942A6F", "1C0B8B", "74FA29", "D489C1", "00156D", "002722", "DC9FDB", "18E829",
        ],
    ),
    (
        "Mikrotik",
        &[
            "085531", "B869F4", "000C42", "F41E57", "789A18", "DC2C6E", "488F5A", "C4AD34",
            "6C3B6B", "D401C3", "04F41C", "D0EA11", "48A98A", "2CC81B", "64D154", "E48D8C",
            "18FD74", "4C5E0C", "D4CA6D", "744D28", "CC2DE0", "38327A",
        ],
    ),
    (
        "Cisco",
        &[
            "E80AB9", "9CE330", "B4DF91", "481BA4", "6C03B5", "B8AB61", "908855", "687161",
            "4CEC0F", "5C64F1", "08F1B3", "5C3E06", "C828E5", "D009C8", "44643C", "24161B",
            "E8DC6C", "E8D322", "34B883", "806A00", "ACBCD9", "4006D5", "0845D1", "6887C6",
            "CC9C3E", "881544", "001D7E", "0014BF", "48F8B3", "C0C1C0", "6C7F0C", "CC6E2A",
        ],
    ),
    (
        "D-Link",
        &[
            "BC2228", "A0A3F0", "BC0F9A", "74DADA", "1062EB", "1C5F2B", "0050BA", "00179A",
            "001CF0", "001E58", "0022B0", "002401", "1CAFF7", "14D64D", "9094E4", "CCB255",
            "28107B", "FC7516", "84C9B2", "C8D3A3", "C412F5", "B0C554", "5CD998", "4086CB",
            "642943", "F07D68", "78321B", "10BEF5", "0015E9", "001B11", "00265A", "C8BE19",
            "C0A0BB", "D8FEE3", "8876B9", "34029C", "0C0E76", "085A11", "A8637D", "0CB6D2",
        ],
    ),
    ("Positivo", &["440BAB", "DC35F1", "00EE01", "009EEE"]),
    ("Multilaser", &["745889"]),
    (
        "Raspberry Pi",
        &[
            "D83ADD", "DCA632", "E45F01", "88A29E", "98FE54", "28CDC1", "B827EB", "2CCF67",
        ],
    ),
    ("Realtek (genérico)", &["00E04C", "FC934E"]),
];

/// Normaliza um MAC para `AA:BB:CC:DD:EE:FF`. Aceita `-`, `:` e dígitos sem zero
/// à esquerda (formato do macOS). Devolve `None` se não parecer um MAC.
pub fn normalizar_mac(s: &str) -> Option<String> {
    let partes: Vec<&str> = s.trim().split([':', '-']).collect();
    if partes.len() != 6 {
        return None;
    }
    let mut out = Vec::with_capacity(6);
    for p in partes {
        if p.is_empty() || p.len() > 2 || !p.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        out.push(format!("{:02X}", u8::from_str_radix(p, 16).ok()?));
    }
    let mac = out.join(":");
    if mac == "00:00:00:00:00:00" || mac == "FF:FF:FF:FF:FF:FF" {
        return None;
    }
    Some(mac)
}

/// Fabricante pelos três primeiros bytes do MAC, se estiver na tabela embutida.
pub fn fabricante_por_mac(mac: &str) -> Option<&'static str> {
    let mac = normalizar_mac(mac)?;
    let prefixo: String = mac.replace(':', "").chars().take(6).collect();
    OUI.iter()
        .find(|(_, lista)| lista.contains(&prefixo.as_str()))
        .map(|(nome, _)| *nome)
}

// ---------------- Inventário ----------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tipo {
    Camera,
    Gravador,
    Roteador,
    Switch,
    Computador,
    Desconhecido,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Dispositivo {
    pub ip: String,
    pub mac: Option<String>,
    pub fabricante: Option<String>,
    pub modelo: Option<String>,
    pub tipo: Tipo,
    pub portas: Vec<u16>,
    pub onvif: bool,
    /// Preenchido quando o IP bate com uma câmera cadastrada.
    pub camera_id: Option<i64>,
    pub camera_nome: Option<String>,
    /// Este computador.
    pub eu: bool,
    /// Rota padrão (roteador).
    pub gateway: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Foto {
    /// Unix (s).
    pub quando: i64,
    /// Ex.: `192.168.1.0/24`.
    pub rede: String,
    pub meu_ip: Option<String>,
    pub gateway: Option<String>,
    pub dispositivos: Vec<Dispositivo>,
}

/// Decide o tipo provável de um aparelho pelo que ele mostrou.
pub fn inferir_tipo(
    fabricante: Option<&str>,
    modelo: Option<&str>,
    portas: &[u16],
    onvif: bool,
    eu: bool,
    gateway: bool,
) -> Tipo {
    if eu {
        return Tipo::Computador;
    }
    if gateway {
        return Tipo::Roteador;
    }
    let m = modelo.unwrap_or("").to_ascii_lowercase();
    let gravador = [
        "nvr", "dvr", "mhdx", "nvd", "xvr", "hdcvi", "gravador", "ds-7", "ds-8",
    ]
    .iter()
    .any(|k| m.contains(k));
    let fab = fabricante.unwrap_or("");
    let marca_cftv = matches!(fab, "Intelbras" | "Dahua" | "Hikvision");
    let marca_rede = matches!(
        fab,
        "TP-Link" | "Ubiquiti" | "Mikrotik" | "Cisco" | "D-Link" | "Multilaser"
    );
    let tem = |p: u16| portas.contains(&p);
    if onvif || marca_cftv && (tem(554) || tem(37777) || tem(8000)) {
        return if gravador {
            Tipo::Gravador
        } else {
            Tipo::Camera
        };
    }
    if gravador {
        return Tipo::Gravador;
    }
    if marca_rede && !tem(554) {
        return Tipo::Switch;
    }
    if matches!(fab, "Raspberry Pi" | "Positivo" | "Realtek (genérico)") {
        return Tipo::Computador;
    }
    if tem(554) {
        return Tipo::Camera;
    }
    Tipo::Desconhecido
}

// ---------------- Diferença entre fotos ----------------

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Mudanca {
    Apareceu {
        ip: String,
        mac: Option<String>,
        fabricante: Option<String>,
    },
    Sumiu {
        ip: String,
        mac: Option<String>,
        fabricante: Option<String>,
        camera_nome: Option<String>,
    },
    MudouIp {
        mac: String,
        de: String,
        para: String,
        fabricante: Option<String>,
        camera_nome: Option<String>,
    },
}

/// Compara duas fotos. Quem tem MAC é seguido pelo MAC (pega troca de IP);
/// quem não tem é seguido pelo IP.
pub fn comparar(anterior: &Foto, atual: &Foto) -> Vec<Mudanca> {
    let mut out = Vec::new();
    let por_mac = |f: &Foto| -> BTreeMap<String, Dispositivo> {
        f.dispositivos
            .iter()
            .filter_map(|d| d.mac.clone().map(|m| (m, d.clone())))
            .collect()
    };
    let a_mac = por_mac(anterior);
    let b_mac = por_mac(atual);
    for (mac, d) in &b_mac {
        match a_mac.get(mac) {
            None => out.push(Mudanca::Apareceu {
                ip: d.ip.clone(),
                mac: Some(mac.clone()),
                fabricante: d.fabricante.clone(),
            }),
            Some(ant) if ant.ip != d.ip => out.push(Mudanca::MudouIp {
                mac: mac.clone(),
                de: ant.ip.clone(),
                para: d.ip.clone(),
                fabricante: d.fabricante.clone(),
                camera_nome: ant.camera_nome.clone().or(d.camera_nome.clone()),
            }),
            _ => {}
        }
    }
    for (mac, d) in &a_mac {
        if !b_mac.contains_key(mac) {
            out.push(Mudanca::Sumiu {
                ip: d.ip.clone(),
                mac: Some(mac.clone()),
                fabricante: d.fabricante.clone(),
                camera_nome: d.camera_nome.clone(),
            });
        }
    }
    // Sem MAC: só por IP.
    fn ips(f: &Foto) -> impl Iterator<Item = &Dispositivo> {
        f.dispositivos.iter().filter(|d| d.mac.is_none())
    }
    for d in ips(atual) {
        if !anterior.dispositivos.iter().any(|a| a.ip == d.ip) {
            out.push(Mudanca::Apareceu {
                ip: d.ip.clone(),
                mac: None,
                fabricante: d.fabricante.clone(),
            });
        }
    }
    for d in ips(anterior) {
        if !atual.dispositivos.iter().any(|a| a.ip == d.ip) {
            out.push(Mudanca::Sumiu {
                ip: d.ip.clone(),
                mac: None,
                fabricante: d.fabricante.clone(),
                camera_nome: d.camera_nome.clone(),
            });
        }
    }
    out
}

// ---------------- URL RTSP: host e troca de IP ----------------

/// Host e porta de uma URL `rtsp://[user:pass@]host[:porta]/...`.
pub fn host_da_url(url: &str) -> Option<(String, u16)> {
    let resto = url.split("://").nth(1)?;
    let autoridade = resto.split('/').next()?;
    let hp = autoridade.rsplit('@').next()?;
    let (host, porta) = match hp.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) && !p.is_empty() => {
            (h, p.parse().unwrap_or(554))
        }
        _ => (hp, 554),
    };
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), porta))
}

/// Reescreve só o host da URL, mantendo esquema, usuário, senha, porta e caminho.
pub fn trocar_host(url: &str, novo_host: &str) -> Option<String> {
    let (esquema, resto) = url.split_once("://")?;
    let fim = resto.find('/').unwrap_or(resto.len());
    let (autoridade, caminho) = resto.split_at(fim);
    let (cred, hp) = match autoridade.rsplit_once('@') {
        Some((c, h)) => (Some(c), h),
        None => (None, autoridade),
    };
    let porta = match hp.rsplit_once(':') {
        Some((_, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => Some(p),
        _ => None,
    };
    let mut out = format!("{esquema}://");
    if let Some(c) = cred {
        out.push_str(c);
        out.push('@');
    }
    out.push_str(novo_host);
    if let Some(p) = porta {
        out.push(':');
        out.push_str(p);
    }
    out.push_str(caminho);
    Some(out)
}

// ---------------- Tabela ARP e rota padrão (por sistema) ----------------

/// Interpreta a saída de `arp -a` do Windows e do macOS/BSD.
pub fn parse_arp_a(texto: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for linha in texto.lines() {
        let l = linha.trim();
        if l.is_empty() {
            continue;
        }
        // macOS: `? (192.168.1.1) at a:bb:cc:dd:ee:ff on en0 ifscope [ethernet]`
        if let Some(i) = l.find(" at ") {
            let ip = l[..i]
                .rsplit('(')
                .next()
                .and_then(|s| s.strip_suffix(')'))
                .unwrap_or("");
            let mac = l[i + 4..].split_whitespace().next().unwrap_or("");
            if let (Ok(ip), Some(mac)) = (ip.parse::<Ipv4Addr>(), normalizar_mac(mac)) {
                map.insert(ip.to_string(), mac);
            }
            continue;
        }
        // Windows: `  192.168.1.1           aa-bb-cc-dd-ee-ff     dinâmico`
        let mut cols = l.split_whitespace();
        if let (Some(ip), Some(mac)) = (cols.next(), cols.next()) {
            if let (Ok(ip), Some(mac)) = (ip.parse::<Ipv4Addr>(), normalizar_mac(mac)) {
                map.insert(ip.to_string(), mac);
            }
        }
    }
    map
}

fn rodar(cmd: &str, args: &[&str]) -> Option<String> {
    let mut c = std::process::Command::new(cmd);
    c.args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let out = c.output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Tabela ARP: `/proc/net/arp` no Linux; `arp -a` no Windows e macOS.
/// Chamada bloqueante (rápida); use em `spawn_blocking` quando houver runtime.
pub fn arp_table() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string("/proc/net/arp") {
        for line in text.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 4 {
                if let Some(mac) = normalizar_mac(cols[3]) {
                    map.insert(cols[0].to_string(), mac);
                }
            }
        }
        return map;
    }
    if let Some(saida) = rodar("arp", &["-a"]) {
        map = parse_arp_a(&saida);
    }
    map
}

/// `/proc/net/route`: gateway da rota 0.0.0.0 (hex little-endian).
pub fn parse_proc_route(texto: &str) -> Option<Ipv4Addr> {
    for l in texto.lines().skip(1) {
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.len() >= 3 && c[1] == "00000000" {
            let n = u32::from_str_radix(c[2], 16).ok()?;
            let ip = Ipv4Addr::from(n.swap_bytes());
            if !ip.is_unspecified() {
                return Some(ip);
            }
        }
    }
    None
}

/// `route print -4` (Windows): linha `0.0.0.0  0.0.0.0  <gateway>  <interface>  métrica`.
pub fn parse_route_print(texto: &str) -> Option<Ipv4Addr> {
    let mut melhor: Option<(u32, Ipv4Addr)> = None;
    for l in texto.lines() {
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.len() >= 5 && c[0] == "0.0.0.0" && c[1] == "0.0.0.0" {
            if let Ok(gw) = c[2].parse::<Ipv4Addr>() {
                let metrica = c[4].parse::<u32>().unwrap_or(u32::MAX);
                if melhor.is_none_or(|(m, _)| metrica < m) {
                    melhor = Some((metrica, gw));
                }
            }
        }
    }
    melhor.map(|(_, gw)| gw)
}

/// `netstat -rn` (macOS/BSD): linha `default  <gateway>  flags  interface`.
pub fn parse_netstat_rn(texto: &str) -> Option<Ipv4Addr> {
    texto.lines().find_map(|l| {
        let mut c = l.split_whitespace();
        (c.next()? == "default")
            .then(|| c.next()?.parse::<Ipv4Addr>().ok())
            .flatten()
    })
}

/// Roteador da rota padrão (bloqueante, rápido).
pub fn gateway_padrao() -> Option<Ipv4Addr> {
    if let Ok(t) = std::fs::read_to_string("/proc/net/route") {
        if let Some(gw) = parse_proc_route(&t) {
            return Some(gw);
        }
    }
    if cfg!(windows) {
        return rodar("route", &["print", "-4"]).and_then(|s| parse_route_print(&s));
    }
    if cfg!(target_os = "macos") {
        return rodar("netstat", &["-rn", "-f", "inet"]).and_then(|s| parse_netstat_rn(&s));
    }
    None
}

// ---------------- Varredura ----------------

/// Varre a /24 de `meu` nas `portas`, uma porta por vez (≤ 254 conexões
/// simultâneas, para não estourar descritores). `progresso` conta sondas feitas.
pub async fn varrer_portas(
    meu: Ipv4Addr,
    portas: &[u16],
    progresso: Arc<AtomicUsize>,
) -> BTreeMap<Ipv4Addr, Vec<u16>> {
    let [a, b, c, _] = meu.octets();
    let mut achados: BTreeMap<Ipv4Addr, Vec<u16>> = BTreeMap::new();
    for &porta in portas {
        let mut tarefas = tokio::task::JoinSet::new();
        for d in 1..=254u8 {
            let ip = Ipv4Addr::new(a, b, c, d);
            let prog = progresso.clone();
            tarefas.spawn(async move {
                let alvo = SocketAddr::from((ip, porta));
                let ok = matches!(
                    tokio::time::timeout(Duration::from_millis(700), TcpStream::connect(alvo))
                        .await,
                    Ok(Ok(_))
                );
                prog.fetch_add(1, Ordering::Relaxed);
                ok.then_some(ip)
            });
        }
        while let Some(r) = tarefas.join_next().await {
            if let Ok(Some(ip)) = r {
                achados.entry(ip).or_default().push(porta);
            }
        }
    }
    for v in achados.values_mut() {
        v.sort_unstable();
    }
    achados
}

/// Câmera cadastrada, só o que a rede precisa saber.
#[derive(Debug, Clone)]
pub struct CameraRede {
    pub id: i64,
    pub nome: String,
    pub host: String,
    pub porta: u16,
}

impl CameraRede {
    pub fn de(cam: &crate::store::Camera) -> Option<Self> {
        if cam.fonte != "rtsp" {
            return None;
        }
        let (host, porta) = host_da_url(&cam.url)?;
        Some(Self {
            id: cam.id,
            nome: cam.nome.clone(),
            host,
            porta,
        })
    }
}

fn agora_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Monta a foto a partir do que a varredura achou (função pura, testável).
pub fn montar_foto(
    meu: Option<Ipv4Addr>,
    gateway: Option<Ipv4Addr>,
    portas: &BTreeMap<Ipv4Addr, Vec<u16>>,
    onvif: &[crate::onvif::Device],
    arp: &BTreeMap<String, String>,
    cameras: &[CameraRede],
    quando: i64,
) -> Foto {
    let rede = meu
        .map(|m| {
            let [a, b, c, _] = m.octets();
            format!("{a}.{b}.{c}.0/24")
        })
        .unwrap_or_else(|| "sem rede".into());
    let mut ips: Vec<Ipv4Addr> = portas.keys().copied().collect();
    for d in onvif {
        if let Ok(ip) = d.ip.parse::<Ipv4Addr>() {
            if !ips.contains(&ip) {
                ips.push(ip);
            }
        }
    }
    if let Some(m) = meu {
        if !ips.contains(&m) {
            ips.push(m);
        }
    }
    if let Some(g) = gateway {
        if !ips.contains(&g) {
            ips.push(g);
        }
    }
    // ARP traz quem respondeu a qualquer coisa (mesmo sem porta aberta).
    for ip in arp.keys() {
        if let Ok(ip) = ip.parse::<Ipv4Addr>() {
            if let Some(m) = meu {
                if m.octets()[..3] == ip.octets()[..3] && !ips.contains(&ip) {
                    ips.push(ip);
                }
            }
        }
    }
    ips.sort_unstable();
    let dispositivos = ips
        .into_iter()
        .map(|ip| {
            let ip_s = ip.to_string();
            let dev = onvif.iter().find(|d| d.ip == ip_s);
            let mac = arp
                .get(&ip_s)
                .cloned()
                .or_else(|| dev.and_then(|d| d.mac.clone()));
            let fabricante = dev.and_then(|d| d.fabricante.clone()).or_else(|| {
                mac.as_deref()
                    .and_then(fabricante_por_mac)
                    .map(String::from)
            });
            let modelo = dev.and_then(|d| d.modelo.clone().or_else(|| d.nome.clone()));
            let mut portas_ip = portas.get(&ip).cloned().unwrap_or_default();
            portas_ip.sort_unstable();
            let eu = meu == Some(ip);
            let gw = gateway == Some(ip);
            let cam = cameras.iter().find(|c| c.host == ip_s);
            let tipo = inferir_tipo(
                fabricante.as_deref(),
                modelo.as_deref(),
                &portas_ip,
                dev.is_some(),
                eu,
                gw,
            );
            let tipo = if cam.is_some() && matches!(tipo, Tipo::Desconhecido) {
                Tipo::Camera
            } else {
                tipo
            };
            Dispositivo {
                ip: ip_s,
                mac,
                fabricante,
                modelo,
                tipo,
                portas: portas_ip,
                onvif: dev.is_some(),
                camera_id: cam.map(|c| c.id),
                camera_nome: cam.map(|c| c.nome.clone()),
                eu,
                gateway: gw,
            }
        })
        .collect();
    Foto {
        quando,
        rede,
        meu_ip: meu.map(|m| m.to_string()),
        gateway: gateway.map(|g| g.to_string()),
        dispositivos,
    }
}

/// Tira a foto da rede: portas + ONVIF em paralelo, depois ARP e rota padrão.
/// Leva uns 3–4 s. Nunca falha: sem rede, devolve só este computador.
pub async fn tirar_foto(cameras: &[CameraRede], progresso: Arc<AtomicUsize>) -> Foto {
    let meu = crate::conectar::ip_local();
    let (portas, onvif) = match meu {
        Some(m) => {
            tokio::join!(
                varrer_portas(m, &PORTAS, progresso.clone()),
                crate::onvif::discover(Duration::from_secs(3))
            )
        }
        None => (BTreeMap::new(), Vec::new()),
    };
    let (arp, gw) = tokio::task::spawn_blocking(|| (arp_table(), gateway_padrao()))
        .await
        .unwrap_or_default();
    progresso.store(total_sondas(), Ordering::Relaxed);
    montar_foto(meu, gw, &portas, &onvif, &arp, cameras, agora_unix())
}

// ---------------- Diagnóstico por câmera ----------------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Dica {
    pub codigo: &'static str,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<&'static str, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostico {
    pub camera_id: i64,
    pub nome: String,
    pub host: String,
    pub porta: u16,
    /// Respondeu na porta RTSP pelo menos uma vez.
    pub alcance: bool,
    /// Média das tentativas que responderam.
    pub latencia_ms: Option<u32>,
    /// Tentativas sem resposta (de 3).
    pub perda: u8,
    pub mesma_rede: Option<bool>,
    pub gateway_ok: Option<bool>,
    pub dns_ok: Option<bool>,
    pub mac: Option<String>,
    /// IP onde o MAC desta câmera apareceu, se diferente do cadastrado.
    pub novo_ip: Option<String>,
    pub dicas: Vec<Dica>,
}

fn faixa(ip: Ipv4Addr) -> String {
    let [a, b, c, _] = ip.octets();
    format!("{a}.{b}.{c}.x")
}

/// Regras do diagnóstico, separadas da rede para teste.
#[allow(clippy::too_many_arguments)]
pub fn avaliar(
    cam: &CameraRede,
    ip_cam: Option<Ipv4Addr>,
    meu: Option<Ipv4Addr>,
    tentativas: &[Option<u32>],
    gateway_ok: Option<bool>,
    dns_ok: Option<bool>,
    mac: Option<String>,
    novo_ip: Option<String>,
) -> Diagnostico {
    let respostas: Vec<u32> = tentativas.iter().flatten().copied().collect();
    let perda = (tentativas.len() - respostas.len()) as u8;
    let alcance = !respostas.is_empty();
    let latencia_ms = alcance.then(|| respostas.iter().sum::<u32>() / respostas.len() as u32);
    let mesma_rede = match (ip_cam, meu) {
        (Some(c), Some(m)) => Some(c.octets()[..3] == m.octets()[..3]),
        _ => None,
    };
    let mut dicas = Vec::new();
    let p = |pares: &[(&'static str, String)]| -> BTreeMap<&'static str, String> {
        pares.iter().cloned().collect()
    };
    if dns_ok == Some(false) {
        dicas.push(Dica {
            codigo: "dns_falhou",
            params: p(&[("host", cam.host.clone())]),
        });
    }
    if let (Some(false), Some(c), Some(m)) = (mesma_rede, ip_cam, meu) {
        dicas.push(Dica {
            codigo: "fora_da_faixa",
            params: p(&[("camera", faixa(c)), ("pc", faixa(m))]),
        });
    }
    if let Some(novo) = &novo_ip {
        dicas.push(Dica {
            codigo: "mudou_ip",
            params: p(&[("de", cam.host.clone()), ("para", novo.clone())]),
        });
    }
    if !alcance {
        if dns_ok != Some(false) && novo_ip.is_none() && mesma_rede != Some(false) {
            dicas.push(Dica {
                codigo: "sem_resposta",
                params: p(&[("host", cam.host.clone()), ("porta", cam.porta.to_string())]),
            });
        }
    } else {
        if perda > 0 {
            dicas.push(Dica {
                codigo: "instavel",
                params: p(&[("perda", perda.to_string())]),
            });
        }
        if latencia_ms.is_some_and(|l| l > 200) {
            dicas.push(Dica {
                codigo: "lenta",
                params: p(&[("ms", latencia_ms.unwrap_or(0).to_string())]),
            });
        }
    }
    if gateway_ok == Some(false) {
        dicas.push(Dica {
            codigo: "gateway_fora",
            params: BTreeMap::new(),
        });
    }
    if dicas.is_empty() {
        dicas.push(Dica {
            codigo: "tudo_ok",
            params: BTreeMap::new(),
        });
    }
    Diagnostico {
        camera_id: cam.id,
        nome: cam.nome.clone(),
        host: cam.host.clone(),
        porta: cam.porta,
        alcance,
        latencia_ms,
        perda,
        mesma_rede,
        gateway_ok,
        dns_ok,
        mac,
        novo_ip,
        dicas,
    }
}

/// Uma tentativa de conexão TCP; devolve a latência em ms se abriu.
pub async fn sondar(ip: IpAddr, porta: u16, limite: Duration) -> Option<u32> {
    let t0 = Instant::now();
    match tokio::time::timeout(limite, TcpStream::connect(SocketAddr::new(ip, porta))).await {
        Ok(Ok(_)) => Some(t0.elapsed().as_millis().min(u32::MAX as u128) as u32),
        _ => None,
    }
}

/// Diagnóstico de uma câmera: 3 tentativas na porta RTSP, rede, gateway e DNS.
/// `macs_conhecidos`: MAC aprendido para esta câmera (da última foto/vigília).
pub async fn diagnosticar(
    cam: &CameraRede,
    meu: Option<Ipv4Addr>,
    gateway: Option<Ipv4Addr>,
    arp: &BTreeMap<String, String>,
    mac_conhecido: Option<&str>,
) -> Diagnostico {
    let (ip_cam, dns_ok) = match cam.host.parse::<IpAddr>() {
        Ok(ip) => (Some(ip), None),
        Err(_) => {
            let host = cam.host.clone();
            let r = tokio::task::spawn_blocking(move || {
                std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), 554))
                    .ok()
                    .and_then(|mut it| it.next())
                    .map(|s| s.ip())
            })
            .await
            .ok()
            .flatten();
            (r, Some(r.is_some()))
        }
    };
    let mut tentativas = Vec::with_capacity(3);
    if let Some(ip) = ip_cam {
        for _ in 0..3 {
            tentativas.push(sondar(ip, cam.porta, Duration::from_millis(1500)).await);
        }
    } else {
        tentativas = vec![None, None, None];
    }
    let gateway_ok = match gateway {
        Some(g) => {
            let na_arp = arp.contains_key(&g.to_string());
            Some(
                na_arp
                    || sondar(IpAddr::V4(g), 80, Duration::from_millis(800))
                        .await
                        .is_some()
                    || sondar(IpAddr::V4(g), 443, Duration::from_millis(800))
                        .await
                        .is_some(),
            )
        }
        None => None,
    };
    let ip_v4 = match ip_cam {
        Some(IpAddr::V4(v)) => Some(v),
        _ => None,
    };
    let mac = arp
        .get(&cam.host)
        .cloned()
        .or_else(|| mac_conhecido.map(String::from));
    let alcance = tentativas.iter().any(Option::is_some);
    let novo_ip = match (&mac, alcance) {
        (Some(m), false) => arp
            .iter()
            .find(|(ip, mm)| *mm == m && **ip != cam.host)
            .map(|(ip, _)| ip.clone()),
        _ => None,
    };
    avaliar(
        cam,
        ip_v4,
        meu,
        &tentativas,
        gateway_ok,
        dns_ok,
        mac,
        novo_ip,
    )
}

// ---------------- Armazenamento das fotos ----------------

#[derive(Debug, Clone, Serialize)]
pub struct FotoResumo {
    pub id: i64,
    pub quando: i64,
    pub dispositivos: usize,
}

impl crate::store::Store {
    fn garantir_fotos_rede(&self) -> crate::store::Result<()> {
        self.lock().execute_batch(
            "CREATE TABLE IF NOT EXISTS fotos_rede (
                 id     INTEGER PRIMARY KEY,
                 quando INTEGER NOT NULL,
                 json   TEXT NOT NULL
             );",
        )
    }

    /// Guarda uma foto; mantém só as 30 mais recentes.
    pub fn foto_rede_salvar(&self, foto: &Foto) -> crate::store::Result<i64> {
        self.garantir_fotos_rede()?;
        let json = serde_json::to_string(foto).unwrap_or_default();
        let c = self.lock();
        c.execute(
            "INSERT INTO fotos_rede(quando, json) VALUES (?1, ?2)",
            rusqlite::params![foto.quando, json],
        )?;
        let id = c.last_insert_rowid();
        c.execute(
            "DELETE FROM fotos_rede WHERE id <= (SELECT id FROM fotos_rede ORDER BY id DESC LIMIT 1 OFFSET 30)",
            [],
        )?;
        Ok(id)
    }

    /// Últimas `n` fotos, da mais nova para a mais velha.
    pub fn fotos_rede(&self, n: i64) -> crate::store::Result<Vec<(i64, Foto)>> {
        self.garantir_fotos_rede()?;
        let c = self.lock();
        let mut st = c.prepare("SELECT id, json FROM fotos_rede ORDER BY id DESC LIMIT ?1")?;
        let v = st
            .query_map(rusqlite::params![n], |r| {
                let id: i64 = r.get(0)?;
                let json: String = r.get(1)?;
                Ok((id, json))
            })?
            .filter_map(|r| r.ok())
            .filter_map(|(id, json)| serde_json::from_str::<Foto>(&json).ok().map(|f| (id, f)))
            .collect();
        Ok(v)
    }

    pub fn fotos_rede_resumo(&self, n: i64) -> crate::store::Result<Vec<FotoResumo>> {
        Ok(self
            .fotos_rede(n)?
            .into_iter()
            .map(|(id, f)| FotoResumo {
                id,
                quando: f.quando,
                dispositivos: f.dispositivos.len(),
            })
            .collect())
    }

    /// Troca a URL de uma câmera (usada quando ela muda de IP).
    pub fn atualizar_url_camera(&self, id: i64, url: &str) -> crate::store::Result<bool> {
        Ok(self.lock().execute(
            "UPDATE cameras SET url = ?2 WHERE id = ?1",
            rusqlite::params![id, url],
        )? > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disp(ip: &str, mac: Option<&str>) -> Dispositivo {
        Dispositivo {
            ip: ip.into(),
            mac: mac.map(String::from),
            fabricante: mac.and_then(fabricante_por_mac).map(String::from),
            modelo: None,
            tipo: Tipo::Desconhecido,
            portas: vec![],
            onvif: false,
            camera_id: None,
            camera_nome: None,
            eu: false,
            gateway: false,
        }
    }

    fn foto(d: Vec<Dispositivo>) -> Foto {
        Foto {
            quando: 0,
            rede: "192.168.1.0/24".into(),
            meu_ip: None,
            gateway: None,
            dispositivos: d,
        }
    }

    #[test]
    fn mac_e_fabricante() {
        assert_eq!(
            normalizar_mac("58-10-8c-aa-bb-cc").as_deref(),
            Some("58:10:8C:AA:BB:CC")
        );
        assert_eq!(
            normalizar_mac("a:b:c:d:e:f").as_deref(),
            Some("0A:0B:0C:0D:0E:0F")
        );
        assert_eq!(normalizar_mac("00:00:00:00:00:00"), None);
        assert_eq!(normalizar_mac("zz:10:8c:aa:bb:cc"), None);
        assert_eq!(fabricante_por_mac("58:10:8C:01:02:03"), Some("Intelbras"));
        assert_eq!(fabricante_por_mac("3c-ef-8c-01-02-03"), Some("Dahua"));
        assert_eq!(fabricante_por_mac("44:19:B6:00:00:01"), Some("Hikvision"));
        assert_eq!(
            fabricante_por_mac("B8:27:EB:00:00:01"),
            Some("Raspberry Pi")
        );
        assert_eq!(fabricante_por_mac("02:00:00:00:00:01"), None);
    }

    #[test]
    fn tipos() {
        assert_eq!(
            inferir_tipo(
                Some("Intelbras"),
                Some("VIP 1230"),
                &[80, 554],
                true,
                false,
                false
            ),
            Tipo::Camera
        );
        assert_eq!(
            inferir_tipo(
                Some("Intelbras"),
                Some("MHDX 1108"),
                &[80, 554, 37777],
                true,
                false,
                false
            ),
            Tipo::Gravador
        );
        assert_eq!(
            inferir_tipo(Some("TP-Link"), None, &[80], false, false, false),
            Tipo::Switch
        );
        assert_eq!(
            inferir_tipo(Some("TP-Link"), None, &[80], false, false, true),
            Tipo::Roteador
        );
        assert_eq!(
            inferir_tipo(None, None, &[554], false, false, false),
            Tipo::Camera
        );
        assert_eq!(
            inferir_tipo(None, None, &[], false, true, false),
            Tipo::Computador
        );
        assert_eq!(
            inferir_tipo(Some("Raspberry Pi"), None, &[80], false, false, false),
            Tipo::Computador
        );
        assert_eq!(
            inferir_tipo(None, None, &[80], false, false, false),
            Tipo::Desconhecido
        );
    }

    #[test]
    fn diferenca_entre_fotos() {
        let a = foto(vec![
            disp("192.168.1.108", Some("58:10:8C:00:00:01")),
            disp("192.168.1.50", Some("B8:27:EB:00:00:02")),
            disp("192.168.1.7", None),
        ]);
        let b = foto(vec![
            disp("192.168.1.120", Some("58:10:8C:00:00:01")),
            disp("192.168.1.60", Some("3C:EF:8C:00:00:03")),
            disp("192.168.1.8", None),
        ]);
        let m = comparar(&a, &b);
        assert!(m.contains(&Mudanca::MudouIp {
            mac: "58:10:8C:00:00:01".into(),
            de: "192.168.1.108".into(),
            para: "192.168.1.120".into(),
            fabricante: Some("Intelbras".into()),
            camera_nome: None,
        }));
        assert!(
            matches!(&m[..], _ if m.iter().any(|x| matches!(x, Mudanca::Apareceu { ip, .. } if ip == "192.168.1.60")))
        );
        assert!(m
            .iter()
            .any(|x| matches!(x, Mudanca::Sumiu { ip, .. } if ip == "192.168.1.50")));
        assert!(m
            .iter()
            .any(|x| matches!(x, Mudanca::Apareceu { ip, mac: None, .. } if ip == "192.168.1.8")));
        assert!(m
            .iter()
            .any(|x| matches!(x, Mudanca::Sumiu { ip, mac: None, .. } if ip == "192.168.1.7")));
        assert_eq!(m.len(), 5);
        assert!(comparar(&a, &a).is_empty());
    }

    #[test]
    fn host_e_troca() {
        let u = "rtsp://admin:se%40nha@192.168.1.108:554/cam/realmonitor?channel=1&subtype=1";
        assert_eq!(host_da_url(u), Some(("192.168.1.108".into(), 554)));
        assert_eq!(
            trocar_host(u, "192.168.1.120").as_deref(),
            Some("rtsp://admin:se%40nha@192.168.1.120:554/cam/realmonitor?channel=1&subtype=1")
        );
        assert_eq!(
            host_da_url("rtsp://cam.local/x"),
            Some(("cam.local".into(), 554))
        );
        assert_eq!(
            trocar_host("rtsp://10.0.0.5/live", "10.0.0.9").as_deref(),
            Some("rtsp://10.0.0.9/live")
        );
        assert_eq!(host_da_url("lixo"), None);
    }

    #[test]
    fn arp_windows_e_macos() {
        let win = "\nInterface: 192.168.1.10 --- 0xb\n  Endereço IP           Endereço físico       Tipo\n  192.168.1.1           aa-bb-cc-dd-ee-ff     dinâmico\n  192.168.1.255         ff-ff-ff-ff-ff-ff     estático\n  224.0.0.22            01-00-5e-00-00-16     estático\n";
        let m = parse_arp_a(win);
        assert_eq!(
            m.get("192.168.1.1").map(String::as_str),
            Some("AA:BB:CC:DD:EE:FF")
        );
        assert!(!m.contains_key("192.168.1.255"));
        let mac = "? (192.168.1.1) at a:bb:cc:dd:ee:ff on en0 ifscope [ethernet]\n? (192.168.1.108) at 58:10:8c:1:2:3 on en0 ifscope [ethernet]\n? (224.0.0.251) at 1:0:5e:0:0:fb on en0 ifscope permanent [ethernet]\n";
        let m = parse_arp_a(mac);
        assert_eq!(
            m.get("192.168.1.108").map(String::as_str),
            Some("58:10:8C:01:02:03")
        );
        assert_eq!(
            m.get("192.168.1.1").map(String::as_str),
            Some("0A:BB:CC:DD:EE:FF")
        );
    }

    #[test]
    fn rotas_padrao() {
        let proc = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\neth0\t00000000\t0101A8C0\t0003\t0\t0\t100\t00000000\t0\t0\t0\neth0\t0001A8C0\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0\n";
        assert_eq!(parse_proc_route(proc), Some(Ipv4Addr::new(192, 168, 1, 1)));
        let win = "Rotas ativas:\nEndereço de rede  Máscara  Gateway  Interface  Custo\n          0.0.0.0          0.0.0.0      192.168.0.1     192.168.0.15     35\n          0.0.0.0          0.0.0.0      10.0.0.1        10.0.0.2         50\n        127.0.0.0        255.0.0.0         No vínculo        127.0.0.1    331\n";
        assert_eq!(parse_route_print(win), Some(Ipv4Addr::new(192, 168, 0, 1)));
        let mac = "Routing tables\n\nInternet:\nDestination        Gateway            Flags           Netif Expire\ndefault            192.168.1.1        UGScg             en0\n127                127.0.0.1          UCS               lo0\n";
        assert_eq!(parse_netstat_rn(mac), Some(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(parse_netstat_rn("nada"), None);
    }

    #[test]
    fn monta_foto_com_camera_cadastrada() {
        let meu = Ipv4Addr::new(192, 168, 1, 10);
        let gw = Ipv4Addr::new(192, 168, 1, 1);
        let mut portas = BTreeMap::new();
        portas.insert(Ipv4Addr::new(192, 168, 1, 108), vec![554, 80]);
        portas.insert(Ipv4Addr::new(192, 168, 1, 2), vec![80]);
        let mut arp = BTreeMap::new();
        arp.insert("192.168.1.108".to_string(), "58:10:8C:00:00:01".to_string());
        arp.insert("192.168.1.2".to_string(), "50:C7:BF:00:00:02".to_string());
        arp.insert("192.168.1.1".to_string(), "74:58:89:00:00:03".to_string());
        arp.insert("10.0.0.9".to_string(), "00:E0:4C:00:00:04".to_string()); // outra rede: ignora
        let cams = vec![CameraRede {
            id: 7,
            nome: "Portão".into(),
            host: "192.168.1.108".into(),
            porta: 554,
        }];
        let f = montar_foto(Some(meu), Some(gw), &portas, &[], &arp, &cams, 1);
        assert_eq!(f.rede, "192.168.1.0/24");
        assert_eq!(f.dispositivos.len(), 4);
        let cam = f
            .dispositivos
            .iter()
            .find(|d| d.ip == "192.168.1.108")
            .unwrap();
        assert_eq!(cam.tipo, Tipo::Camera);
        assert_eq!(cam.camera_id, Some(7));
        assert_eq!(cam.fabricante.as_deref(), Some("Intelbras"));
        assert_eq!(cam.portas, vec![80, 554]);
        let sw = f
            .dispositivos
            .iter()
            .find(|d| d.ip == "192.168.1.2")
            .unwrap();
        assert_eq!(sw.tipo, Tipo::Switch);
        let r = f.dispositivos.iter().find(|d| d.gateway).unwrap();
        assert_eq!(r.tipo, Tipo::Roteador);
        assert_eq!(r.fabricante.as_deref(), Some("Multilaser"));
        assert!(f
            .dispositivos
            .iter()
            .find(|d| d.eu)
            .is_some_and(|d| d.tipo == Tipo::Computador));
    }

    #[test]
    fn dicas_do_diagnostico() {
        let cam = CameraRede {
            id: 1,
            nome: "Caixa".into(),
            host: "192.168.0.108".into(),
            porta: 554,
        };
        let meu = Some(Ipv4Addr::new(192, 168, 1, 10));
        let d = avaliar(
            &cam,
            Some(Ipv4Addr::new(192, 168, 0, 108)),
            meu,
            &[None, None, None],
            Some(true),
            None,
            None,
            None,
        );
        assert!(!d.alcance);
        assert_eq!(d.perda, 3);
        assert_eq!(d.dicas[0].codigo, "fora_da_faixa");
        assert_eq!(d.dicas[0].params["camera"], "192.168.0.x");
        assert_eq!(d.dicas[0].params["pc"], "192.168.1.x");
        assert!(!d.dicas.iter().any(|x| x.codigo == "sem_resposta"));

        let cam2 = CameraRede {
            host: "192.168.1.108".into(),
            ..cam.clone()
        };
        let d = avaliar(
            &cam2,
            Some(Ipv4Addr::new(192, 168, 1, 108)),
            meu,
            &[Some(12), None, Some(300)],
            Some(true),
            None,
            Some("58:10:8C:00:00:01".into()),
            None,
        );
        assert!(d.alcance);
        assert_eq!(d.latencia_ms, Some(156));
        assert_eq!(d.perda, 1);
        let cods: Vec<_> = d.dicas.iter().map(|x| x.codigo).collect();
        assert_eq!(cods, vec!["instavel"]);

        let d = avaliar(
            &cam2,
            Some(Ipv4Addr::new(192, 168, 1, 108)),
            meu,
            &[Some(5), Some(6), Some(7)],
            Some(true),
            None,
            None,
            None,
        );
        assert_eq!(d.dicas[0].codigo, "tudo_ok");

        let d = avaliar(
            &cam2,
            Some(Ipv4Addr::new(192, 168, 1, 108)),
            meu,
            &[None, None, None],
            Some(false),
            None,
            Some("58:10:8C:00:00:01".into()),
            Some("192.168.1.120".into()),
        );
        let cods: Vec<_> = d.dicas.iter().map(|x| x.codigo).collect();
        assert_eq!(cods, vec!["mudou_ip", "gateway_fora"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sonda_e_diagnostico_no_localhost() {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((s, _)) = l.accept().await else { return };
                drop(s);
            }
        });
        let cam = CameraRede {
            id: 1,
            nome: "Local".into(),
            host: "127.0.0.1".into(),
            porta,
        };
        let d = diagnosticar(&cam, None, None, &BTreeMap::new(), None).await;
        assert!(d.alcance);
        assert_eq!(d.perda, 0);
        assert_eq!(d.dicas[0].codigo, "tudo_ok");
        // Porta fechada.
        let l2 = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let fechada = l2.local_addr().unwrap().port();
        drop(l2);
        let cam2 = CameraRede {
            porta: fechada,
            ..cam
        };
        let d = diagnosticar(&cam2, None, None, &BTreeMap::new(), None).await;
        assert!(!d.alcance);
        assert_eq!(d.dicas[0].codigo, "sem_resposta");
    }

    #[test]
    fn guarda_fotos_e_troca_url() {
        let s = crate::store::Store::memory().unwrap();
        assert!(s.fotos_rede(5).unwrap().is_empty());
        let f = foto(vec![disp("192.168.1.108", Some("58:10:8C:00:00:01"))]);
        let id = s.foto_rede_salvar(&f).unwrap();
        let lista = s.fotos_rede(5).unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].0, id);
        assert_eq!(lista[0].1, f);
        for _ in 0..35 {
            s.foto_rede_salvar(&f).unwrap();
        }
        assert_eq!(s.fotos_rede(100).unwrap().len(), 30);
        assert_eq!(s.fotos_rede_resumo(1).unwrap()[0].dispositivos, 1);
        let cam = s
            .criar_camera("Portão", "rtsp", "rtsp://a:b@192.168.1.108:554/x")
            .unwrap();
        assert!(s
            .atualizar_url_camera(cam, "rtsp://a:b@192.168.1.120:554/x")
            .unwrap());
        assert_eq!(
            s.camera(cam).unwrap().unwrap().url,
            "rtsp://a:b@192.168.1.120:554/x"
        );
        assert!(!s.atualizar_url_camera(999, "x").unwrap());
    }
}
