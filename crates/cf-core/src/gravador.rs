//! Gravador (DVR/NVR) ou câmera? E quantos canais?
//!
//! Depois que a senha confere no canal 1, o programa descobre se o aparelho
//! é um gravador, nesta ordem:
//! 1. ONVIF com login (`GetVideoSources`/`GetProfiles`): mais de uma fonte de
//!    vídeo é gravador, e os perfis dão o nome dos canais;
//! 2. modelo visto na busca ONVIF com cara de gravador (NVR, DVR, XVR, MHDX…);
//! 3. `DESCRIBE` rápido no canal 2 e seguintes, com o mesmo login, em
//!    paralelo moderado (4 por vez) e tempo curto.
//!
//! Só vale para os padrões de caminho com canal (Intelbras/Dahua
//! `cam/realmonitor` e Hikvision `Streaming/Channels`).

use crate::onvif::tag_values;
use crate::rtsp::{self, Stream};
use serde::Serialize;
use std::future::Future;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Teto de canais sondados.
pub const MAX_CANAIS: u32 = 64;
/// Sondas de canal em paralelo.
const PARALELO: u32 = 4;
/// Tempo máximo de cada sonda de canal.
const LIMITE_SONDA: Duration = Duration::from_millis(2500);

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CanalGravador {
    pub canal: u32,
    /// Nome dado pelo ONVIF, quando há.
    pub nome: Option<String>,
    /// Respondeu ao `DESCRIBE` com o login informado.
    pub responde: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Deteccao {
    pub gravador: bool,
    pub canais: Vec<CanalGravador>,
    /// `onvif`, `modelo`, `rtsp` ou `camera`.
    pub origem: &'static str,
}

impl Deteccao {
    fn camera() -> Self {
        Self {
            gravador: false,
            canais: Vec::new(),
            origem: "camera",
        }
    }
}

/// O texto (modelo/nome do ONVIF) tem cara de gravador?
pub fn parece_gravador(texto: &str) -> bool {
    let t = texto.to_ascii_uppercase();
    const MARCAS: &[&str] = &[
        "NVR", "DVR", "XVR", "HCVR", "MHDX", "NVD", "IMHDX", "DS-71", "DS-72", "DS-73", "DS-76",
        "DS-77", "DS-78", "DS-79", "DS-86", "DS-96", "IDS-72", "IDS-73", "IDS-77", "IDS-96",
        "RECORDER", "GRAVADOR",
    ];
    MARCAS.iter().any(|m| t.contains(m))
}

// ---------------- Sonda de canais por RTSP ----------------

/// Abstração da sonda, para testar sem aparelho.
pub trait Sonda: Sync {
    fn responde(&self, caminho: String) -> impl Future<Output = bool> + Send;
}

/// Sonda de verdade: `DESCRIBE` autenticado.
pub struct SondaRtsp<'a> {
    pub ip: &'a str,
    pub porta: u16,
    pub usuario: &'a str,
    pub senha: &'a str,
}

impl Sonda for SondaRtsp<'_> {
    async fn responde(&self, caminho: String) -> bool {
        tokio::time::timeout(
            LIMITE_SONDA,
            crate::conectar::caminho_responde(
                self.ip,
                self.porta,
                &caminho,
                self.usuario,
                self.senha,
            ),
        )
        .await
        .unwrap_or(false)
    }
}

/// Sonda os canais `de..=ate` (stream secundário), 4 por vez. Com
/// `parar_cedo`, para quando um lote inteiro não responde.
pub async fn sondar_canais<S: Sonda>(
    s: &S,
    fabricante: &str,
    de: u32,
    ate: u32,
    parar_cedo: bool,
) -> Vec<(u32, bool)> {
    let mut out = Vec::new();
    let mut c = de.max(1);
    let ate = ate.min(MAX_CANAIS);
    while c <= ate {
        let fim = (c + PARALELO - 1).min(ate);
        let lote: Vec<u32> = (c..=fim).collect();
        let res = futuros_juntos(lote.iter().map(|&n| {
            let caminho = rtsp::caminho_canal(fabricante, n, Stream::Secundario);
            async move {
                match caminho {
                    Some(cm) => (n, s.responde(cm).await),
                    None => (n, false),
                }
            }
        }))
        .await;
        let algum = res.iter().any(|(_, ok)| *ok);
        out.extend(res);
        if parar_cedo && !algum {
            break;
        }
        c = fim + 1;
    }
    out
}

/// Roda os futuros juntos (sem `spawn`: as sondas emprestam a referência).
async fn futuros_juntos<F, T>(fs: impl Iterator<Item = F>) -> Vec<T>
where
    F: Future<Output = T>,
{
    let mut fs: Vec<std::pin::Pin<Box<F>>> = fs.map(Box::pin).collect();
    let mut prontos: Vec<Option<T>> = fs.iter().map(|_| None).collect();
    std::future::poll_fn(|cx| {
        let mut todos = true;
        for (i, f) in fs.iter_mut().enumerate() {
            if prontos[i].is_none() {
                match f.as_mut().poll(cx) {
                    std::task::Poll::Ready(v) => prontos[i] = Some(v),
                    std::task::Poll::Pending => todos = false,
                }
            }
        }
        if todos {
            std::task::Poll::Ready(())
        } else {
            std::task::Poll::Pending
        }
    })
    .await;
    prontos.into_iter().flatten().collect()
}

/// Decide gravador × câmera. `onvif`: nomes das fontes de vídeo (se o ONVIF
/// respondeu); `modelo`: o que a busca viu.
pub async fn decidir<S: Sonda>(
    s: &S,
    url_conectada: &str,
    onvif: Option<Vec<Option<String>>>,
    modelo: Option<&str>,
) -> Deteccao {
    let Some(fab) = rtsp::fabricante_do_caminho(url_conectada) else {
        return Deteccao::camera();
    };
    let montar = |sondados: Vec<(u32, bool)>, total: u32, nomes: &[Option<String>]| {
        (1..=total)
            .map(|n| CanalGravador {
                canal: n,
                nome: nomes.get(n as usize - 1).cloned().flatten(),
                // O canal 1 já conectou.
                responde: n == 1 || sondados.iter().any(|(c, ok)| *c == n && *ok),
            })
            .collect::<Vec<_>>()
    };
    // 1. ONVIF com mais de uma fonte de vídeo.
    if let Some(nomes) = onvif.filter(|v| v.len() > 1) {
        let total = (nomes.len() as u32).min(MAX_CANAIS);
        let sondados = sondar_canais(s, fab, 2, total, false).await;
        return Deteccao {
            gravador: true,
            canais: montar(sondados, total, &nomes),
            origem: "onvif",
        };
    }
    // 2. Modelo de gravador: sonda até parar de responder (mínimo 4 canais).
    if modelo.is_some_and(parece_gravador) {
        let sondados = sondar_canais(s, fab, 2, MAX_CANAIS, true).await;
        let maior = sondados
            .iter()
            .filter(|(_, ok)| *ok)
            .map(|(c, _)| *c)
            .max()
            .unwrap_or(1);
        return Deteccao {
            gravador: true,
            canais: montar(sondados, maior.max(4), &[]),
            origem: "modelo",
        };
    }
    // 3. Canal 2 responde? Então é gravador; sonda o resto.
    let sondados = sondar_canais(s, fab, 2, MAX_CANAIS, true).await;
    let maior = sondados.iter().filter(|(_, ok)| *ok).map(|(c, _)| *c).max();
    match maior {
        Some(m) if sondados.iter().any(|(c, ok)| *c == 2 && *ok) => Deteccao {
            gravador: true,
            canais: montar(sondados, m, &[]),
            origem: "rtsp",
        },
        _ => Deteccao::camera(),
    }
}

/// Detecção completa com a rede de verdade (ONVIF na porta 80 + RTSP).
pub async fn detectar(
    ip: &str,
    porta: u16,
    usuario: &str,
    senha: &str,
    url_conectada: &str,
    modelo: Option<&str>,
) -> Deteccao {
    if rtsp::fabricante_do_caminho(url_conectada).is_none() {
        return Deteccao::camera();
    }
    let onvif = tokio::time::timeout(Duration::from_secs(6), canais_onvif(ip, 80, usuario, senha))
        .await
        .ok()
        .flatten();
    let s = SondaRtsp {
        ip,
        porta,
        usuario,
        senha,
    };
    decidir(&s, url_conectada, onvif, modelo).await
}

// ---------------- ONVIF com login (SOAP) ----------------

/// Data UTC no formato do WS-Security (`2026-11-08T12:00:00Z`).
fn data_utc(unix: i64) -> String {
    let dias = unix.div_euclid(86_400);
    let seg = unix.rem_euclid(86_400);
    // Algoritmo civil_from_days (Howard Hinnant).
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let a = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{a:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        seg / 3600,
        (seg % 3600) / 60,
        seg % 60
    )
}

/// Cabeçalho WS-Security UsernameToken (PasswordDigest).
pub fn usuario_token(usuario: &str, senha: &str, nonce: &[u8], criado: &str) -> String {
    use base64::Engine;
    use sha1::{Digest, Sha1};
    let mut h = Sha1::new();
    h.update(nonce);
    h.update(criado.as_bytes());
    h.update(senha.as_bytes());
    let b64 = base64::engine::general_purpose::STANDARD;
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    format!(
        concat!(
            r#"<Security s:mustUnderstand="1" xmlns="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd">"#,
            r#"<UsernameToken><Username>{}</Username>"#,
            r#"<Password Type="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest">{}</Password>"#,
            r#"<Nonce EncodingType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary">{}</Nonce>"#,
            r#"<Created xmlns="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd">{}</Created>"#,
            r#"</UsernameToken></Security>"#
        ),
        esc(usuario),
        b64.encode(h.finalize()),
        b64.encode(nonce),
        criado
    )
}

fn envelope(seguranca: &str, corpo: &str) -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope" "#,
            r#"xmlns:tds="http://www.onvif.org/ver10/device/wsdl" xmlns:trt="http://www.onvif.org/ver10/media/wsdl" "#,
            r#"xmlns:tt="http://www.onvif.org/ver10/schema">"#,
            r#"<s:Header>{}</s:Header><s:Body>{}</s:Body></s:Envelope>"#
        ),
        seguranca, corpo
    )
}

/// POST SOAP mínimo (HTTP/1.1, `Connection: close`). Devolve o corpo.
async fn post_soap(ip: &str, porta: u16, caminho: &str, xml: &str) -> Option<String> {
    let mut s = tokio::time::timeout(Duration::from_secs(2), TcpStream::connect((ip, porta)))
        .await
        .ok()?
        .ok()?;
    let req = format!(
        "POST {caminho} HTTP/1.1\r\nHost: {ip}:{porta}\r\nContent-Type: application/soap+xml; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{xml}",
        xml.len()
    );
    s.write_all(req.as_bytes()).await.ok()?;
    let mut buf = Vec::new();
    let mut parte = [0u8; 8192];
    loop {
        let n = tokio::time::timeout(Duration::from_secs(3), s.read(&mut parte))
            .await
            .ok()?
            .ok()?;
        if n == 0 || buf.len() > 512 * 1024 {
            break;
        }
        buf.extend_from_slice(&parte[..n]);
    }
    let texto = String::from_utf8_lossy(&buf).to_string();
    let (cab, corpo) = texto.split_once("\r\n\r\n")?;
    if !cab.starts_with("HTTP/1.") || !cab.split_whitespace().nth(1)?.starts_with('2') {
        return None;
    }
    let chunked = cab
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked");
    Some(if chunked {
        desfazer_chunked(corpo)
    } else {
        corpo.to_string()
    })
}

fn desfazer_chunked(mut s: &str) -> String {
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

/// Caminho do serviço de mídia a partir do `GetCapabilities`.
pub fn caminho_media(xml: &str) -> Option<String> {
    let i = xml.find("Media>").or_else(|| xml.find("Media "))?;
    let xaddr = tag_values(&xml[i..], "XAddr").into_iter().next()?;
    let resto = xaddr.split_once("://")?.1;
    Some(
        resto
            .find('/')
            .map(|p| resto[p..].to_string())
            .unwrap_or_else(|| "/".into()),
    )
}

/// Tokens das fontes de vídeo (`GetVideoSourcesResponse`).
pub fn fontes_de_video(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut resto = xml;
    while let Some(lt) = resto.find('<') {
        resto = &resto[lt + 1..];
        let Some(gt) = resto.find('>') else { break };
        let tag = &resto[..gt];
        let nome = tag.split_whitespace().next().unwrap_or("");
        if nome.rsplit(':').next() == Some("VideoSources") {
            if let Some(t) = tag.split("token=\"").nth(1) {
                out.push(t.split('"').next().unwrap_or("").to_string());
            }
        }
    }
    out
}

/// Nome de cada fonte de vídeo, pelos perfis (`GetProfilesResponse`): o
/// `Name` da `VideoSourceConfiguration` que aponta para o token.
pub fn nomes_por_fonte(xml: &str, fontes: &[String]) -> Vec<Option<String>> {
    let blocos: Vec<&str> = xml.split("VideoSourceConfiguration").collect();
    fontes
        .iter()
        .map(|tok| {
            blocos.iter().find_map(|b| {
                let fonte = tag_values(b, "SourceToken").into_iter().next()?;
                if fonte != tok {
                    return None;
                }
                tag_values(b, "Name")
                    .into_iter()
                    .next()
                    .map(|n| n.trim().to_string())
                    .filter(|n| !n.is_empty())
            })
        })
        .collect()
}

/// Nomes das fontes de vídeo pelo ONVIF com login. `None` se não respondeu.
pub async fn canais_onvif(
    ip: &str,
    porta: u16,
    usuario: &str,
    senha: &str,
) -> Option<Vec<Option<String>>> {
    let seg = || {
        let mut nonce = [0u8; 16];
        let _ = getrandom::fill(&mut nonce);
        let agora = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        usuario_token(usuario, senha, &nonce, &data_utc(agora))
    };
    let caps = post_soap(
        ip,
        porta,
        "/onvif/device_service",
        &envelope(
            &seg(),
            "<tds:GetCapabilities><tds:Category>Media</tds:Category></tds:GetCapabilities>",
        ),
    )
    .await?;
    let media = caminho_media(&caps).unwrap_or_else(|| "/onvif/media_service".into());
    let fontes = post_soap(
        ip,
        porta,
        &media,
        &envelope(&seg(), "<trt:GetVideoSources/>"),
    )
    .await?;
    let tokens = fontes_de_video(&fontes);
    if tokens.is_empty() {
        return None;
    }
    let perfis = post_soap(ip, porta, &media, &envelope(&seg(), "<trt:GetProfiles/>"))
        .await
        .unwrap_or_default();
    Some(nomes_por_fonte(&perfis, &tokens))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Gravador falso: responde nos canais listados.
    struct Falsa(Vec<u32>);

    impl Sonda for Falsa {
        async fn responde(&self, caminho: String) -> bool {
            let (c, _) = rtsp::canal_da_url(&format!("rtsp://h{caminho}")).unwrap();
            self.0.contains(&c)
        }
    }

    const DAHUA: &str = "rtsp://admin:x@10.0.0.5:554/cam/realmonitor?channel=1&subtype=1";
    const HIK: &str = "rtsp://admin:x@10.0.0.6:554/Streaming/Channels/102";

    #[test]
    fn modelos_de_gravador() {
        for m in [
            "NVD 1404",
            "MHDX 1108",
            "DHI-NVR4104-P",
            "XVR5108HS",
            "DS-7608NI-K2",
            "iDS-7204HQHI",
        ] {
            assert!(parece_gravador(m), "{m}");
        }
        for m in ["VIP 1230 B G4", "IPC-HFW1230S", "DS-2CD1023G0", "VHD 3230"] {
            assert!(!parece_gravador(m), "{m}");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn camera_so_canal_1() {
        let d = decidir(&Falsa(vec![1]), DAHUA, None, Some("VIP 1230")).await;
        assert!(!d.gravador);
        assert_eq!(d.origem, "camera");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gravador_pelo_rtsp_para_quando_o_lote_nao_responde() {
        let d = decidir(&Falsa(vec![1, 2, 3, 4, 5, 6]), HIK, None, None).await;
        assert!(d.gravador);
        assert_eq!(d.origem, "rtsp");
        assert_eq!(d.canais.len(), 6);
        assert!(d.canais.iter().all(|c| c.responde));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gravador_pelo_onvif_com_nomes_e_canais_mudos() {
        let nomes = vec![
            Some("Portão".to_string()),
            None,
            Some("Fundos".to_string()),
            None,
        ];
        let d = decidir(&Falsa(vec![1, 3]), DAHUA, Some(nomes), None).await;
        assert!(d.gravador);
        assert_eq!(d.origem, "onvif");
        assert_eq!(d.canais.len(), 4);
        assert_eq!(d.canais[0].nome.as_deref(), Some("Portão"));
        assert_eq!(
            d.canais.iter().map(|c| c.responde).collect::<Vec<_>>(),
            [true, false, true, false]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn gravador_pelo_modelo_mostra_ao_menos_4() {
        let d = decidir(&Falsa(vec![1]), DAHUA, None, Some("MHDX 1104")).await;
        assert!(d.gravador);
        assert_eq!(d.origem, "modelo");
        assert_eq!(d.canais.len(), 4);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn caminho_generico_e_camera() {
        let d = decidir(&Falsa(vec![1, 2]), "rtsp://10.0.0.1/live/ch1", None, None).await;
        assert!(!d.gravador);
    }

    #[test]
    fn le_respostas_onvif() {
        let caps = r#"<s:Envelope><s:Body><tds:GetCapabilitiesResponse><tds:Capabilities>
            <tt:Media><tt:XAddr>http://192.168.1.50/onvif/Media</tt:XAddr></tt:Media>
            </tds:Capabilities></tds:GetCapabilitiesResponse></s:Body></s:Envelope>"#;
        assert_eq!(caminho_media(caps).as_deref(), Some("/onvif/Media"));
        let fontes = r#"<trt:GetVideoSourcesResponse>
            <trt:VideoSources token="VS1"><tt:Framerate>25</tt:Framerate></trt:VideoSources>
            <trt:VideoSources token="VS2"><tt:Framerate>25</tt:Framerate></trt:VideoSources>
            <trt:VideoSources token="VS3"></trt:VideoSources>
            </trt:GetVideoSourcesResponse>"#;
        let t = fontes_de_video(fontes);
        assert_eq!(t, ["VS1", "VS2", "VS3"]);
        let perfis = r#"<trt:Profiles token="P1"><tt:Name>MainStream</tt:Name>
            <tt:VideoSourceConfiguration token="C1"><tt:Name>Portao</tt:Name><tt:UseCount>2</tt:UseCount>
            <tt:SourceToken>VS1</tt:SourceToken></tt:VideoSourceConfiguration></trt:Profiles>
            <trt:Profiles token="P3"><tt:Name>x</tt:Name>
            <tt:VideoSourceConfiguration token="C3"><tt:Name>Garagem</tt:Name>
            <tt:SourceToken>VS3</tt:SourceToken></tt:VideoSourceConfiguration></trt:Profiles>"#;
        assert_eq!(
            nomes_por_fonte(perfis, &t),
            vec![Some("Portao".into()), None, Some("Garagem".into())]
        );
    }

    #[test]
    fn token_de_seguranca_onvif() {
        let nonce = [0u8; 16];
        let x = usuario_token("admin", "senha", &nonce, "2026-11-08T12:00:00Z");
        assert!(x.contains("<Username>admin</Username>"));
        assert!(x.contains("AAAAAAAAAAAAAAAAAAAAAA=="));
        assert_eq!(data_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(data_utc(1_794_139_200), "2026-11-08T12:00:00Z");
        assert_eq!(
            desfazer_chunked("4\r\nabcd\r\n2\r\nef\r\n0\r\n\r\n"),
            "abcdef"
        );
    }

    /// Gravador falso de verdade (TCP): canais 1 a 3 respondem 200, o resto 404.
    async fn nvr_falso() -> u16 {
        use tokio::net::TcpListener;
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let Ok(n) = s.read(&mut buf).await else {
                        return;
                    };
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let uri = req.split_whitespace().nth(1).unwrap_or("").to_string();
                    let canal = rtsp::canal_da_url(&uri).map(|(c, _)| c).unwrap_or(0);
                    let r = if (1..=3).contains(&canal) {
                        "RTSP/1.0 200 OK\r\nCSeq: 1\r\nContent-Length: 0\r\n\r\n"
                    } else {
                        "RTSP/1.0 404 Not Found\r\nCSeq: 1\r\n\r\n"
                    };
                    let _ = s.write_all(r.as_bytes()).await;
                });
            }
        });
        porta
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sonda_rtsp_de_verdade_acha_3_canais() {
        let porta = nvr_falso().await;
        let s = SondaRtsp {
            ip: "127.0.0.1",
            porta,
            usuario: "",
            senha: "",
        };
        let url = format!("rtsp://127.0.0.1:{porta}/cam/realmonitor?channel=1&subtype=1");
        let d = decidir(&s, &url, None, None).await;
        assert!(d.gravador);
        assert_eq!(d.canais.len(), 3);
    }
}
