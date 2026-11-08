//! Gerador de endereço RTSP por fabricante e teste de conexão.
//!
//! Para adicionar um fabricante, use a skill `novo-fabricante-rtsp` e cite a
//! documentação oficial no PR.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Stream da câmera. A IA usa o secundário por padrão, porque é mais leve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Principal,
    #[default]
    Secundario,
    Extra,
}

/// Padrão de URL de um fabricante.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Vendor {
    pub id: &'static str,
    pub nome: &'static str,
    /// Marcas que usam o mesmo padrão.
    pub tambem: &'static [&'static str],
    #[serde(skip)]
    pattern: Pattern,
}

#[derive(Debug, Clone, Copy)]
enum Pattern {
    /// `/cam/realmonitor?channel=N&subtype=S` (0 principal, 1 secundário, 2 extra)
    Realmonitor,
    /// `/Streaming/Channels/NS` (N*100 + 1 principal, 2 secundário, 3 extra)
    StreamingChannels,
}

pub const VENDORS: &[Vendor] = &[
    Vendor {
        id: "intelbras",
        nome: "Intelbras",
        tambem: &["Dahua"],
        pattern: Pattern::Realmonitor,
    },
    Vendor {
        id: "dahua",
        nome: "Dahua",
        tambem: &["Intelbras"],
        pattern: Pattern::Realmonitor,
    },
    Vendor {
        id: "hikvision",
        nome: "Hikvision",
        tambem: &["HiLook"],
        pattern: Pattern::StreamingChannels,
    },
];

pub fn vendor(id: &str) -> Option<&'static Vendor> {
    let id = id.trim().to_ascii_lowercase();
    VENDORS.iter().find(|v| v.id == id)
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildRequest {
    pub fabricante: String,
    pub ip: String,
    #[serde(default = "default_port")]
    pub porta: u16,
    #[serde(default)]
    pub usuario: String,
    #[serde(default)]
    pub senha: String,
    #[serde(default = "default_channel")]
    pub canal: u32,
    #[serde(default)]
    pub stream: Stream,
}

fn default_port() -> u16 {
    554
}
fn default_channel() -> u32 {
    1
}

#[derive(Debug, PartialEq, Eq)]
pub enum BuildError {
    FabricanteDesconhecido,
    IpInvalido,
    CanalInvalido,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::FabricanteDesconhecido => "fabricante desconhecido",
            Self::IpInvalido => "endereço IP ou nome inválido",
            Self::CanalInvalido => "o canal precisa ser 1 ou maior",
        })
    }
}

/// Monta o endereço RTSP. Usuário e senha são codificados para URL.
pub fn build(req: &BuildRequest) -> Result<String, BuildError> {
    let v = vendor(&req.fabricante).ok_or(BuildError::FabricanteDesconhecido)?;
    let host = req.ip.trim();
    if host.is_empty() || host.contains(['/', '@', ' ', '?', '#']) {
        return Err(BuildError::IpInvalido);
    }
    if req.canal == 0 {
        return Err(BuildError::CanalInvalido);
    }
    let auth = if req.usuario.is_empty() {
        String::new()
    } else {
        format!("{}:{}@", encode(&req.usuario), encode(&req.senha))
    };
    let path = match v.pattern {
        Pattern::Realmonitor => {
            let sub = match req.stream {
                Stream::Principal => 0,
                Stream::Secundario => 1,
                Stream::Extra => 2,
            };
            format!("/cam/realmonitor?channel={}&subtype={}", req.canal, sub)
        }
        Pattern::StreamingChannels => {
            let s = match req.stream {
                Stream::Principal => 1,
                Stream::Secundario => 2,
                Stream::Extra => 3,
            };
            format!("/Streaming/Channels/{}", req.canal * 100 + s)
        }
    };
    Ok(format!("rtsp://{auth}{host}:{}{path}", req.porta))
}

/// Versão do endereço sem a senha, para mostrar na tela e em logs.
pub fn redact(url: &str) -> String {
    match (url.find("://"), url.find('@')) {
        (Some(s), Some(at)) if at > s => {
            let creds = &url[s + 3..at];
            let user = creds.split(':').next().unwrap_or("");
            format!("{}{}:***{}", &url[..s + 3], user, &url[at..])
        }
        _ => url.to_string(),
    }
}

/// Usuário e senha (opcional) ainda codificados, como vêm no endereço.
type Credenciais<'a> = (&'a str, Option<&'a str>);

/// Separa `esquema://`, as credenciais e o resto do endereço.
fn partes(url: &str) -> Option<(&str, Option<Credenciais<'_>>, &str)> {
    let s = url.find("://")? + 3;
    let resto = &url[s..];
    // O `@` das credenciais vem antes da primeira barra do caminho.
    let fim_host = resto.find('/').unwrap_or(resto.len());
    match resto[..fim_host].rfind('@') {
        Some(at) => {
            let creds = &resto[..at];
            let cred = match creds.split_once(':') {
                Some((u, p)) => (u, Some(p)),
                None => (creds, None),
            };
            Some((&url[..s], Some(cred), &resto[at + 1..]))
        }
        None => Some((&url[..s], None, resto)),
    }
}

/// Troca (ou põe) a senha do endereço. Mantém o usuário; sem usuário, usa `admin`.
pub fn trocar_senha(url: &str, senha: &str) -> String {
    match partes(url) {
        Some((esq, cred, resto)) => {
            let usuario = cred
                .map(|(u, _)| u)
                .filter(|u| !u.is_empty())
                .unwrap_or("admin");
            format!("{esq}{usuario}:{}@{resto}", encode(senha))
        }
        None => url.to_string(),
    }
}

/// Endereço novo vindo da tela de edição: a senha nunca sai do programa, então
/// a tela manda `***` (de [`redact`]) ou nada. Nesses casos a senha antiga fica.
pub fn manter_senha(nova: &str, antiga: &str) -> String {
    let senha_antiga = match partes(antiga) {
        Some((_, Some((u, Some(p))), _)) => Some((u, p)),
        _ => None,
    };
    let Some((u_antigo, p_antigo)) = senha_antiga else {
        return nova.to_string();
    };
    match partes(nova) {
        Some((esq, Some((u, Some("***"))), resto)) => format!("{esq}{u}:{p_antigo}@{resto}"),
        Some((esq, Some((u, None)), resto)) => format!("{esq}{u}:{p_antigo}@{resto}"),
        Some((esq, None, resto)) => format!("{esq}{u_antigo}:{p_antigo}@{resto}"),
        _ => nova.to_string(),
    }
}

/// Percent-encoding dos caracteres reservados em usuário e senha.
fn encode(s: &str) -> String {
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

#[derive(Debug, Serialize)]
pub struct TestResult {
    pub ok: bool,
    /// Código de status RTSP da resposta ao OPTIONS, se houve.
    pub status: Option<u16>,
    pub servidor: Option<String>,
    pub mensagem: String,
    pub ms: u128,
}

/// Teste rápido: abre TCP na porta e envia `OPTIONS`. Não decodifica vídeo.
pub async fn test(host: &str, port: u16) -> TestResult {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let start = std::time::Instant::now();
    let fail = |m: &str, start: std::time::Instant| TestResult {
        ok: false,
        status: None,
        servidor: None,
        mensagem: m.to_string(),
        ms: start.elapsed().as_millis(),
    };

    let addr = format!("{}:{}", host.trim(), port);
    let conn = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::net::TcpStream::connect(&addr),
    )
    .await;
    let mut stream = match conn {
        Ok(Ok(s)) => s,
        Ok(Err(_)) => return fail("A câmera recusou a conexão nesta porta.", start),
        Err(_) => {
            return fail(
                "Sem resposta. Confira o IP, a porta e se a câmera está na mesma rede.",
                start,
            )
        }
    };

    let req = format!(
        "OPTIONS rtsp://{addr}/ RTSP/1.0\r\nCSeq: 1\r\nUser-Agent: CamFacil-IA/{}\r\n\r\n",
        crate::VERSION
    );
    if stream.write_all(req.as_bytes()).await.is_err() {
        return fail("A conexão caiu ao enviar o teste.", start);
    }
    let mut buf = [0u8; 1024];
    let n = match tokio::time::timeout(Duration::from_secs(3), stream.read(&mut buf)).await {
        Ok(Ok(n)) if n > 0 => n,
        _ => return fail("A porta abriu, mas não respondeu como RTSP.", start),
    };
    let text = String::from_utf8_lossy(&buf[..n]);
    let (status, servidor) = parse_response(&text);
    let ok = matches!(status, Some(200) | Some(401));
    TestResult {
        ok,
        status,
        servidor,
        mensagem: match status {
            Some(200) => "Servidor RTSP respondeu. Endereço pronto para usar.".into(),
            Some(401) => "Servidor RTSP respondeu e pede senha (normal).".into(),
            Some(c) => format!("Servidor RTSP respondeu com o código {c}."),
            None => "A porta respondeu, mas não parece ser RTSP.".into(),
        },
        ms: start.elapsed().as_millis(),
    }
}

fn parse_response(text: &str) -> (Option<u16>, Option<String>) {
    let mut lines = text.lines();
    let status = lines.next().and_then(|l| {
        let mut p = l.split_whitespace();
        (p.next()? == "RTSP/1.0").then_some(())?;
        p.next()?.parse().ok()
    });
    let servidor = text.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.eq_ignore_ascii_case("server")
            .then(|| v.trim().to_string())
    });
    (status, servidor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn senha_mantida_na_edicao() {
        let antiga = "rtsp://admin:s3%40nh4@192.168.1.10:554/cam/realmonitor?channel=1";
        let tela = redact(antiga);
        assert!(!tela.contains("s3%40nh4"));
        // Mudou só o IP: a senha antiga continua.
        let nova = tela.replace("192.168.1.10", "192.168.1.20");
        assert_eq!(
            manter_senha(&nova, antiga),
            "rtsp://admin:s3%40nh4@192.168.1.20:554/cam/realmonitor?channel=1"
        );
        // Sem credenciais na tela: reaproveita usuário e senha.
        assert_eq!(
            manter_senha("rtsp://10.0.0.5:554/x", antiga),
            "rtsp://admin:s3%40nh4@10.0.0.5:554/x"
        );
        // Senha nova escrita na própria URL vale.
        assert_eq!(
            manter_senha("rtsp://op:nova@10.0.0.5/x", antiga),
            "rtsp://op:nova@10.0.0.5/x"
        );
        // Câmera sem senha antes: nada a manter.
        assert_eq!(manter_senha("rtsp://h/x", "rtsp://h/y"), "rtsp://h/x");
    }

    #[test]
    fn troca_senha_codificada() {
        assert_eq!(
            trocar_senha("rtsp://op:velha@h:554/a@b", "n@va:1"),
            "rtsp://op:n%40va%3A1@h:554/a@b"
        );
        assert_eq!(trocar_senha("rtsp://h/x", "p"), "rtsp://admin:p@h/x");
    }

    fn req(fab: &str, canal: u32, stream: Stream) -> BuildRequest {
        BuildRequest {
            fabricante: fab.into(),
            ip: "192.168.1.108".into(),
            porta: 554,
            usuario: "admin".into(),
            senha: "s3nh@#1".into(),
            canal,
            stream,
        }
    }

    #[test]
    fn intelbras_dahua() {
        assert_eq!(
            build(&req("intelbras", 1, Stream::Principal)).unwrap(),
            "rtsp://admin:s3nh%40%231@192.168.1.108:554/cam/realmonitor?channel=1&subtype=0"
        );
        assert_eq!(
            build(&req("Dahua", 3, Stream::Secundario)).unwrap(),
            "rtsp://admin:s3nh%40%231@192.168.1.108:554/cam/realmonitor?channel=3&subtype=1"
        );
        assert!(build(&req("intelbras", 1, Stream::Extra))
            .unwrap()
            .ends_with("subtype=2"));
    }

    #[test]
    fn hikvision() {
        assert!(build(&req("hikvision", 1, Stream::Principal))
            .unwrap()
            .ends_with("/Streaming/Channels/101"));
        assert!(build(&req("hikvision", 1, Stream::Secundario))
            .unwrap()
            .ends_with("/Streaming/Channels/102"));
        assert!(build(&req("hikvision", 12, Stream::Extra))
            .unwrap()
            .ends_with("/Streaming/Channels/1203"));
    }

    #[test]
    fn sem_usuario() {
        let mut r = req("hikvision", 1, Stream::Secundario);
        r.usuario.clear();
        assert_eq!(
            build(&r).unwrap(),
            "rtsp://192.168.1.108:554/Streaming/Channels/102"
        );
    }

    #[test]
    fn erros() {
        assert_eq!(
            build(&req("marca-x", 1, Stream::Principal)),
            Err(BuildError::FabricanteDesconhecido)
        );
        assert_eq!(
            build(&req("intelbras", 0, Stream::Principal)),
            Err(BuildError::CanalInvalido)
        );
        let mut r = req("intelbras", 1, Stream::Principal);
        r.ip = "1.2.3.4/x".into();
        assert_eq!(build(&r), Err(BuildError::IpInvalido));
    }

    #[test]
    fn esconde_senha() {
        let url = build(&req("intelbras", 1, Stream::Principal)).unwrap();
        let r = redact(&url);
        assert!(r.starts_with("rtsp://admin:***@192.168.1.108"));
        assert!(!r.contains("s3nh"));
    }

    #[test]
    fn le_resposta_rtsp() {
        let (s, srv) =
            parse_response("RTSP/1.0 401 Unauthorized\r\nCSeq: 1\r\nServer: Rtsp Server/3.0\r\n");
        assert_eq!(s, Some(401));
        assert_eq!(srv.as_deref(), Some("Rtsp Server/3.0"));
        assert_eq!(parse_response("HTTP/1.1 200 OK\r\n").0, None);
    }
}
