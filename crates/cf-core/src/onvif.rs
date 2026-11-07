//! Busca de câmeras ONVIF na rede (WS-Discovery) e leitura do MAC.
//!
//! Envia um `Probe` multicast para 239.255.255.250:3702 e junta as respostas por
//! alguns segundos. Roda só quando o técnico pede.

use serde::Serialize;
use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;

const MULTICAST: SocketAddr = SocketAddr::V4(std::net::SocketAddrV4::new(
    Ipv4Addr::new(239, 255, 255, 250),
    3702,
));

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Device {
    pub ip: String,
    pub fabricante: Option<String>,
    pub modelo: Option<String>,
    pub nome: Option<String>,
    pub enderecos: Vec<String>,
    pub mac: Option<String>,
}

fn probe_message(id: &str) -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<e:Envelope xmlns:e="http://www.w3.org/2003/05/soap-envelope" "#,
            r#"xmlns:w="http://schemas.xmlsoap.org/ws/2004/08/addressing" "#,
            r#"xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" "#,
            r#"xmlns:dn="http://www.onvif.org/ver10/network/wsdl">"#,
            r#"<e:Header><w:MessageID>uuid:{}</w:MessageID>"#,
            r#"<w:To e:mustUnderstand="true">urn:schemas-xmlsoap-org:ws:2005:04:discovery</w:To>"#,
            r#"<w:Action e:mustUnderstand="true">http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</w:Action>"#,
            r#"</e:Header><e:Body><d:Probe><d:Types>dn:NetworkVideoTransmitter</d:Types>"#,
            r#"</d:Probe></e:Body></e:Envelope>"#
        ),
        id
    )
}

/// Identificador pseudoaleatório no formato UUID, sem dependência extra.
fn message_id() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
        ^ (std::process::id() as u128) << 64;
    let h = format!("{n:032x}");
    format!(
        "{}-{}-4{}-a{}-{}",
        &h[0..8],
        &h[8..12],
        &h[13..16],
        &h[17..20],
        &h[20..32]
    )
}

/// Conteúdo de todas as tags cujo nome local é `local` (ignora o prefixo).
fn tag_values<'a>(xml: &'a str, local: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        rest = &rest[lt + 1..];
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[..gt];
        if tag.starts_with('/') || tag.starts_with('?') || tag.ends_with('/') {
            continue;
        }
        let name = tag.split_whitespace().next().unwrap_or("");
        let name_local = name.rsplit(':').next().unwrap_or(name);
        if name_local == local {
            let body = &rest[gt + 1..];
            if let Some(end) = body.find("</") {
                out.push(body[..end].trim());
            }
        }
    }
    out
}

fn decode_scope(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'_' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).trim().to_string()
}

/// Interpreta um `ProbeMatches`. Retorna `None` se não for uma resposta válida.
pub fn parse_probe_match(xml: &str, from_ip: &str) -> Option<Device> {
    if !xml.contains("ProbeMatch") {
        return None;
    }
    let enderecos: Vec<String> = tag_values(xml, "XAddrs")
        .iter()
        .flat_map(|s| s.split_whitespace())
        .map(str::to_string)
        .collect();
    let scopes: Vec<&str> = tag_values(xml, "Scopes")
        .iter()
        .flat_map(|s| s.split_whitespace())
        .collect();
    let scope = |key: &str| {
        scopes.iter().find_map(|s| {
            s.strip_prefix(&format!("onvif://www.onvif.org/{key}/"))
                .map(decode_scope)
        })
    };
    let nome = scope("name");
    let modelo = scope("hardware");
    let fabricante = scope("mfr")
        .or_else(|| scope("manufacturer"))
        .or_else(|| nome.as_deref().and_then(guess_vendor).map(str::to_string));
    Some(Device {
        ip: from_ip.to_string(),
        fabricante,
        modelo,
        nome,
        enderecos,
        mac: None,
    })
}

/// Tenta reconhecer o fabricante pelo nome anunciado.
fn guess_vendor(name: &str) -> Option<&'static str> {
    let n = name.to_ascii_lowercase();
    [
        ("intelbras", "Intelbras"),
        ("dahua", "Dahua"),
        ("hikvision", "Hikvision"),
        ("hilook", "Hikvision"),
        ("axis", "Axis"),
        ("uniview", "Uniview"),
    ]
    .into_iter()
    .find(|(k, _)| n.contains(k))
    .map(|(_, v)| v)
}

/// Lê a tabela ARP para encontrar o MAC de cada IP (Linux). Em outros sistemas
/// retorna vazio por enquanto (SDD-05 cobre Windows na fase 4).
pub fn arp_table() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let Ok(text) = std::fs::read_to_string("/proc/net/arp") {
        for line in text.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() >= 4 && cols[3] != "00:00:00:00:00:00" {
                map.insert(cols[0].to_string(), cols[3].to_ascii_uppercase());
            }
        }
    }
    map
}

/// Busca câmeras por `wait`. Nunca falha: erros de rede viram lista vazia.
pub async fn discover(wait: Duration) -> Vec<Device> {
    let Ok(sock) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await else {
        return Vec::new();
    };
    let _ = sock.set_multicast_ttl_v4(2);
    let msg = probe_message(&message_id());
    // Duas sondas: UDP perde pacotes em redes cheias.
    for _ in 0..2 {
        let _ = sock.send_to(msg.as_bytes(), MULTICAST).await;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    let mut found: BTreeMap<String, Device> = BTreeMap::new();
    let mut buf = vec![0u8; 16 * 1024];
    let deadline = tokio::time::Instant::now() + wait;
    while let Ok(Ok((n, from))) = tokio::time::timeout_at(deadline, sock.recv_from(&mut buf)).await
    {
        let ip = from.ip().to_string();
        if let Some(d) = parse_probe_match(&String::from_utf8_lossy(&buf[..n]), &ip) {
            found.entry(ip).or_insert(d);
        }
    }

    let arp = arp_table();
    let mut list: Vec<Device> = found.into_values().collect();
    for d in &mut list {
        d.mac = arp.get(&d.ip).cloned();
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<?xml version="1.0"?><s:Envelope xmlns:s="x" xmlns:d="y"><s:Body>
<d:ProbeMatches><d:ProbeMatch><d:Scopes>onvif://www.onvif.org/type/video_encoder
onvif://www.onvif.org/name/IPC_Intelbras onvif://www.onvif.org/hardware/VIP%201230%20B
onvif://www.onvif.org/location/country/brazil</d:Scopes>
<d:XAddrs>http://192.168.1.108/onvif/device_service http://[fe80::1]/onvif/device_service</d:XAddrs>
</d:ProbeMatch></d:ProbeMatches></s:Body></s:Envelope>"#;

    #[test]
    fn interpreta_resposta() {
        let d = parse_probe_match(SAMPLE, "192.168.1.108").unwrap();
        assert_eq!(d.ip, "192.168.1.108");
        assert_eq!(d.nome.as_deref(), Some("IPC Intelbras"));
        assert_eq!(d.modelo.as_deref(), Some("VIP 1230 B"));
        assert_eq!(d.fabricante.as_deref(), Some("Intelbras"));
        assert_eq!(d.enderecos.len(), 2);
    }

    #[test]
    fn ignora_lixo() {
        assert!(parse_probe_match("<a>oi</a>", "1.1.1.1").is_none());
    }

    #[test]
    fn sonda_tem_tipo_nvt() {
        let m = probe_message("abc");
        assert!(m.contains("NetworkVideoTransmitter"));
        assert!(m.contains("uuid:abc"));
        assert_eq!(message_id().len(), 36);
    }
}
