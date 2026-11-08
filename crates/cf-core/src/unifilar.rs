//! Diagrama unifilar da instalação (SDD-05), gerado a partir de uma foto da rede.
//!
//! SVG simples, sem dependência: roteador → switch → máquinas/gravadores →
//! câmeras, cada caixa com tipo, nome, IP e MAC. A varredura não enxerga
//! cabos, então as ligações abaixo do roteador são presumidas (todas por um
//! switch) e o carimbo diz isso.
//!
//! Dois temas: `Pagina` usa os tokens de `web/tokens.css` (o SVG vai inline
//! na dashboard) e `Papel` usa cores fixas claras para o arquivo exportado,
//! que precisa abrir sozinho em qualquer visualizador e ser impresso.

use crate::rede::{Dispositivo, Foto, Tipo};
use std::fmt::Write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tema {
    Pagina,
    Papel,
}

const CAIXA_L: i32 = 176;
const CAIXA_A: i32 = 76;
const VAO_X: i32 = 16;
const VAO_Y: i32 = 56;
const MARGEM: i32 = 40;
/// Eixo do tronco vertical que desce do switch, à esquerda das caixas.
const TRONCO_X: i32 = 20;
const MAX_COLUNAS: usize = 6;
const CARIMBO_A: i32 = 64;

/// Escapa texto para dentro de SVG/XML (nome de câmera e dados ONVIF vêm de fora).
pub fn escapar(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c if (c as u32) < 0x20 && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

fn cortar(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut o: String = s.chars().take(max.saturating_sub(1)).collect();
        o.push('…');
        o
    }
}

/// Data `DD/MM/AAAA HH:MM` sem crate de datas (algoritmo civil de Howard Hinnant).
pub fn data_hora(unix: i64, offset_s: i64) -> String {
    let t = unix + offset_s;
    let dias = t.div_euclid(86_400);
    let seg = t.rem_euclid(86_400);
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
        "{d:02}/{m:02}/{a:04} {:02}:{:02}",
        seg / 3600,
        (seg % 3600) / 60
    )
}

/// Caixa a desenhar: um aparelho da foto ou um marcador (roteador/switch não achados).
struct Caixa<'a> {
    d: Option<&'a Dispositivo>,
    tipo: Tipo,
    /// Linhas extras para os marcadores.
    titulo: String,
    detalhe: String,
}

fn de(d: &Dispositivo) -> Caixa<'_> {
    Caixa {
        d: Some(d),
        tipo: if d.gateway { Tipo::Roteador } else { d.tipo },
        titulo: String::new(),
        detalhe: String::new(),
    }
}

fn chave_tipo(t: Tipo) -> &'static str {
    match t {
        Tipo::Camera => "camera",
        Tipo::Gravador => "gravador",
        Tipo::Roteador => "roteador",
        Tipo::Switch => "switch",
        Tipo::Computador => "computador",
        Tipo::Desconhecido => "desconhecido",
    }
}

fn ip_ordem(ip: &str) -> Vec<u32> {
    ip.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

fn estilo(tema: Tema) -> String {
    // (fundo, superfície, linha, linha forte, texto, texto 2, texto 3, destaque)
    let c = match tema {
        Tema::Pagina => [
            "var(--bg)",
            "var(--surface)",
            "var(--line-strong)",
            "var(--text-3)",
            "var(--text)",
            "var(--text-2)",
            "var(--text-3)",
            "var(--accent)",
        ],
        // Arquivo exportado: papel claro para imprimir; não depende da dashboard.
        Tema::Papel => [
            "#ffffff", "#ffffff", "#b4b4b4", "#3a3a3a", "#0a0a0a", "#3a3a3a", "#6b6b6b", "#05952f",
        ],
    };
    format!(
        ".cf-unifilar .fundo{{fill:{fundo}}}\
.cf-unifilar .cx{{fill:{sup};stroke:{linha};stroke-width:1}}\
.cf-unifilar .cx.cad{{stroke:{dest}}}\
.cf-unifilar .cx.pres{{stroke-dasharray:4 3}}\
.cf-unifilar .fio{{fill:none;stroke:{forte};stroke-width:1.5}}\
.cf-unifilar .no{{fill:{forte}}}\
.cf-unifilar text{{font-family:system-ui,-apple-system,'Segoe UI',Roboto,sans-serif;fill:{t1}}}\
.cf-unifilar .tipo{{font-size:10px;fill:{t3};letter-spacing:.04em;text-transform:uppercase}}\
.cf-unifilar .tipo.cad{{fill:{dest}}}\
.cf-unifilar .nome{{font-size:12px;font-weight:600}}\
.cf-unifilar .mono{{font-family:ui-monospace,Consolas,'SF Mono',monospace;font-size:11px;fill:{t2}}}\
.cf-unifilar .mono.mac{{font-size:10px;fill:{t3}}}\
.cf-unifilar .carimbo{{fill:none;stroke:{linha}}}\
.cf-unifilar .titulo{{font-size:13px;font-weight:600}}\
.cf-unifilar .pe{{font-size:10px;fill:{t3}}}",
        fundo = c[0],
        sup = c[1],
        linha = c[2],
        forte = c[3],
        t1 = c[4],
        t2 = c[5],
        t3 = c[6],
        dest = c[7],
    )
}

/// Desenha o diagrama. `t` traduz chaves (`rede.tipo.*`, `rede.diagrama.*`);
/// `offset_s` é o fuso local para a data do carimbo.
pub fn desenhar(foto: &Foto, tema: Tema, offset_s: i64, t: &dyn Fn(&str) -> String) -> String {
    // ---- separa as camadas ----
    let mut ordenados: Vec<&Dispositivo> = foto.dispositivos.iter().collect();
    ordenados.sort_by_key(|d| ip_ordem(&d.ip));
    let gw = ordenados.iter().copied().find(|d| d.gateway);
    let switches: Vec<&Dispositivo> = ordenados
        .iter()
        .copied()
        .filter(|d| !d.gateway && d.tipo == Tipo::Switch)
        .collect();
    let maquinas: Vec<&Dispositivo> = ordenados
        .iter()
        .copied()
        .filter(|d| !d.gateway && !matches!(d.tipo, Tipo::Switch | Tipo::Camera))
        .collect();
    // Este PC e gravadores primeiro: são o "meio" da instalação.
    let mut maquinas = maquinas;
    maquinas.sort_by_key(|d| {
        (
            match (d.eu, d.tipo) {
                (true, _) => 0,
                (_, Tipo::Gravador) => 1,
                (_, Tipo::Computador) => 2,
                (_, Tipo::Roteador) => 3,
                _ => 4,
            },
            ip_ordem(&d.ip),
        )
    });
    let cameras: Vec<&Dispositivo> = ordenados
        .iter()
        .copied()
        .filter(|d| !d.gateway && d.tipo == Tipo::Camera)
        .collect();

    let roteador = match gw {
        Some(d) => de(d),
        None => Caixa {
            d: None,
            tipo: Tipo::Roteador,
            titulo: t("rede.diagrama.roteador_nao_achado"),
            detalhe: foto.gateway.clone().unwrap_or_else(|| "?".into()),
        },
    };
    let linha_switch: Vec<Caixa> = if switches.is_empty() {
        vec![Caixa {
            d: None,
            tipo: Tipo::Switch,
            titulo: t("rede.diagrama.switch_presumido"),
            detalhe: foto.rede.clone(),
        }]
    } else {
        switches.into_iter().map(de).collect()
    };
    let mut camadas: Vec<Vec<Caixa>> = vec![vec![roteador], linha_switch];
    for grupo in [maquinas, cameras] {
        for pedaco in grupo.chunks(MAX_COLUNAS) {
            camadas.push(pedaco.iter().map(|d| de(d)).collect());
        }
    }

    // ---- geometria ----
    let colunas = camadas.iter().map(Vec::len).max().unwrap_or(1).max(3) as i32;
    let largura = MARGEM * 2 + colunas * CAIXA_L + (colunas - 1) * VAO_X;
    let topo = 28; // espaço da linha "Internet"
    let y_camada = |i: usize| topo + VAO_Y / 2 + i as i32 * (CAIXA_A + VAO_Y);
    let fim_caixas = y_camada(camadas.len() - 1) + CAIXA_A;
    let altura = fim_caixas + 24 + CARIMBO_A + 16;
    let x_caixa = |n: usize, j: usize| {
        let usada = n as i32 * CAIXA_L + (n as i32 - 1) * VAO_X;
        (largura - usada) / 2 + j as i32 * (CAIXA_L + VAO_X)
    };

    let mut s = String::with_capacity(4096 + foto.dispositivos.len() * 600);
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"cf-unifilar\" viewBox=\"0 0 {largura} {altura}\" width=\"{largura}\" height=\"{altura}\" role=\"img\" aria-label=\"{}\">",
        escapar(&t("rede.diagrama.titulo"))
    );
    let _ = write!(s, "<style>{}</style>", estilo(tema));
    let _ = write!(
        s,
        "<rect class=\"fundo\" x=\"0\" y=\"0\" width=\"{largura}\" height=\"{altura}\"/>"
    );

    // ---- fios ----
    let cx_rot = largura / 2;
    let y_rot = y_camada(0);
    // Internet → roteador.
    let _ = write!(
        s,
        "<text class=\"pe\" x=\"{}\" y=\"16\" text-anchor=\"middle\">{}</text>",
        cx_rot,
        escapar(&t("rede.diagrama.internet"))
    );
    let _ = write!(s, "<path class=\"fio\" d=\"M{cx_rot} 22V{y_rot}\"/>");
    // Roteador → switches.
    let y_sw = y_camada(1);
    let n_sw = camadas[1].len();
    let barra = y_sw - VAO_Y / 2;
    let _ = write!(
        s,
        "<path class=\"fio\" d=\"M{cx_rot} {}V{barra}\"/>",
        y_rot + CAIXA_A
    );
    if n_sw > 1 {
        let _ = write!(
            s,
            "<path class=\"fio\" d=\"M{} {barra}H{}\"/>",
            x_caixa(n_sw, 0) + CAIXA_L / 2,
            x_caixa(n_sw, n_sw - 1) + CAIXA_L / 2
        );
    }
    for j in 0..n_sw {
        let x = x_caixa(n_sw, j) + CAIXA_L / 2;
        let _ = write!(s, "<path class=\"fio\" d=\"M{x} {barra}V{y_sw}\"/>");
    }
    // Primeiro switch → tronco à esquerda → barramento de cada camada abaixo.
    if camadas.len() > 2 {
        let x_sw = x_caixa(n_sw, 0) + CAIXA_L / 2;
        let saida = y_sw + CAIXA_A + VAO_Y / 4;
        let ultima_barra = y_camada(camadas.len() - 1) - VAO_Y / 2;
        let _ = write!(
            s,
            "<path class=\"fio\" d=\"M{x_sw} {}V{saida}H{TRONCO_X}V{ultima_barra}\"/>",
            y_sw + CAIXA_A
        );
        for (i, camada) in camadas.iter().enumerate().skip(2) {
            let y = y_camada(i);
            let b = y - VAO_Y / 2;
            let n = camada.len();
            let fim = x_caixa(n, n - 1) + CAIXA_L / 2;
            let _ = write!(s, "<path class=\"fio\" d=\"M{TRONCO_X} {b}H{fim}\"/>");
            let _ = write!(
                s,
                "<circle class=\"no\" cx=\"{TRONCO_X}\" cy=\"{b}\" r=\"2.5\"/>"
            );
            for j in 0..n {
                let x = x_caixa(n, j) + CAIXA_L / 2;
                let _ = write!(s, "<path class=\"fio\" d=\"M{x} {b}V{y}\"/>");
            }
        }
    }

    // ---- caixas ----
    for (i, camada) in camadas.iter().enumerate() {
        let y = y_camada(i);
        let n = camada.len();
        for (j, c) in camada.iter().enumerate() {
            let x = x_caixa(n, j);
            desenhar_caixa(&mut s, c, x, y, t);
        }
    }

    // ---- carimbo ----
    let yc = fim_caixas + 24;
    let _ = write!(
        s,
        "<rect class=\"carimbo\" x=\"{MARGEM}\" y=\"{yc}\" width=\"{}\" height=\"{CARIMBO_A}\" rx=\"4\"/>",
        largura - 2 * MARGEM
    );
    let _ = write!(
        s,
        "<text class=\"titulo\" x=\"{}\" y=\"{}\">{}</text>",
        MARGEM + 12,
        yc + 22,
        escapar(&t("rede.diagrama.titulo"))
    );
    let resumo = t("rede.diagrama.resumo")
        .replace("{rede}", &foto.rede)
        .replace("{data}", &data_hora(foto.quando, offset_s))
        .replace("{n}", &foto.dispositivos.len().to_string());
    let _ = write!(
        s,
        "<text class=\"mono\" x=\"{}\" y=\"{}\">{}</text>",
        MARGEM + 12,
        yc + 40,
        escapar(&resumo)
    );
    let _ = write!(
        s,
        "<text class=\"pe\" x=\"{}\" y=\"{}\">{}</text>",
        MARGEM + 12,
        yc + 55,
        escapar(&t("rede.diagrama.aviso"))
    );
    s.push_str("</svg>");
    s
}

fn desenhar_caixa(s: &mut String, c: &Caixa, x: i32, y: i32, t: &dyn Fn(&str) -> String) {
    let cad = c.d.is_some_and(|d| d.camera_id.is_some());
    let pres = c.d.is_none();
    let classe = match (cad, pres) {
        (true, _) => "cx cad",
        (_, true) => "cx pres",
        _ => "cx",
    };
    let _ = write!(
        s,
        "<rect class=\"{classe}\" x=\"{x}\" y=\"{y}\" width=\"{CAIXA_L}\" height=\"{CAIXA_A}\" rx=\"6\"/>"
    );
    let tx = x + 10;
    let (tipo, nome, ip, mac) = match c.d {
        Some(d) => {
            let mut tipo = t(&format!(
                "rede.tipo.{}",
                if d.eu {
                    "computador"
                } else {
                    chave_tipo(c.tipo)
                }
            ));
            if d.eu {
                tipo = format!("{tipo} · {}", t("rede.eu"));
            }
            let nome = d
                .camera_nome
                .clone()
                .or_else(|| match (&d.fabricante, &d.modelo) {
                    (Some(f), Some(m)) => Some(format!("{f} {m}")),
                    (Some(f), None) => Some(f.clone()),
                    (None, Some(m)) => Some(m.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| t("rede.diagrama.sem_nome"));
            let mac = d.mac.clone().unwrap_or_else(|| t("rede.diagrama.sem_mac"));
            (tipo, nome, d.ip.clone(), mac)
        }
        None => (
            t(&format!("rede.tipo.{}", chave_tipo(c.tipo))),
            c.titulo.clone(),
            c.detalhe.clone(),
            String::new(),
        ),
    };
    let _ = write!(
        s,
        "<text class=\"tipo{}\" x=\"{tx}\" y=\"{}\">{}</text>",
        if cad { " cad" } else { "" },
        y + 16,
        escapar(&cortar(&tipo, 28))
    );
    let _ = write!(
        s,
        "<text class=\"nome\" x=\"{tx}\" y=\"{}\">{}</text>",
        y + 33,
        escapar(&cortar(&nome, 24))
    );
    let _ = write!(
        s,
        "<text class=\"mono\" x=\"{tx}\" y=\"{}\">{}</text>",
        y + 50,
        escapar(&cortar(&ip, 26))
    );
    if !mac.is_empty() {
        let _ = write!(
            s,
            "<text class=\"mono mac\" x=\"{tx}\" y=\"{}\">{}</text>",
            y + 65,
            escapar(&cortar(&mac, 26))
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disp(ip: &str, tipo: Tipo) -> Dispositivo {
        Dispositivo {
            ip: ip.into(),
            mac: Some(format!("58:10:8C:00:00:{:02X}", ip_ordem(ip)[3])),
            fabricante: Some("Intelbras".into()),
            modelo: None,
            tipo,
            portas: vec![554],
            onvif: false,
            camera_id: None,
            camera_nome: None,
            eu: false,
            gateway: false,
        }
    }

    fn rot(k: &str) -> String {
        // Rótulo curto e reconhecível: o último pedaço da chave.
        format!("<{}>", k.rsplit('.').next().unwrap_or(k))
    }

    fn foto(dispositivos: Vec<Dispositivo>) -> Foto {
        Foto {
            quando: 1_794_099_224,
            rede: "192.168.1.0/24".into(),
            meu_ip: Some("192.168.1.10".into()),
            gateway: Some("192.168.1.1".into()),
            dispositivos,
        }
    }

    #[test]
    fn data_sem_crate() {
        assert_eq!(data_hora(0, 0), "01/01/1970 00:00");
        assert_eq!(data_hora(951_782_400, 0), "29/02/2000 00:00");
        // 2026-11-08 00:53:44 UTC → 21:53 do dia 7 em Brasília.
        assert_eq!(data_hora(1_794_099_224, -3 * 3600), "07/11/2026 21:53");
    }

    #[test]
    fn escapa_nomes_de_fora() {
        assert_eq!(escapar("<script>&\"'"), "&lt;script&gt;&amp;&quot;&#39;");
        let mut c = disp("192.168.1.101", Tipo::Camera);
        c.camera_nome = Some("<img onerror=x>".into());
        c.camera_id = Some(1);
        let svg = desenhar(&foto(vec![c]), Tema::Pagina, 0, &rot);
        assert!(!svg.contains("<img"));
        assert!(svg.contains("&lt;img onerror=x&gt;"));
    }

    #[test]
    fn camadas_e_marcadores() {
        let mut gw = disp("192.168.1.1", Tipo::Roteador);
        gw.gateway = true;
        let mut eu = disp("192.168.1.10", Tipo::Computador);
        eu.eu = true;
        let mut cad = disp("192.168.1.108", Tipo::Camera);
        cad.camera_id = Some(7);
        cad.camera_nome = Some("Portaria".into());
        let mut ds = vec![gw, eu, disp("192.168.1.20", Tipo::Gravador), cad];
        for i in 0..7 {
            ds.push(disp(&format!("192.168.1.{}", 120 + i), Tipo::Camera));
        }
        let svg = desenhar(&foto(ds), Tema::Pagina, 0, &rot);
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        // Sem switch na varredura: aparece o presumido, tracejado.
        assert!(svg.contains("&lt;switch_presumido&gt;"));
        assert!(svg.contains("cx pres"));
        assert!(!svg.contains("roteador_nao_achado"));
        // 8 câmeras em 6 colunas: duas camadas; 1 cadastrada em destaque.
        assert_eq!(svg.matches("cx cad").count(), 1);
        assert!(svg.contains("Portaria"));
        assert!(svg.contains("58:10:8C:00:00:6C"));
        // 4 camadas de caixas abaixo do switch (máquinas, câmeras ×2) → 3 barramentos.
        assert_eq!(svg.matches("<circle class=\"no\"").count(), 3);
        assert!(svg.contains("var(--accent)"));
        assert!(svg.contains("&lt;computador&gt; · &lt;eu&gt;"));
        // Papel: cores fixas, nada de var().
        let papel = desenhar(&foto(vec![]), Tema::Papel, 0, &rot);
        assert!(!papel.contains("var("));
        assert!(papel.contains("&lt;roteador_nao_achado&gt;"));
        assert!(papel.contains("192.168.1.1"));
    }
}
