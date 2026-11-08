//! Autenticação HTTP Digest (RFC 7616, MD5 com e sem `qop=auth`) e um GET
//! HTTP mínimo que a usa. É o que DVR/NVR Intelbras/Dahua/Hikvision pedem nas
//! APIs CGI/ISAPI. Sem dependência nova: o MD5 já vinha pelo RTSP.
//!
//! Também usado pelos botões de relé (`camfacil::botoes`), que montam o
//! cabeçalho a partir do desafio que o reqwest recebeu.

use md5::{Digest, Md5};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Desafio `WWW-Authenticate: Digest …` já lido.
#[derive(Debug, Clone, PartialEq)]
pub struct Desafio {
    pub realm: String,
    pub nonce: String,
    pub opaque: Option<String>,
    /// O servidor aceita `qop=auth` (se não, é o Digest antigo da RFC 2069).
    pub qop_auth: bool,
    /// `MD5-sess` em vez de `MD5`.
    pub sess: bool,
}

/// Lê `nome="valor"` (ou `nome=valor`) de um desafio.
pub fn campo(desafio: &str, nome: &str) -> Option<String> {
    let mut resto = desafio;
    while let Some(i) = find_ci(resto, nome) {
        let antes = resto[..i].chars().last();
        let depois = resto[i + nome.len()..].trim_start();
        if matches!(antes, None | Some(' ' | ',' | '\t')) && depois.starts_with('=') {
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

fn find_ci(s: &str, agulha: &str) -> Option<usize> {
    let a = agulha.as_bytes();
    s.as_bytes()
        .windows(a.len())
        .position(|w| w.eq_ignore_ascii_case(a))
}

impl Desafio {
    /// Lê o valor de um cabeçalho `WWW-Authenticate`. `None` se não for
    /// Digest MD5 (SHA-256 sozinho não é aceito: os aparelhos sempre
    /// oferecem MD5 junto).
    pub fn ler(valor: &str) -> Option<Self> {
        let v = valor.trim();
        if v.len() < 7 || !v[..6].eq_ignore_ascii_case("digest") {
            return None;
        }
        let alg = campo(v, "algorithm").unwrap_or_else(|| "MD5".into());
        let sess = alg.eq_ignore_ascii_case("MD5-sess");
        if !sess && !alg.eq_ignore_ascii_case("MD5") {
            return None;
        }
        let qop_auth = campo(v, "qop")
            .is_some_and(|q| q.split(',').any(|x| x.trim().eq_ignore_ascii_case("auth")));
        Some(Self {
            realm: campo(v, "realm")?,
            nonce: campo(v, "nonce")?,
            opaque: campo(v, "opaque"),
            qop_auth,
            sess,
        })
    }

    /// Escolhe o primeiro desafio Digest MD5 entre vários cabeçalhos.
    pub fn escolher<'a>(valores: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        valores.into_iter().find_map(Self::ler)
    }

    /// Só o `response` (útil para conferir com os vetores das RFCs).
    pub fn resposta(
        &self,
        metodo: &str,
        uri: &str,
        usuario: &str,
        senha: &str,
        cnonce: &str,
        nc: u32,
    ) -> String {
        let mut ha1 = md5_hex(&format!("{usuario}:{}:{senha}", self.realm));
        if self.sess {
            ha1 = md5_hex(&format!("{ha1}:{}:{cnonce}", self.nonce));
        }
        let ha2 = md5_hex(&format!("{metodo}:{uri}"));
        if self.qop_auth {
            md5_hex(&format!(
                "{ha1}:{}:{nc:08x}:{cnonce}:auth:{ha2}",
                self.nonce
            ))
        } else {
            md5_hex(&format!("{ha1}:{}:{ha2}", self.nonce))
        }
    }

    /// Valor do cabeçalho `Authorization`.
    pub fn autorizacao(
        &self,
        metodo: &str,
        uri: &str,
        usuario: &str,
        senha: &str,
        cnonce: &str,
        nc: u32,
    ) -> String {
        let resp = self.resposta(metodo, uri, usuario, senha, cnonce, nc);
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let mut h = format!(
            r#"Digest username="{}", realm="{}", nonce="{}", uri="{}", response="{resp}""#,
            esc(usuario),
            esc(&self.realm),
            esc(&self.nonce),
            uri
        );
        h.push_str(if self.sess {
            ", algorithm=MD5-sess"
        } else {
            ", algorithm=MD5"
        });
        if self.qop_auth {
            h.push_str(&format!(r#", qop=auth, nc={nc:08x}, cnonce="{cnonce}""#));
        }
        if let Some(o) = &self.opaque {
            h.push_str(&format!(r#", opaque="{}""#, esc(o)));
        }
        h
    }
}

pub fn md5_hex(s: &str) -> String {
    Md5::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `cnonce` aleatório (16 bytes em hex).
pub fn novo_cnonce() -> String {
    let mut b = [0u8; 16];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `Authorization` para um conjunto de desafios: Digest se houver, Basic se
/// for o único. `None` se não há como responder.
pub fn autorizacao_para(
    desafios: &[&str],
    metodo: &str,
    uri: &str,
    usuario: &str,
    senha: &str,
) -> Option<String> {
    if let Some(d) = Desafio::escolher(desafios.iter().copied()) {
        return Some(d.autorizacao(metodo, uri, usuario, senha, &novo_cnonce(), 1));
    }
    if desafios
        .iter()
        .any(|d| d.trim().len() >= 5 && d.trim()[..5].eq_ignore_ascii_case("basic"))
    {
        use base64::Engine;
        let b = base64::engine::general_purpose::STANDARD.encode(format!("{usuario}:{senha}"));
        return Some(format!("Basic {b}"));
    }
    None
}

// ---------------- GET HTTP mínimo (só http://, sem TLS) ----------------

/// Resposta crua: código, cabeçalhos (texto) e corpo.
struct Resposta {
    codigo: u16,
    cabecalhos: String,
    corpo: String,
}

/// Corpo máximo lido (as listas de canais são pequenas).
const MAX_CORPO: usize = 512 * 1024;

async fn pedir(
    ip: &str,
    porta: u16,
    caminho: &str,
    auth: Option<&str>,
    limite: Duration,
) -> Option<Resposta> {
    let mut s = tokio::time::timeout(limite, TcpStream::connect((ip, porta)))
        .await
        .ok()?
        .ok()?;
    let mut req = format!(
        "GET {caminho} HTTP/1.1\r\nHost: {ip}:{porta}\r\nUser-Agent: CamFacilIA\r\nAccept: */*\r\nConnection: close\r\n"
    );
    if let Some(a) = auth {
        req.push_str(&format!("Authorization: {a}\r\n"));
    }
    req.push_str("\r\n");
    s.write_all(req.as_bytes()).await.ok()?;
    let mut buf = Vec::new();
    let mut parte = [0u8; 8192];
    loop {
        let n = tokio::time::timeout(limite, s.read(&mut parte))
            .await
            .ok()?
            .ok()?;
        if n == 0 || buf.len() > MAX_CORPO {
            break;
        }
        buf.extend_from_slice(&parte[..n]);
    }
    let texto = String::from_utf8_lossy(&buf).to_string();
    let (cab, corpo) = texto.split_once("\r\n\r\n")?;
    let codigo = cab
        .strip_prefix("HTTP/1.")?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let chunked = cab
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked");
    Some(Resposta {
        codigo,
        cabecalhos: cab.to_string(),
        corpo: if chunked {
            desfazer_chunked(corpo)
        } else {
            corpo.to_string()
        },
    })
}

/// Valores de `WWW-Authenticate` nos cabeçalhos.
pub fn desafios_dos_cabecalhos(cab: &str) -> Vec<&str> {
    cab.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("www-authenticate")
                .then_some(v.trim())
        })
        .collect()
}

/// `GET http://ip:porta/caminho` com login (Digest ou Basic, conforme o
/// desafio). Devolve o corpo só se a resposta for 2xx. `limite` vale para
/// cada etapa (conectar, ler); o chamador põe o teto total.
pub async fn get(
    ip: &str,
    porta: u16,
    caminho: &str,
    usuario: &str,
    senha: &str,
    limite: Duration,
) -> Option<String> {
    let r = pedir(ip, porta, caminho, None, limite).await?;
    let r = if r.codigo == 401 {
        let desafios = desafios_dos_cabecalhos(&r.cabecalhos);
        let auth = autorizacao_para(&desafios, "GET", caminho, usuario, senha)?;
        pedir(ip, porta, caminho, Some(&auth), limite).await?
    } else {
        r
    };
    (200..300).contains(&r.codigo).then_some(r.corpo)
}

pub fn desfazer_chunked(mut s: &str) -> String {
    let mut out = String::new();
    while let Some((tam, resto)) = s.split_once("\r\n") {
        let Ok(n) = usize::from_str_radix(tam.trim().split(';').next().unwrap_or(""), 16) else {
            break;
        };
        if n == 0 || resto.len() < n {
            out.push_str(&resto[..n.min(resto.len())]);
            break;
        }
        out.push_str(&resto[..n]);
        s = resto[n..].trim_start_matches("\r\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 2617 §3.5 (MD5, qop=auth).
    #[test]
    fn vetor_rfc_2617() {
        let d = Desafio::ler(
            r#"Digest realm="testrealm@host.com", qop="auth,auth-int", nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093", opaque="5ccc069c403ebaf9f0171e9517f40e41""#,
        )
        .unwrap();
        assert!(d.qop_auth && !d.sess);
        assert_eq!(
            d.resposta(
                "GET",
                "/dir/index.html",
                "Mufasa",
                "Circle Of Life",
                "0a4f113b",
                1
            ),
            "6629fae49393a05397450978507c4ef1"
        );
        let h = d.autorizacao(
            "GET",
            "/dir/index.html",
            "Mufasa",
            "Circle Of Life",
            "0a4f113b",
            1,
        );
        assert!(h.contains("nc=00000001"));
        assert!(h.contains(r#"opaque="5ccc069c403ebaf9f0171e9517f40e41""#));
    }

    /// RFC 7616 §3.9.1 (MD5).
    #[test]
    fn vetor_rfc_7616_md5() {
        let d = Desafio::ler(
            r#"Digest realm="http-auth@example.org", qop="auth, auth-int", algorithm=MD5, nonce="7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v", opaque="FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS""#,
        )
        .unwrap();
        assert_eq!(
            d.resposta(
                "GET",
                "/dir/index.html",
                "Mufasa",
                "Circle of Life",
                "f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ",
                1
            ),
            "8ca523f5e9506fed4657c9700eebdbec"
        );
    }

    #[test]
    fn desafios_variados() {
        // Só SHA-256: não respondemos.
        assert!(Desafio::ler(r#"Digest realm="r", nonce="n", algorithm=SHA-256"#).is_none());
        assert!(Desafio::ler(r#"Basic realm="r""#).is_none());
        // Sem qop: RFC 2069.
        let d = Desafio::ler(r#"Digest realm="r", nonce="n""#).unwrap();
        assert!(!d.qop_auth);
        assert_eq!(
            d.resposta("GET", "/", "a", "b", "x", 1),
            md5_hex(&format!("{}:n:{}", md5_hex("a:r:b"), md5_hex("GET:/")))
        );
        // Vários cabeçalhos: escolhe o MD5.
        let e = Desafio::escolher([
            r#"Digest realm="r", nonce="s", algorithm=SHA-256, qop="auth""#,
            r#"Digest realm="r", nonce="m", algorithm=MD5, qop="auth""#,
        ])
        .unwrap();
        assert_eq!(e.nonce, "m");
        assert!(
            autorizacao_para(&[r#"Basic realm="x""#], "GET", "/", "a", "b")
                .unwrap()
                .starts_with("Basic ")
        );
    }

    /// Servidor HTTP falso que só aceita Digest com o login certo.
    async fn servidor_digest(corpo: &'static str) -> u16 {
        use tokio::net::TcpListener;
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let Ok(n) = s.read(&mut buf).await else {
                        return;
                    };
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let uri = req.split_whitespace().nth(1).unwrap_or("").to_string();
                    let auth = req
                        .lines()
                        .find_map(|l| l.strip_prefix("Authorization: "))
                        .unwrap_or("");
                    let ok = auth.starts_with("Digest ") && {
                        let d = Desafio {
                            realm: "Login to NVR".into(),
                            nonce: "abc123".into(),
                            opaque: None,
                            qop_auth: true,
                            sess: false,
                        };
                        let cn = campo(auth, "cnonce").unwrap_or_default();
                        let nc = u32::from_str_radix(&campo(auth, "nc").unwrap_or_default(), 16)
                            .unwrap_or(0);
                        campo(auth, "response").as_deref()
                            == Some(&d.resposta("GET", &uri, "admin", "s3nh@", &cn, nc)[..])
                            && campo(auth, "uri").as_deref() == Some(&uri[..])
                    };
                    let r = if ok {
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{corpo}\r\n0\r\n\r\n",
                            corpo.len()
                        )
                    } else {
                        "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Digest realm=\"Login to NVR\", qop=\"auth\", nonce=\"abc123\", opaque=\"\"\r\nContent-Length: 0\r\n\r\n".to_string()
                    };
                    let _ = s.write_all(r.as_bytes()).await;
                });
            }
        });
        porta
    }

    #[tokio::test(flavor = "current_thread")]
    async fn get_com_digest_em_servidor_falso() {
        let porta = servidor_digest("table.ChannelTitle[0].Name=PORTAO").await;
        let lim = Duration::from_secs(2);
        let c = "/cgi-bin/configManager.cgi?action=getConfig&name=ChannelTitle";
        let ok = get("127.0.0.1", porta, c, "admin", "s3nh@", lim).await;
        assert_eq!(ok.as_deref(), Some("table.ChannelTitle[0].Name=PORTAO"));
        assert_eq!(
            get("127.0.0.1", porta, c, "admin", "errada", lim).await,
            None
        );
    }

    #[test]
    fn chunked() {
        assert_eq!(
            desfazer_chunked("4\r\nabcd\r\n2\r\nef\r\n0\r\n\r\n"),
            "abcdef"
        );
    }
}
