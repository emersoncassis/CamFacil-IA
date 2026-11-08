//! Nome real de cada canal do gravador: o título que aparece no vídeo (OSD),
//! como "INTERNA FUNDO". Fontes, em ordem:
//! 1. Intelbras/Dahua: `configManager.cgi?action=getConfig&name=ChannelTitle`;
//! 2. Hikvision: `ISAPI/ContentMgmt/InputProxy/channels` (NVR) e
//!    `ISAPI/System/Video/inputs/channels` (DVR/câmera);
//! 3. ONVIF (nome da fonte/perfil), só se não for genérico.
//!
//! Nome genérico ("MainStream", "Profile_1", "CAM 1"…) é descartado: aí fica
//! o "<gravador> · Canal N" de sempre.

use crate::digest;
use crate::onvif::tag_values;
use std::collections::BTreeMap;
use std::time::Duration;

/// Teto total da busca por HTTP (não pode atrasar o "Ligar câmera").
pub const LIMITE: Duration = Duration::from_secs(3);

pub const CAMINHO_DAHUA: &str = "/cgi-bin/configManager.cgi?action=getConfig&name=ChannelTitle";
pub const CAMINHO_HIK_NVR: &str = "/ISAPI/ContentMgmt/InputProxy/channels";
pub const CAMINHO_HIK_DVR: &str = "/ISAPI/System/Video/inputs/channels";

/// Canal (1, 2…) → nome.
pub type Nomes = BTreeMap<u32, String>;

/// Desfaz as entidades XML básicas.
fn desescapar(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fn limpar(s: &str) -> Option<String> {
    let n: String = s
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(60)
        .collect();
    let n = n.trim().to_string();
    (!n.is_empty()).then_some(n)
}

/// `table.ChannelTitle[0].Name=INTERNA FUNDO` (índice 0 = canal 1).
pub fn ler_dahua(txt: &str) -> Nomes {
    let mut out = Nomes::new();
    for l in txt.lines() {
        let Some(resto) = l.trim().strip_prefix("table.ChannelTitle[") else {
            continue;
        };
        let Some((i, resto)) = resto.split_once(']') else {
            continue;
        };
        let Some(v) = resto.strip_prefix(".Name=") else {
            continue;
        };
        if let (Ok(i), Some(n)) = (i.parse::<u32>(), limpar(v)) {
            out.insert(i + 1, n);
        }
    }
    out
}

/// Blocos `<tag …>…</tag>` (ignora `<tagList>` e afins).
fn blocos<'a>(xml: &'a str, tag: &str) -> Vec<&'a str> {
    let abre = format!("<{tag}");
    let fecha = format!("</{tag}>");
    let mut out = Vec::new();
    let mut resto = xml;
    while let Some(i) = resto.find(&abre) {
        let depois = &resto[i + abre.len()..];
        if !depois.starts_with([' ', '>', '\t', '\r', '\n']) {
            resto = depois;
            continue;
        }
        let fim = depois.find(&fecha).unwrap_or(depois.len());
        out.push(&depois[..fim]);
        resto = &depois[fim..];
    }
    out
}

fn ler_hik(xml: &str, tag: &str) -> Nomes {
    blocos(xml, tag)
        .into_iter()
        .filter_map(|b| {
            let id: u32 = tag_values(b, "id").first()?.trim().parse().ok()?;
            let nome = limpar(&desescapar(tag_values(b, "name").first()?))?;
            (id > 0).then_some((id, nome))
        })
        .collect()
}

/// NVR Hikvision: `<InputProxyChannel><id>1</id><name>…</name>`.
pub fn ler_hik_nvr(xml: &str) -> Nomes {
    ler_hik(xml, "InputProxyChannel")
}

/// DVR/câmera Hikvision: `<VideoInputChannel><id>1</id><name>…</name>`.
pub fn ler_hik_dvr(xml: &str) -> Nomes {
    ler_hik(xml, "VideoInputChannel")
}

/// Nome que não diz nada ("MainStream", "VideoSourceConfig_1", "Profile_1",
/// "user0", "CAM 1", "Camera 01", "Canal 3", "D1"…).
pub fn generico(nome: &str) -> bool {
    let n: String = nome
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let base = n.trim_end_matches(|c: char| c.is_ascii_digit());
    const IGUAIS: &[&str] = &[
        "",
        "cam",
        "camera",
        "camara",
        "canal",
        "channel",
        "ch",
        "chn",
        "ipc",
        "ipcam",
        "ipcamera",
        "d",
        "a",
        "ip",
        "video",
        "videoinput",
        "input",
        "source",
        "sourcetoken",
        "vs",
        "vsc",
        "user",
        "default",
        "stream",
        "main",
        "sub",
        "extra",
        "third",
        "none",
        "null",
        "token",
        "config",
        "h264",
        "h265",
        "mjpeg",
        "onvif",
        "media",
    ];
    const PREFIXOS: &[&str] = &[
        "mainstream",
        "substream",
        "extrastream",
        "thirdstream",
        "videosource",
        "videoencoder",
        "videoinput",
        "mediaprofile",
        "profile",
        "vsconf",
        "defaultprofile",
    ];
    IGUAIS.contains(&base) || PREFIXOS.iter().any(|p| n.starts_with(p))
}

/// Junta as fontes na ordem de prioridade, descartando os genéricos.
pub fn juntar(fontes: &[&Nomes]) -> Nomes {
    let mut out = Nomes::new();
    for f in fontes {
        for (c, n) in f.iter() {
            if !generico(n) {
                out.entry(*c).or_insert_with(|| n.clone());
            }
        }
    }
    out
}

/// ONVIF (lista por posição: índice 0 = canal 1) em `Nomes`.
pub fn de_onvif(lista: &[Option<String>]) -> Nomes {
    lista
        .iter()
        .enumerate()
        .filter_map(|(i, n)| Some((i as u32 + 1, limpar(n.as_deref()?)?)))
        .collect()
}

/// Busca por HTTP (porta `porta_http`, normalmente 80), conforme o padrão do
/// caminho RTSP (`intelbras` ou `hikvision`). Falha = vazio. O chamador
/// põe o teto ([`LIMITE`]).
pub async fn buscar_http(
    ip: &str,
    porta_http: u16,
    usuario: &str,
    senha: &str,
    fabricante: &str,
) -> Nomes {
    let lim = Duration::from_millis(2500);
    match fabricante {
        "intelbras" | "dahua" => digest::get(ip, porta_http, CAMINHO_DAHUA, usuario, senha, lim)
            .await
            .map(|t| ler_dahua(&t))
            .unwrap_or_default(),
        "hikvision" => {
            let (nvr, dvr) = tokio::join!(
                digest::get(ip, porta_http, CAMINHO_HIK_NVR, usuario, senha, lim),
                digest::get(ip, porta_http, CAMINHO_HIK_DVR, usuario, senha, lim),
            );
            let nvr = nvr.map(|x| ler_hik_nvr(&x)).unwrap_or_default();
            let dvr = dvr.map(|x| ler_hik_dvr(&x)).unwrap_or_default();
            // No NVR os canais IP vêm do InputProxy; nos híbridos as entradas
            // analógicas vêm do VideoInput.
            juntar(&[&nvr, &dvr])
        }
        _ => Nomes::new(),
    }
}

/// Busca completa com teto: HTTP do fabricante, ONVIF por último.
pub async fn buscar(
    ip: &str,
    porta_http: u16,
    usuario: &str,
    senha: &str,
    fabricante: &str,
    onvif: &[Option<String>],
) -> Nomes {
    let http = tokio::time::timeout(
        LIMITE,
        buscar_http(ip, porta_http, usuario, senha, fabricante),
    )
    .await
    .unwrap_or_default();
    juntar(&[&http, &de_onvif(onvif)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_dahua() {
        let txt = "table.ChannelTitle[0].Name=INTERNA FUNDO\r\ntable.ChannelTitle[1].Name=PORTÃO\r\ntable.ChannelTitle[2].Name=\r\ntable.ChannelTitle[3].Name=CAM 4\r\n";
        let n = ler_dahua(txt);
        assert_eq!(n.get(&1).map(String::as_str), Some("INTERNA FUNDO"));
        assert_eq!(n.get(&2).map(String::as_str), Some("PORTÃO"));
        assert!(!n.contains_key(&3));
        assert_eq!(juntar(&[&n]).len(), 2, "CAM 4 é genérico");
    }

    #[test]
    fn le_hik_nvr() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<InputProxyChannelList version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
<InputProxyChannel version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
<id>1</id>
<name>Garagem &amp; Rampa</name>
<sourceInputPortDescriptor>
<proxyProtocol>HIKVISION</proxyProtocol>
<addressingFormatType>ipaddress</addressingFormatType>
<ipAddress>192.168.254.2</ipAddress>
<managePortNo>8000</managePortNo>
<srcInputPort>1</srcInputPort>
<userName>admin</userName>
<streamType>auto</streamType>
</sourceInputPortDescriptor>
<enableAnr>false</enableAnr>
</InputProxyChannel>
<InputProxyChannel version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
<id>2</id>
<name>Camera 02</name>
</InputProxyChannel>
</InputProxyChannelList>"#;
        let n = ler_hik_nvr(xml);
        assert_eq!(n.len(), 2);
        assert_eq!(n[&1], "Garagem & Rampa");
        assert_eq!(n[&2], "Camera 02");
        assert_eq!(juntar(&[&n]).len(), 1);
    }

    #[test]
    fn le_hik_dvr() {
        let xml = r#"<VideoInputChannelList version="2.0" xmlns="http://www.isapi.org/ver20/XMLSchema">
<VideoInputChannel version="2.0" xmlns="http://www.isapi.org/ver20/XMLSchema">
<id>1</id><inputPort>1</inputPort><videoInputEnabled>true</videoInputEnabled>
<name>RECEPCAO</name><videoFormat>PAL</videoFormat><resDesc>1920*1080P25</resDesc>
</VideoInputChannel>
<VideoInputChannel version="2.0" xmlns="http://www.isapi.org/ver20/XMLSchema">
<id>3</id><inputPort>3</inputPort><name>ESTOQUE</name>
</VideoInputChannel>
</VideoInputChannelList>"#;
        let n = ler_hik_dvr(xml);
        assert_eq!(n[&1], "RECEPCAO");
        assert_eq!(n[&3], "ESTOQUE");
        assert!(ler_hik_nvr(xml).is_empty());
    }

    #[test]
    fn descarta_genericos() {
        for g in [
            "MainStream",
            "SubStream",
            "VideoSourceConfig_1",
            "VideoSourceConfig_Channel3",
            "Profile_1",
            "MediaProfile000",
            "user0",
            "CAM 1",
            "Camera 01",
            "Canal 3",
            "Channel1",
            "D1",
            "IPCamera 05",
            "VSC_2",
            "000",
            "  ",
        ] {
            assert!(generico(g), "{g}");
        }
        for b in [
            "INTERNA FUNDO",
            "Portão",
            "Cam Garagem",
            "Caixa 2",
            "Doca B",
        ] {
            assert!(!generico(b), "{b}");
        }
    }

    #[test]
    fn junta_por_prioridade() {
        let http: Nomes = [(1, "PORTAO".into()), (2, "CAM 2".into())].into();
        let onvif = de_onvif(&[
            Some("Entrada".into()),
            Some("Fundos".into()),
            Some("MainStream".into()),
        ]);
        let j = juntar(&[&http, &onvif]);
        assert_eq!(j[&1], "PORTAO");
        assert_eq!(j[&2], "Fundos");
        assert!(!j.contains_key(&3));
    }

    /// NVR Intelbras falso: exige Digest e devolve os títulos.
    #[tokio::test(flavor = "current_thread")]
    async fn busca_http_em_gravador_falso() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let porta = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                let mut b = vec![0u8; 4096];
                let n = s.read(&mut b).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&b[..n]).to_string();
                let r = if req.contains("Authorization: Digest") {
                    let c = "table.ChannelTitle[0].Name=INTERNA FUNDO\r\n";
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{c}", c.len())
                } else {
                    "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Digest realm=\"x\", nonce=\"y\", qop=\"auth\"\r\n\r\n".into()
                };
                let _ = s.write_all(r.as_bytes()).await;
            }
        });
        let n = buscar("127.0.0.1", porta, "admin", "x", "intelbras", &[]).await;
        assert_eq!(n[&1], "INTERNA FUNDO");
        // Porta fechada: vazio, sem travar.
        let t = std::time::Instant::now();
        assert!(buscar("127.0.0.1", 1, "a", "b", "hikvision", &[])
            .await
            .is_empty());
        assert!(t.elapsed() < LIMITE + Duration::from_millis(500));
    }
}
