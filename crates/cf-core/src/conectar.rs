//! Conexão rápida: acha câmeras na rede e liga com usuário e senha.
//!
//! O técnico só informa a senha. O CamFácil IA:
//! 1. varre a rede local atrás de quem responde RTSP (porta 554), além do ONVIF;
//! 2. confere usuário e senha com `DESCRIBE` autenticado (Digest ou Basic);
//! 3. descobre o caminho do vídeo testando os padrões conhecidos.
//!
//! Para não bloquear a câmera (muitas travam depois de 5 senhas erradas), uma
//! senha recusada encerra a tentativa na hora: nunca repetimos a mesma senha
//! em outros caminhos.

use md5::{Digest, Md5};
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Caminhos testados, em ordem. Começa pelo stream secundário (leve para PC
/// velho) e cai para o principal e para padrões genéricos.
const CAMINHOS: &[(&str, &str)] = &[
    ("intelbras", "/cam/realmonitor?channel=1&subtype=1"),
    ("hikvision", "/Streaming/Channels/102"),
    ("intelbras", "/cam/realmonitor?channel=1&subtype=0"),
    ("hikvision", "/Streaming/Channels/101"),
    ("generico", "/h264/ch1/sub/av_stream"),
    ("generico", "/live/ch1"),
    ("generico", "/stream2"),
    ("generico", "/stream1"),
    ("generico", "/"),
];

/// IPv4 desta máquina na rede local (sem mandar pacote: só consulta a rota).
pub fn ip_local() -> Option<Ipv4Addr> {
    let s = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // Qualquer endereço fora da rede serve; UDP `connect` não envia nada.
    s.connect((Ipv4Addr::new(10, 255, 255, 255), 9)).ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/// Varre a /24 da máquina procurando a porta RTSP aberta. Leva ~1 s.
pub async fn varrer_rtsp(porta: u16) -> Vec<String> {
    let Some(meu) = ip_local() else {
        return Vec::new();
    };
    let [a, b, c, _] = meu.octets();
    let mut tarefas = tokio::task::JoinSet::new();
    for d in 1..=254u8 {
        let ip = Ipv4Addr::new(a, b, c, d);
        if ip == meu {
            continue;
        }
        tarefas.spawn(async move {
            let alvo = SocketAddr::from((ip, porta));
            matches!(
                tokio::time::timeout(Duration::from_millis(700), TcpStream::connect(alvo)).await,
                Ok(Ok(_))
            )
            .then_some(ip)
        });
    }
    let mut achados = Vec::new();
    while let Some(r) = tarefas.join_next().await {
        if let Ok(Some(ip)) = r {
            achados.push(ip);
        }
    }
    achados.sort();
    achados.into_iter().map(|i| i.to_string()).collect()
}

#[derive(Debug, Serialize)]
pub struct Conectado {
    /// Endereço completo, com credenciais (guardado só na máquina).
    #[serde(skip)]
    pub url: String,
    pub url_sem_senha: String,
    pub fabricante: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ErroConexao {
    SemResposta,
    NaoERtsp,
    SenhaErrada,
    CaminhoNaoAchado,
}

impl std::fmt::Display for ErroConexao {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::SemResposta => {
                "A câmera não respondeu. Confira se o IP está certo e se ela está na mesma rede deste computador."
            }
            Self::NaoERtsp => "Esse endereço respondeu, mas não parece ser uma câmera (não fala RTSP).",
            Self::SenhaErrada => {
                "Usuário ou senha errados. Confira com calma: depois de algumas tentativas erradas a câmera pode bloquear o acesso por alguns minutos."
            }
            Self::CaminhoNaoAchado => {
                "A senha está certa, mas não achei o caminho do vídeo dessa marca. Use o gerador de endereço em Câmeras → modo avançado."
            }
        })
    }
}

/// Liga na câmera: confere a senha e descobre o caminho do vídeo.
pub async fn conectar(
    ip: &str,
    porta: u16,
    usuario: &str,
    senha: &str,
    fabricante: Option<&str>,
) -> Result<Conectado, ErroConexao> {
    let ip = ip.trim();
    if ip.is_empty() || ip.contains(['/', '@', ' ', '?', '#']) {
        return Err(ErroConexao::SemResposta);
    }
    // Se o ONVIF já disse a marca, testa os caminhos dela primeiro.
    let dica = fabricante.unwrap_or("").to_ascii_lowercase();
    let dica = match dica.as_str() {
        d if d.contains("intelbras") || d.contains("dahua") => "intelbras",
        d if d.contains("hik") => "hikvision",
        _ => "",
    };
    let mut ordem: Vec<&(&str, &str)> = CAMINHOS.iter().filter(|c| c.0 == dica).collect();
    ordem.extend(CAMINHOS.iter().filter(|c| c.0 != dica));

    let mut algum_respondeu = false;
    for (fab, caminho) in ordem {
        let uri = format!("rtsp://{ip}:{porta}{caminho}");
        match describe(ip, porta, &uri, usuario, senha).await {
            Resposta::Ok => {
                let cred = if usuario.is_empty() {
                    String::new()
                } else {
                    format!("{}:{}@", codificar(usuario), codificar(senha))
                };
                let url = format!("rtsp://{cred}{ip}:{porta}{caminho}");
                return Ok(Conectado {
                    url_sem_senha: crate::rtsp::redact(&url),
                    url,
                    fabricante: fab.to_string(),
                });
            }
            Resposta::SenhaRecusada => return Err(ErroConexao::SenhaErrada),
            Resposta::SemConexao if !algum_respondeu => return Err(ErroConexao::SemResposta),
            Resposta::NaoRtsp if !algum_respondeu => return Err(ErroConexao::NaoERtsp),
            _ => algum_respondeu = true,
        }
    }
    Err(ErroConexao::CaminhoNaoAchado)
}

#[derive(Debug, PartialEq, Eq)]
enum Resposta {
    Ok,
    SenhaRecusada,
    OutroCodigo(u16),
    NaoRtsp,
    SemConexao,
}

/// `DESCRIBE` com autenticação. Uma tentativa sem senha (para pegar o
/// desafio) e no máximo uma com senha.
async fn describe(ip: &str, porta: u16, uri: &str, usuario: &str, senha: &str) -> Resposta {
    let conn = tokio::time::timeout(Duration::from_secs(3), TcpStream::connect((ip, porta))).await;
    let Ok(Ok(mut s)) = conn else {
        return Resposta::SemConexao;
    };
    let (cod, cab) = match pedir(&mut s, uri, 1, None).await {
        Some(r) => r,
        None => return Resposta::NaoRtsp,
    };
    if cod != 401 {
        return classificar(cod);
    }
    let Some(auth) = autorizacao(&cab, uri, usuario, senha) else {
        return Resposta::SenhaRecusada;
    };
    match pedir(&mut s, uri, 2, Some(&auth)).await {
        Some((401, _)) => Resposta::SenhaRecusada,
        Some((c, _)) => classificar(c),
        None => Resposta::NaoRtsp,
    }
}

fn classificar(cod: u16) -> Resposta {
    match cod {
        200 => Resposta::Ok,
        401 => Resposta::SenhaRecusada,
        c => Resposta::OutroCodigo(c),
    }
}

/// Envia um DESCRIBE e devolve (código, cabeçalhos).
async fn pedir(
    s: &mut TcpStream,
    uri: &str,
    cseq: u32,
    auth: Option<&str>,
) -> Option<(u16, String)> {
    let mut req = format!(
        "DESCRIBE {uri} RTSP/1.0\r\nCSeq: {cseq}\r\nAccept: application/sdp\r\nUser-Agent: CamFacil-IA/{}\r\n",
        crate::VERSION
    );
    if let Some(a) = auth {
        req.push_str(&format!("Authorization: {a}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).await.ok()?;
    let mut buf = vec![0u8; 4096];
    let mut n = 0;
    // Lê até o fim dos cabeçalhos (o corpo SDP não interessa).
    while n < buf.len() {
        let lido = tokio::time::timeout(Duration::from_secs(4), s.read(&mut buf[n..]))
            .await
            .ok()?
            .ok()?;
        if lido == 0 {
            break;
        }
        n += lido;
        if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
            break;
        }
    }
    let texto = String::from_utf8_lossy(&buf[..n]).to_string();
    let mut l = texto.lines().next()?.split_whitespace();
    if l.next()? != "RTSP/1.0" {
        return None;
    }
    let cod = l.next()?.parse().ok()?;
    Some((cod, texto))
}

/// Monta o cabeçalho Authorization a partir do desafio WWW-Authenticate.
/// Prefere Digest (padrão das câmeras); usa Basic só se for o único.
fn autorizacao(cabecalhos: &str, uri: &str, usuario: &str, senha: &str) -> Option<String> {
    let desafios: Vec<&str> = cabecalhos
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("www-authenticate")
                .then_some(v.trim())
        })
        .collect();
    if let Some(d) = desafios
        .iter()
        .find(|d| d.len() > 6 && d[..6].eq_ignore_ascii_case("digest"))
    {
        let realm = campo(d, "realm")?;
        let nonce = campo(d, "nonce")?;
        let ha1 = md5_hex(&format!("{usuario}:{realm}:{senha}"));
        let ha2 = md5_hex(&format!("DESCRIBE:{uri}"));
        let resp = md5_hex(&format!("{ha1}:{nonce}:{ha2}"));
        return Some(format!(
            r#"Digest username="{usuario}", realm="{realm}", nonce="{nonce}", uri="{uri}", response="{resp}""#
        ));
    }
    if desafios
        .iter()
        .any(|d| d.len() > 5 && d[..5].eq_ignore_ascii_case("basic"))
    {
        use base64::Engine;
        let b = base64::engine::general_purpose::STANDARD.encode(format!("{usuario}:{senha}"));
        return Some(format!("Basic {b}"));
    }
    None
}

/// Lê `nome="valor"` de um desafio Digest.
fn campo(desafio: &str, nome: &str) -> Option<String> {
    let mut resto = desafio;
    while let Some(i) = resto.find(nome) {
        let antes = resto[..i].chars().last();
        let depois = resto[i + nome.len()..].trim_start();
        if matches!(antes, None | Some(' ' | ',')) && depois.starts_with('=') {
            let v = depois[1..].trim_start();
            return Some(match v.strip_prefix('"') {
                Some(q) => q.split('"').next()?.to_string(),
                None => v.split([',', ' ']).next()?.to_string(),
            });
        }
        resto = &resto[i + nome.len()..];
    }
    None
}

fn md5_hex(s: &str) -> String {
    Md5::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn codificar(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    #[test]
    fn le_campos_do_digest() {
        let d = r#"Digest realm="IP Camera(C1234)", nonce="abc123", stale="FALSE""#;
        assert_eq!(campo(d, "realm").as_deref(), Some("IP Camera(C1234)"));
        assert_eq!(campo(d, "nonce").as_deref(), Some("abc123"));
        assert_eq!(campo(d, "opaque"), None);
    }

    #[test]
    fn digest_rfc2069() {
        // Valor conferido à mão com o algoritmo MD5 do RFC 2069.
        let cab =
            "RTSP/1.0 401 Unauthorized\r\nWWW-Authenticate: Digest realm=\"r\", nonce=\"n\"\r\n";
        let a = autorizacao(cab, "rtsp://1.2.3.4:554/", "admin", "x").unwrap();
        let ha1 = md5_hex("admin:r:x");
        let ha2 = md5_hex("DESCRIBE:rtsp://1.2.3.4:554/");
        assert!(a.contains(&format!(
            "response=\"{}\"",
            md5_hex(&format!("{ha1}:n:{ha2}"))
        )));
        assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
    }

    /// Câmera falsa: exige Digest com admin/certa e só tem o caminho Hikvision.
    async fn camera_falsa() -> u16 {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = l.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    loop {
                        let n = match s.read(&mut buf).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => n,
                        };
                        let req = String::from_utf8_lossy(&buf[..n]).to_string();
                        let uri = req.split_whitespace().nth(1).unwrap_or("").to_string();
                        let resp = match req.lines().find(|l| l.starts_with("Authorization:")) {
                            None => "RTSP/1.0 401 Unauthorized\r\nCSeq: 1\r\nWWW-Authenticate: Digest realm=\"cam\", nonce=\"x1\"\r\n\r\n".to_string(),
                            Some(a) => {
                                let ha1 = md5_hex("admin:cam:certa");
                                let ha2 = md5_hex(&format!("DESCRIBE:{uri}"));
                                let ok = a.contains(&md5_hex(&format!("{ha1}:x1:{ha2}")));
                                if !ok {
                                    "RTSP/1.0 401 Unauthorized\r\nCSeq: 2\r\n\r\n".into()
                                } else if uri.ends_with("/Streaming/Channels/102") {
                                    "RTSP/1.0 200 OK\r\nCSeq: 2\r\nContent-Length: 0\r\n\r\n".into()
                                } else {
                                    "RTSP/1.0 404 Not Found\r\nCSeq: 2\r\n\r\n".into()
                                }
                            }
                        };
                        if s.write_all(resp.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        porta
    }

    #[tokio::test(flavor = "current_thread")]
    async fn acha_caminho_com_senha_certa() {
        let porta = camera_falsa().await;
        let c = conectar("127.0.0.1", porta, "admin", "certa", None)
            .await
            .unwrap();
        assert!(c.url.ends_with("/Streaming/Channels/102"));
        assert!(c.url.contains("admin:certa@"));
        assert!(!c.url_sem_senha.contains("certa"));
        assert_eq!(c.fabricante, "hikvision");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn senha_errada_para_na_primeira() {
        let porta = camera_falsa().await;
        let e = conectar("127.0.0.1", porta, "admin", "errada", Some("Intelbras"))
            .await
            .unwrap_err();
        assert_eq!(e, ErroConexao::SenhaErrada);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn porta_fechada() {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = l.local_addr().unwrap().port();
        drop(l);
        let e = conectar("127.0.0.1", porta, "admin", "x", None)
            .await
            .unwrap_err();
        assert_eq!(e, ErroConexao::SemResposta);
    }
}
