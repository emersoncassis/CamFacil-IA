//! Relatório de validação / proposta em PDF (PRD §5.6, SDD-06).
//!
//! Gerador PDF mínimo próprio (sem crate): páginas A4, Helvetica (fonte padrão
//! do leitor, nada embutido), linhas, retângulos e uma foto JPEG por câmera via
//! `DCTDecode` (o JPEG do ffmpeg entra como está). O arquivo sai em
//! `data_dir/relatorios/` e pode ser baixado ou enviado pelo Telegram.
//!
//! As formas das regras desenhadas sobre a foto são só para o técnico ver onde
//! as regras estão; é um documento de trabalho, não a tela do cliente.

use crate::i18n::{t, tf};
use crate::{App, Shared};
use axum::{
    extract::{Path as AxPath, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Escritor PDF mínimo
// ---------------------------------------------------------------------------

pub const A4_L: f32 = 595.0;
pub const A4_A: f32 = 842.0;
const MARGEM: f32 = 50.0;

/// Larguras da Helvetica (AFM) para os códigos 32..=126, em milésimos.
const LARGURAS: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 222, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// Letra base de um caractere acentuado (para largura e para o fallback).
fn base(c: char) -> char {
    const ACENTOS: &str = "àáâãäåèéêëìíîïòóôõöùúûüçñýÿÀÁÂÃÄÅÈÉÊËÌÍÎÏÒÓÔÕÖÙÚÛÜÇÑÝ";
    const BASES: &str = "aaaaaaeeeeiiiiooooouuuucnyyAAAAAAEEEEIIIIOOOOOUUUUCNY";
    ACENTOS
        .chars()
        .position(|a| a == c)
        .and_then(|i| BASES.chars().nth(i))
        .unwrap_or(c)
}

/// Largura de um texto em pontos, para alinhar e quebrar linhas.
pub fn largura(texto: &str, tamanho: f32) -> f32 {
    let mil: u32 = texto
        .chars()
        .map(|c| {
            let b = base(c) as u32;
            if (32..=126).contains(&b) {
                LARGURAS[(b - 32) as usize] as u32
            } else {
                match c {
                    '°' | '·' | '•' => 400,
                    '–' => 556,
                    '—' => 1000,
                    _ => 556,
                }
            }
        })
        .sum();
    mil as f32 * tamanho / 1000.0
}

/// UTF-8 → WinAnsiEncoding, já escapado para string literal do PDF.
fn literal(texto: &str) -> String {
    let mut s = String::from("(");
    for c in texto.chars() {
        let code: u8 = match c as u32 {
            0x20..=0x7E => c as u8,
            0xA0..=0xFF => c as u32 as u8,
            _ => match c {
                '€' => 0x80,
                '…' => 0x85,
                '–' => 0x96,
                '—' => 0x97,
                '•' => 0x95,
                '\u{2018}' => 0x91,
                '\u{2019}' => 0x92,
                '\u{201C}' => 0x93,
                '\u{201D}' => 0x94,
                _ => {
                    let b = base(c);
                    if b != c && (b as u32) < 0x7F {
                        b as u8
                    } else {
                        b'?'
                    }
                }
            },
        };
        match code {
            b'(' | b')' | b'\\' => {
                s.push('\\');
                s.push(code as char);
            }
            0x20..=0x7E => s.push(code as char),
            _ => {
                let _ = write!(s, "\\{code:03o}");
            }
        }
    }
    s.push(')');
    s
}

/// Dimensões e nº de componentes de um JPEG (marcador SOF).
pub fn jpeg_info(b: &[u8]) -> Option<(u32, u32, u8)> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 9 < b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let m = b[i + 1];
        if m == 0xFF {
            i += 1;
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if matches!(m, 0xC0..=0xCF) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
            return Some((w, h, b[i + 9]));
        }
        if m == 0xDA {
            return None;
        }
        i += 2 + len;
    }
    None
}

#[derive(Default)]
pub struct Pagina {
    conteudo: String,
    imagens: Vec<usize>,
}

pub struct Pdf {
    paginas: Vec<Pagina>,
    /// (bytes, largura, altura, componentes)
    imagens: Vec<(Vec<u8>, u32, u32, u8)>,
}

impl Default for Pdf {
    fn default() -> Self {
        Self::new()
    }
}

impl Pdf {
    pub fn new() -> Self {
        Self {
            paginas: Vec::new(),
            imagens: Vec::new(),
        }
    }

    pub fn nova_pagina(&mut self) -> &mut Pagina {
        self.paginas.push(Pagina::default());
        self.paginas.last_mut().unwrap()
    }

    pub fn pagina(&mut self) -> &mut Pagina {
        if self.paginas.is_empty() {
            self.nova_pagina();
        }
        self.paginas.last_mut().unwrap()
    }

    /// Registra um JPEG; devolve o índice para `Pagina::imagem`.
    pub fn jpeg(&mut self, bytes: Vec<u8>) -> Option<usize> {
        let (w, h, comp) = jpeg_info(&bytes)?;
        self.imagens.push((bytes, w, h, comp));
        Some(self.imagens.len() - 1)
    }

    /// Serializa: catálogo, páginas, 2 fontes, imagens e xref.
    pub fn bytes(&self) -> Vec<u8> {
        let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets: Vec<usize> = Vec::new();
        // Numeração: 1 catálogo, 2 páginas, 3 Helvetica, 4 Helvetica-Bold,
        // depois imagens (5..), depois, por página, [página, conteúdo].
        let n_img = self.imagens.len();
        let primeiro_pag = 5 + n_img;
        let obj = |out: &mut Vec<u8>, offsets: &mut Vec<usize>, n: usize, corpo: &[u8]| {
            debug_assert_eq!(offsets.len() + 1, n);
            offsets.push(out.len());
            out.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
            out.extend_from_slice(corpo);
            out.extend_from_slice(b"\nendobj\n");
        };
        obj(
            &mut out,
            &mut offsets,
            1,
            b"<< /Type /Catalog /Pages 2 0 R >>",
        );
        let kids: Vec<String> = (0..self.paginas.len())
            .map(|i| format!("{} 0 R", primeiro_pag + i * 2))
            .collect();
        obj(
            &mut out,
            &mut offsets,
            2,
            format!(
                "<< /Type /Pages /Kids [{}] /Count {} >>",
                kids.join(" "),
                self.paginas.len()
            )
            .as_bytes(),
        );
        obj(
            &mut out,
            &mut offsets,
            3,
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        );
        obj(&mut out, &mut offsets, 4, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>");
        for (i, (bytes, w, h, comp)) in self.imagens.iter().enumerate() {
            let cs = if *comp == 1 {
                "DeviceGray"
            } else {
                "DeviceRGB"
            };
            let mut corpo = format!(
                "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /{cs} /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>\nstream\n",
                bytes.len()
            )
            .into_bytes();
            corpo.extend_from_slice(bytes);
            corpo.extend_from_slice(b"\nendstream");
            obj(&mut out, &mut offsets, 5 + i, &corpo);
        }
        for (i, p) in self.paginas.iter().enumerate() {
            let n_pag = primeiro_pag + i * 2;
            let xobj: String = p
                .imagens
                .iter()
                .map(|im| format!("/Im{im} {} 0 R", 5 + im))
                .collect::<Vec<_>>()
                .join(" ");
            obj(
                &mut out,
                &mut offsets,
                n_pag,
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {A4_L} {A4_A}] /Resources << /Font << /F1 3 0 R /F2 4 0 R >> /XObject << {xobj} >> >> /Contents {} 0 R >>",
                    n_pag + 1
                )
                .as_bytes(),
            );
            let mut corpo = format!("<< /Length {} >>\nstream\n", p.conteudo.len()).into_bytes();
            corpo.extend_from_slice(p.conteudo.as_bytes());
            corpo.extend_from_slice(b"\nendstream");
            obj(&mut out, &mut offsets, n_pag + 1, &corpo);
        }
        let xref = out.len();
        let _ = write!(
            out_str(&mut out),
            "xref\n0 {}\n0000000000 65535 f \n",
            offsets.len() + 1
        );
        for o in &offsets {
            let _ = writeln!(out_str(&mut out), "{o:010} 00000 n ");
        }
        let _ = write!(
            out_str(&mut out),
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        );
        out
    }
}

/// Escreve `fmt` direto num `Vec<u8>`.
fn out_str(v: &mut Vec<u8>) -> OutStr<'_> {
    OutStr(v)
}
struct OutStr<'a>(&'a mut Vec<u8>);
impl std::fmt::Write for OutStr<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

/// Cores em cinza (0 = preto, 1 = branco) e RGB.
#[derive(Clone, Copy)]
pub enum Cor {
    Cinza(f32),
    Rgb(f32, f32, f32),
}

impl Cor {
    fn op(self, traco: bool) -> String {
        match (self, traco) {
            (Cor::Cinza(g), false) => format!("{g:.3} g"),
            (Cor::Cinza(g), true) => format!("{g:.3} G"),
            (Cor::Rgb(r, g, b), false) => format!("{r:.3} {g:.3} {b:.3} rg"),
            (Cor::Rgb(r, g, b), true) => format!("{r:.3} {g:.3} {b:.3} RG"),
        }
    }
}

impl Pagina {
    /// Texto na posição (x, y) em coordenadas PDF (origem embaixo à esquerda).
    pub fn texto(&mut self, x: f32, y: f32, tam: f32, negrito: bool, cor: Cor, s: &str) {
        let f = if negrito { "F2" } else { "F1" };
        let _ = writeln!(
            self.conteudo,
            "BT {} /{f} {tam:.1} Tf {x:.1} {y:.1} Td {} Tj ET",
            cor.op(false),
            literal(s)
        );
    }

    pub fn texto_direita(&mut self, xd: f32, y: f32, tam: f32, negrito: bool, cor: Cor, s: &str) {
        let w = largura(s, tam) * if negrito { 1.05 } else { 1.0 };
        self.texto(xd - w, y, tam, negrito, cor, s);
    }

    pub fn texto_centro(&mut self, xc: f32, y: f32, tam: f32, negrito: bool, cor: Cor, s: &str) {
        let w = largura(s, tam) * if negrito { 1.05 } else { 1.0 };
        self.texto(xc - w / 2.0, y, tam, negrito, cor, s);
    }

    pub fn linha(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, esp: f32, cor: Cor) {
        let _ = writeln!(
            self.conteudo,
            "{} {esp:.2} w {x1:.1} {y1:.1} m {x2:.1} {y2:.1} l S",
            cor.op(true)
        );
    }

    /// Polilinha (fechada ou não).
    pub fn poli(&mut self, pts: &[(f32, f32)], fechar: bool, esp: f32, cor: Cor) {
        if pts.len() < 2 {
            return;
        }
        let _ = write!(
            self.conteudo,
            "{} {esp:.2} w {:.1} {:.1} m",
            cor.op(true),
            pts[0].0,
            pts[0].1
        );
        for p in &pts[1..] {
            let _ = write!(self.conteudo, " {:.1} {:.1} l", p.0, p.1);
        }
        let _ = writeln!(self.conteudo, " {}", if fechar { "s" } else { "S" });
    }

    pub fn retangulo(&mut self, x: f32, y: f32, w: f32, h: f32, cor: Cor, preencher: bool) {
        let _ = writeln!(
            self.conteudo,
            "{} {x:.1} {y:.1} {w:.1} {h:.1} re {}",
            cor.op(!preencher),
            if preencher { "f" } else { "S" }
        );
    }

    /// Desenha a imagem `idx` ocupando o retângulo dado.
    pub fn imagem(&mut self, idx: usize, x: f32, y: f32, w: f32, h: f32) {
        if !self.imagens.contains(&idx) {
            self.imagens.push(idx);
        }
        let _ = writeln!(
            self.conteudo,
            "q {w:.1} 0 0 {h:.1} {x:.1} {y:.1} cm /Im{idx} Do Q"
        );
    }
}

/// Quebra um parágrafo em linhas que caibam em `larg` pontos.
pub fn quebrar(texto: &str, tam: f32, larg: f32) -> Vec<String> {
    let mut linhas = Vec::new();
    for par in texto.split('\n') {
        let mut atual = String::new();
        for palavra in par.split_whitespace() {
            let tentativa = if atual.is_empty() {
                palavra.to_string()
            } else {
                format!("{atual} {palavra}")
            };
            if largura(&tentativa, tam) <= larg || atual.is_empty() {
                atual = tentativa;
            } else {
                linhas.push(std::mem::take(&mut atual));
                atual = palavra.to_string();
            }
        }
        linhas.push(atual);
    }
    linhas
}

// ---------------------------------------------------------------------------
// Datas (sem crate): dias desde a época → ano-mês-dia (algoritmo civil).
// ---------------------------------------------------------------------------

pub fn data_civil(unix: i64, offset_s: i64) -> (i32, u32, u32) {
    let dias = (unix + offset_s).div_euclid(86_400);
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

pub fn data_pt(unix: i64, offset_s: i64) -> String {
    let (a, m, d) = data_civil(unix, offset_s);
    format!("{d:02}/{m:02}/{a}")
}

fn data_hora_pt(unix: i64, offset_s: i64) -> String {
    format!(
        "{} {}",
        data_pt(unix, offset_s),
        cf_notify::servico::hora_com_offset(unix, offset_s)
    )
}

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Montagem do relatório
// ---------------------------------------------------------------------------

pub struct Dados {
    pub cliente: String,
    pub tecnico: String,
    pub observacoes: String,
}

const CINZA: Cor = Cor::Cinza(0.45);
const PRETO: Cor = Cor::Cinza(0.0);
const VERDE: Cor = Cor::Rgb(0.02, 0.6, 0.25);
const LINHA: Cor = Cor::Cinza(0.8);

struct Ctx<'a> {
    pdf: &'a mut Pdf,
    y: f32,
    pag: usize,
    cab: String,
}

impl Ctx<'_> {
    fn nova(&mut self) {
        self.pag += 1;
        let p = self.pdf.nova_pagina();
        p.texto(MARGEM, A4_A - 30.0, 8.0, false, CINZA, &self.cab);
        p.texto_direita(
            A4_L - MARGEM,
            A4_A - 30.0,
            8.0,
            false,
            CINZA,
            &tf("pdf.pagina", &[("n", &self.pag.to_string())]),
        );
        p.linha(MARGEM, A4_A - 36.0, A4_L - MARGEM, A4_A - 36.0, 0.5, LINHA);
        p.texto_centro(A4_L / 2.0, 24.0, 7.5, false, CINZA, &t("pdf.rodape"));
        self.y = A4_A - 60.0;
    }

    fn garantir(&mut self, altura: f32) {
        if self.y - altura < MARGEM + 10.0 {
            self.nova();
        }
    }

    fn titulo(&mut self, s: &str) {
        self.garantir(40.0);
        self.pdf
            .pagina()
            .texto(MARGEM, self.y, 16.0, true, PRETO, s);
        self.y -= 8.0;
        self.pdf.pagina().linha(
            MARGEM,
            self.y,
            MARGEM + largura(s, 16.0) * 1.05,
            self.y,
            1.5,
            VERDE,
        );
        self.y -= 20.0;
    }

    fn sub(&mut self, s: &str) {
        self.garantir(30.0);
        self.pdf
            .pagina()
            .texto(MARGEM, self.y, 11.5, true, PRETO, s);
        self.y -= 16.0;
    }

    fn paragrafo(&mut self, s: &str, tam: f32, cor: Cor) {
        for l in quebrar(s, tam, A4_L - 2.0 * MARGEM) {
            self.garantir(tam * 1.5);
            self.pdf.pagina().texto(MARGEM, self.y, tam, false, cor, &l);
            self.y -= tam * 1.45;
        }
        self.y -= 4.0;
    }

    /// Tabela simples: colunas com largura fixa (frações) e cabeçalho.
    fn tabela(&mut self, cab: &[&str], fracoes: &[f32], linhas: &[Vec<String>]) {
        let larg_total = A4_L - 2.0 * MARGEM;
        let xs: Vec<f32> = fracoes
            .iter()
            .scan(MARGEM, |acc, f| {
                let x = *acc;
                *acc += f * larg_total;
                Some(x)
            })
            .collect();
        let desenhar_cab = |c: &mut Ctx| {
            c.garantir(28.0);
            let p = c.pdf.pagina();
            p.retangulo(MARGEM, c.y - 5.0, larg_total, 16.0, Cor::Cinza(0.93), true);
            for (i, h) in cab.iter().enumerate() {
                p.texto(xs[i] + 4.0, c.y, 8.5, true, PRETO, h);
            }
            c.y -= 16.0;
        };
        desenhar_cab(self);
        for l in linhas {
            if self.y - 14.0 < MARGEM + 10.0 {
                self.nova();
                desenhar_cab(self);
            }
            let p = self.pdf.pagina();
            for (i, cel) in l.iter().enumerate() {
                let max = fracoes[i] * larg_total - 8.0;
                let mut s = cel.clone();
                while largura(&s, 8.5) > max && s.chars().count() > 3 {
                    s.pop();
                    s.pop();
                    s.push('…');
                }
                p.texto(xs[i] + 4.0, self.y, 8.5, false, PRETO, &s);
            }
            p.linha(
                MARGEM,
                self.y - 4.0,
                A4_L - MARGEM,
                self.y - 4.0,
                0.3,
                LINHA,
            );
            self.y -= 14.0;
        }
        self.y -= 10.0;
    }
}

fn situacao_cam(e: &Option<crate::video::EstadoCamera>) -> String {
    match e {
        Some(e) if e.online => tf(
            "pdf.cam.ao_vivo",
            &[
                ("fps", &format!("{:.1}", e.fps)),
                ("saude", crate::notificacoes::saude_pt(&e.saude)),
            ],
        ),
        Some(e) => tf(
            "pdf.cam.fora",
            &[(
                "erro",
                &e.erro.clone().unwrap_or_else(|| t("pdf.cam.conectando")),
            )],
        ),
        None => t("pdf.cam.desligada"),
    }
}

fn nome_regra(forma: &str) -> String {
    match forma {
        "zona" => t("pdf.forma.zona"),
        "linha" => t("pdf.forma.linha"),
        _ => t("pdf.forma.regra"),
    }
}

/// Gera o PDF completo em memória.
pub fn gerar(app: &App, d: &Dados) -> Result<Vec<u8>, String> {
    let off = cf_notify::servico::offset_local_s(&app.store);
    let agora_u = agora();
    let cams = app.store.cameras().map_err(|e| e.to_string())?;
    let cams: Vec<_> = cams.into_iter().filter(|c| c.ativa).collect();
    let eventos = app
        .store
        .eventos_desde(agora_u - 7 * 86_400)
        .map_err(|e| e.to_string())?;
    let bench: Option<Value> = app
        .store
        .get("bench_result")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok());
    let mut pdf = Pdf::new();
    let cliente = if d.cliente.trim().is_empty() {
        t("pdf.cliente_padrao")
    } else {
        d.cliente.trim().to_string()
    };
    let mut c = Ctx {
        pdf: &mut pdf,
        y: 0.0,
        pag: 0,
        cab: tf("pdf.cabecalho", &[("cliente", &cliente)]),
    };

    // ---- Capa ----
    c.nova();
    {
        let p = c.pdf.pagina();
        p.retangulo(0.0, A4_A - 200.0, A4_L, 164.0, Cor::Cinza(0.08), true);
        p.texto(
            MARGEM,
            A4_A - 100.0,
            26.0,
            true,
            Cor::Cinza(1.0),
            &t("pdf.capa.titulo"),
        );
        p.texto(
            MARGEM,
            A4_A - 128.0,
            13.0,
            false,
            Cor::Cinza(0.85),
            &t("pdf.capa.subtitulo"),
        );
        p.texto(MARGEM, A4_A - 160.0, 9.0, false, VERDE, &t("pdf.capa.selo"));
        let mut y = A4_A - 260.0;
        let item = |p: &mut Pagina, y: f32, k: &str, v: &str| {
            p.texto(MARGEM, y, 9.0, false, CINZA, k);
            p.texto(MARGEM + 110.0, y, 11.0, true, PRETO, v);
        };
        item(p, y, &t("pdf.capa.cliente"), &cliente);
        y -= 26.0;
        item(p, y, &t("pdf.capa.data"), &data_hora_pt(agora_u, off));
        y -= 26.0;
        item(
            p,
            y,
            &t("pdf.capa.tecnico"),
            if d.tecnico.trim().is_empty() {
                "—"
            } else {
                d.tecnico.trim()
            },
        );
        y -= 26.0;
        item(p, y, &t("pdf.capa.cameras"), &cams.len().to_string());
        y -= 26.0;
        let total_regras: usize = cams
            .iter()
            .map(|k| app.store.regras(k.id).map(|r| r.len()).unwrap_or(0))
            .sum();
        item(p, y, &t("pdf.capa.regras"), &total_regras.to_string());
        y -= 26.0;
        let alertas = eventos
            .iter()
            .filter(|e| {
                matches!(
                    e.tipo.as_str(),
                    "linha_cruzada" | "zona_ocupada" | "permanencia"
                )
            })
            .count();
        item(p, y, &t("pdf.capa.alertas"), &alertas.to_string());
        y -= 26.0;
        item(
            p,
            y,
            &t("pdf.capa.maquina"),
            &format!(
                "{} {} · {}",
                std::env::consts::OS,
                std::env::consts::ARCH,
                bench
                    .as_ref()
                    .and_then(|b| b.get("nivel"))
                    .and_then(|n| n.as_str())
                    .map(|n| tf("pdf.capa.nivel", &[("nivel", n)]))
                    .unwrap_or_else(|| t("pdf.capa.sem_teste"))
            ),
        );
        c.y = y - 40.0;
    }
    if !d.observacoes.trim().is_empty() {
        c.sub(&t("pdf.observacoes"));
        c.paragrafo(d.observacoes.trim(), 10.0, PRETO);
    }

    // ---- Resumo das câmeras ----
    c.nova();
    c.titulo(&t("pdf.resumo.titulo"));
    c.paragrafo(&t("pdf.resumo.desc"), 9.5, CINZA);
    let linhas: Vec<Vec<String>> = cams
        .iter()
        .map(|k| {
            let e = app.video.canal(k.id).map(|c| c.estado());
            let fonte = match k.fonte.as_str() {
                f @ ("rtsp" | "demo" | "webcam" | "arquivo") => t(&format!("pdf.fonte.{f}")),
                f => f.to_string(),
            };
            vec![
                k.nome.clone(),
                fonte,
                situacao_cam(&e),
                app.store
                    .regras(k.id)
                    .map(|r| r.len())
                    .unwrap_or(0)
                    .to_string(),
            ]
        })
        .collect();
    if linhas.is_empty() {
        c.paragrafo(&t("pdf.nenhuma_camera"), 10.0, PRETO);
    } else {
        c.tabela(
            &[
                &t("pdf.col.camera"),
                &t("pdf.col.fonte"),
                &t("pdf.col.situacao"),
                &t("pdf.col.regras"),
            ],
            &[0.3, 0.17, 0.41, 0.12],
            &linhas,
        );
    }
    let m = crate::video::modulos(&app.store);
    let ligados: Vec<&str> = crate::video::MODULOS
        .iter()
        .filter(|(id, _, _, _)| m.ligado(id))
        .map(|(_, nome, _, _)| *nome)
        .collect();
    c.sub(&t("pdf.modulos.titulo"));
    c.paragrafo(
        &if ligados.is_empty() {
            t("pdf.modulos.nenhum")
        } else {
            ligados.join(" · ")
        },
        10.0,
        PRETO,
    );

    // ---- Uma página por câmera: foto + regras ----
    for k in &cams {
        let regras = app.store.regras(k.id).unwrap_or_default();
        c.nova();
        c.titulo(&tf("pdf.camera.titulo", &[("nome", &k.nome)]));
        let foto = app
            .video
            .canal(k.id)
            .and_then(|ch| ch.ultimo.lock().unwrap().clone())
            .map(|j| j.as_ref().clone());
        let larg_img = A4_L - 2.0 * MARGEM;
        match foto.and_then(|j| c.pdf.jpeg(j)) {
            Some(idx) => {
                let (w, h) = {
                    let (_, w, h, _) = c.pdf.imagens[idx];
                    (w as f32, h as f32)
                };
                let ih = (larg_img * h / w).min(320.0);
                let iw = ih * w / h;
                let ix = MARGEM + (larg_img - iw) / 2.0;
                let iy = c.y - ih;
                c.pdf.pagina().imagem(idx, ix, iy, iw, ih);
                // Formas das regras (só para o técnico): linhas finas sobre a foto.
                for r in &regras {
                    let pts: Vec<(f32, f32)> = r
                        .pontos
                        .iter()
                        .map(|p| (ix + p.x * iw, iy + (1.0 - p.y) * ih))
                        .collect();
                    let p = c.pdf.pagina();
                    p.poli(&pts, r.forma == "zona", 1.4, Cor::Rgb(0.1, 0.9, 0.35));
                    if let Some(cx) = pts.iter().map(|p| p.0).reduce(|a, b| a + b) {
                        let cy: f32 = pts.iter().map(|p| p.1).sum();
                        let n = pts.len() as f32;
                        let (cx, cy) = (cx / n, cy / n);
                        let w = largura(&r.nome, 8.0) + 8.0;
                        p.retangulo(cx - w / 2.0, cy - 4.0, w, 12.0, Cor::Cinza(0.0), true);
                        p.texto_centro(cx, cy - 1.0, 8.0, false, Cor::Cinza(1.0), &r.nome);
                    }
                }
                c.y = iy - 14.0;
                c.pdf.pagina().texto(
                    MARGEM,
                    c.y,
                    8.0,
                    false,
                    CINZA,
                    &t("pdf.camera.legenda_foto"),
                );
                c.y -= 22.0;
            }
            None => {
                c.paragrafo(&t("pdf.camera.sem_imagem"), 10.0, CINZA);
            }
        }
        if regras.is_empty() {
            c.paragrafo(&t("pdf.camera.sem_regras"), 10.0, PRETO);
        } else {
            let linhas: Vec<Vec<String>> = regras
                .iter()
                .map(|r| {
                    let n7 = eventos.iter().filter(|e| e.regra_id == Some(r.id)).count();
                    let rot = match r.forma.as_str() {
                        "zona" => format!(
                            "{} / {}",
                            if r.rotulo_on.is_empty() {
                                t("pdf.balao.ocupada")
                            } else {
                                r.rotulo_on.clone()
                            },
                            if r.rotulo_off.is_empty() {
                                t("pdf.balao.livre")
                            } else {
                                r.rotulo_off.clone()
                            }
                        ),
                        _ => t("pdf.balao.contagem"),
                    };
                    vec![r.nome.clone(), nome_regra(&r.forma), rot, n7.to_string()]
                })
                .collect();
            c.tabela(
                &[
                    &t("pdf.col.regra"),
                    &t("pdf.col.tipo"),
                    &t("pdf.col.balao"),
                    &t("pdf.col.alertas7"),
                ],
                &[0.34, 0.14, 0.34, 0.18],
                &linhas,
            );
        }
    }

    // ---- Estatística 7 dias ----
    c.nova();
    c.titulo(&t("pdf.semana.titulo"));
    c.paragrafo(&t("pdf.semana.desc"), 9.5, CINZA);
    let hoje = (agora_u + off).div_euclid(86_400);
    let dias: Vec<i64> = (0..7).rev().map(|i| hoje - i).collect();
    let mut cab: Vec<String> = vec![t("pdf.col.regra_camera")];
    for d in &dias {
        let (_, m, dd) = data_civil(d * 86_400, 0);
        cab.push(format!("{dd:02}/{m:02}"));
    }
    cab.push(t("pdf.col.total"));
    let mut por_regra: BTreeMap<i64, [u32; 7]> = BTreeMap::new();
    for e in &eventos {
        if !matches!(
            e.tipo.as_str(),
            "linha_cruzada" | "zona_ocupada" | "permanencia"
        ) {
            continue;
        }
        let Some(r) = e.regra_id else { continue };
        let dia = (e.quando + off).div_euclid(86_400);
        if let Some(i) = dias.iter().position(|d| *d == dia) {
            por_regra.entry(r).or_default()[i] += 1;
        }
    }
    let mut linhas: Vec<Vec<String>> = Vec::new();
    for k in &cams {
        for r in app.store.regras(k.id).unwrap_or_default() {
            let cont = por_regra.get(&r.id).copied().unwrap_or_default();
            let mut l = vec![format!("{} ({})", r.nome, k.nome)];
            l.extend(cont.iter().map(|n| n.to_string()));
            l.push(cont.iter().sum::<u32>().to_string());
            linhas.push(l);
        }
    }
    if linhas.is_empty() {
        c.paragrafo(&t("pdf.semana.sem_regras"), 10.0, PRETO);
    } else {
        let cab_ref: Vec<&str> = cab.iter().map(|s| s.as_str()).collect();
        let mut fr = vec![0.30];
        fr.extend(std::iter::repeat_n(0.085, 7));
        fr.push(0.105);
        c.tabela(&cab_ref, &fr, &linhas);
    }

    // ---- Saúde das câmeras e da máquina ----
    c.titulo(&t("pdf.saude.titulo"));
    let mut saude_linhas: Vec<Vec<String>> = Vec::new();
    for k in &cams {
        let probs = eventos
            .iter()
            .filter(|e| e.tipo == "saude" && e.camera_id == k.id && e.detalhe != "ok")
            .count();
        let ultimo = eventos
            .iter()
            .find(|e| e.tipo == "saude" && e.camera_id == k.id && e.detalhe != "ok")
            .map(|e| {
                format!(
                    "{} · {}",
                    data_hora_pt(e.quando, off),
                    crate::notificacoes::saude_pt(&e.detalhe)
                )
            })
            .unwrap_or_else(|| t("pdf.saude.nenhum"));
        saude_linhas.push(vec![k.nome.clone(), probs.to_string(), ultimo]);
    }
    if !saude_linhas.is_empty() {
        c.tabela(
            &[
                &t("pdf.col.camera"),
                &t("pdf.col.problemas7"),
                &t("pdf.col.ultimo_problema"),
            ],
            &[0.34, 0.22, 0.44],
            &saude_linhas,
        );
    }
    c.sub(&t("pdf.maquina.titulo"));
    let hw = crate::hardware::monitor()
        .and_then(|m| m.log.desde(agora_u - 7 * 86_400).ok())
        .unwrap_or_default();
    if hw.is_empty() {
        c.paragrafo(&t("pdf.maquina.sem_medicoes"), 10.0, PRETO);
    } else {
        let n = hw.len() as f32;
        let media =
            |f: &dyn Fn(&crate::hardware::Amostra) -> f32| hw.iter().map(f).sum::<f32>() / n;
        let max =
            |f: &dyn Fn(&crate::hardware::Amostra) -> f32| hw.iter().map(f).fold(0.0f32, f32::max);
        let temp: Vec<f32> = hw.iter().filter_map(|a| a.temperatura).collect();
        let ultima = hw.last().unwrap();
        let hw_alertas = app
            .store
            .eventos_tipo("hardware", 500)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.quando >= agora_u - 7 * 86_400)
            .count();
        let linhas = vec![
            vec![
                t("pdf.maquina.processador"),
                tf(
                    "pdf.maquina.media_pct",
                    &[("v", &format!("{:.0}", media(&|a| a.cpu)))],
                ),
                tf(
                    "pdf.maquina.pico_pct",
                    &[("v", &format!("{:.0}", max(&|a| a.cpu)))],
                ),
            ],
            vec![
                t("pdf.maquina.memoria"),
                tf(
                    "pdf.maquina.media_pct",
                    &[("v", &format!("{:.0}", media(&|a| a.ram_pct)))],
                ),
                tf(
                    "pdf.maquina.pico_ram",
                    &[
                        ("v", &format!("{:.0}", max(&|a| a.ram_pct))),
                        ("total", &ultima.ram_total_mb.to_string()),
                    ],
                ),
            ],
            vec![
                t("pdf.maquina.temperatura"),
                if temp.is_empty() {
                    t("pdf.maquina.sem_sensor")
                } else {
                    tf(
                        "pdf.maquina.media_c",
                        &[(
                            "v",
                            &format!("{:.0}", temp.iter().sum::<f32>() / temp.len() as f32),
                        )],
                    )
                },
                if temp.is_empty() {
                    "—".into()
                } else {
                    tf(
                        "pdf.maquina.pico_c",
                        &[(
                            "v",
                            &format!("{:.0}", temp.iter().cloned().fold(0.0, f32::max)),
                        )],
                    )
                },
            ],
            vec![
                t("pdf.maquina.disco"),
                ultima
                    .disco_livre_mb
                    .map(|d| format!("{:.1} GB", d as f32 / 1024.0))
                    .unwrap_or_else(|| "—".into()),
                String::new(),
            ],
            vec![
                t("pdf.maquina.alertas"),
                hw_alertas.to_string(),
                t("pdf.maquina.limites"),
            ],
        ];
        c.tabela(
            &[
                &t("pdf.col.medida"),
                &t("pdf.col.media"),
                &t("pdf.col.pico"),
            ],
            &[0.3, 0.3, 0.4],
            &linhas,
        );
    }
    if let Some(b) = &bench {
        let fps = b
            .get("fps_estimado")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let cm = b.get("cameras_medio").and_then(|v| v.as_u64()).unwrap_or(0);
        c.paragrafo(
            &tf(
                "pdf.bench",
                &[
                    (
                        "nivel",
                        b.get("nivel").and_then(|v| v.as_str()).unwrap_or("?"),
                    ),
                    ("fps", &format!("{fps:.1}")),
                    ("cameras", &cm.to_string()),
                ],
            ),
            9.5,
            PRETO,
        );
    }
    if let Some(s) = app
        .store
        .get("estresse_resultado")
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
    {
        if let Some(v) = s.get("veredito").and_then(|v| v.as_str()) {
            c.paragrafo(&tf("pdf.estresse", &[("veredito", v)]), 9.5, PRETO);
        }
    }

    // ---- Próximo passo ----
    c.nova();
    c.titulo(&t("pdf.proximo.titulo"));
    c.paragrafo(&t("pdf.proximo_passo"), 10.5, PRETO);
    c.y -= 20.0;
    c.sub(&t("pdf.proposta.titulo"));
    c.paragrafo(&t("pdf.proposta.desc"), 10.0, CINZA);
    for _ in 0..6 {
        c.garantir(24.0);
        c.pdf
            .pagina()
            .linha(MARGEM, c.y, A4_L - MARGEM, c.y, 0.4, LINHA);
        c.y -= 22.0;
    }
    Ok(pdf.bytes())
}

// ---------------------------------------------------------------------------
// Arquivos e API
// ---------------------------------------------------------------------------

fn pasta() -> PathBuf {
    cf_core::paths::data_dir().join("relatorios")
}

fn nome_valido(n: &str) -> bool {
    !n.is_empty()
        && n.ends_with(".pdf")
        && n.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !n.contains("..")
}

type ApiResult = Result<Json<Value>, (StatusCode, Json<Value>)>;

fn erro(status: StatusCode, msg: impl ToString) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "erro": msg.to_string() })))
}

fn listar() -> Vec<Value> {
    let mut v: Vec<(String, u64, i64)> = std::fs::read_dir(pasta())
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let nome = e.file_name().to_string_lossy().to_string();
                    if !nome_valido(&nome) {
                        return None;
                    }
                    let md = e.metadata().ok()?;
                    let quando = md
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    Some((nome, md.len(), quando))
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|x| std::cmp::Reverse(x.2));
    v.into_iter()
        .map(|(n, t, q)| json!({ "arquivo": n, "bytes": t, "quando": q }))
        .collect()
}

/// `GET /api/relatorio`: relatórios já gerados e os dados lembrados da capa.
pub async fn get_lista(State(app): State<Shared>) -> Json<Value> {
    Json(json!({
        "relatorios": listar(),
        "cliente": app.store.get("rel_cliente").ok().flatten().unwrap_or_default(),
        "tecnico": app.store.get("rel_tecnico").ok().flatten().unwrap_or_default(),
        "pasta": pasta().display().to_string(),
    }))
}

#[derive(Deserialize)]
pub struct Pedido {
    #[serde(default)]
    cliente: String,
    #[serde(default)]
    tecnico: String,
    #[serde(default)]
    observacoes: String,
}

/// `POST /api/relatorio`: gera e salva.
pub async fn post_gerar(State(app): State<Shared>, Json(p): Json<Pedido>) -> ApiResult {
    let cliente: String = p.cliente.trim().chars().take(80).collect();
    let tecnico: String = p.tecnico.trim().chars().take(80).collect();
    let _ = app.store.set("rel_cliente", &cliente);
    let _ = app.store.set("rel_tecnico", &tecnico);
    let d = Dados {
        cliente: cliente.clone(),
        tecnico,
        observacoes: p.observacoes.chars().take(2000).collect(),
    };
    let app2 = app.clone();
    let bytes = tokio::task::spawn_blocking(move || gerar(&app2, &d))
        .await
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e))?
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let off = cf_notify::servico::offset_local_s(&app.store);
    let t = agora();
    let (a, m, dd) = data_civil(t, off);
    let hora = cf_notify::servico::hora_com_offset(t, off).replace(':', "");
    let slug: String = cliente
        .chars()
        .map(base)
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(24)
        .collect();
    let nome = format!(
        "relatorio-{a}{m:02}{dd:02}-{hora}{}.pdf",
        if slug.is_empty() {
            String::new()
        } else {
            format!("-{slug}")
        }
    );
    std::fs::create_dir_all(pasta()).map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    std::fs::write(pasta().join(&nome), &bytes)
        .map_err(|e| erro(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(json!({ "arquivo": nome, "bytes": bytes.len() })))
}

/// `GET /api/relatorio/{arquivo}`: download.
pub async fn get_baixar(AxPath(nome): AxPath<String>) -> Response {
    if !nome_valido(&nome) {
        return (StatusCode::BAD_REQUEST, "nome inválido").into_response();
    }
    match std::fs::read(pasta().join(&nome)) {
        Ok(b) => (
            [
                (header::CONTENT_TYPE, "application/pdf".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{nome}\""),
                ),
            ],
            b,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "relatório não encontrado").into_response(),
    }
}

pub async fn delete_relatorio(AxPath(nome): AxPath<String>) -> ApiResult {
    if !nome_valido(&nome) {
        return Err(erro(StatusCode::BAD_REQUEST, "nome inválido"));
    }
    std::fs::remove_file(pasta().join(&nome))
        .map_err(|_| erro(StatusCode::NOT_FOUND, "relatório não encontrado"))?;
    Ok(Json(json!({ "removido": true })))
}

/// `POST /api/relatorio/{arquivo}/telegram`: envia aos contatos autorizados.
pub async fn post_telegram(State(app): State<Shared>, AxPath(nome): AxPath<String>) -> ApiResult {
    if !nome_valido(&nome) {
        return Err(erro(StatusCode::BAD_REQUEST, "nome inválido"));
    }
    let bytes = std::fs::read(pasta().join(&nome))
        .map_err(|_| erro(StatusCode::NOT_FOUND, "relatório não encontrado"))?;
    let tg = app
        .telegram
        .get()
        .ok_or_else(|| erro(StatusCode::SERVICE_UNAVAILABLE, "Telegram indisponível"))?;
    let n = tg
        .enviar_arquivo(&nome, bytes, &t("pdf.telegram_legenda"))
        .await
        .map_err(|e| erro(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(json!({ "enviados": n })))
}

/// Chaves de tradução usadas no PDF (conferidas no teste).
#[cfg(test)]
const CHAVES_PDF: &[&str] = &[
    "pdf.proximo_passo",
    "pdf.pagina",
    "pdf.rodape",
    "pdf.cam.ao_vivo",
    "pdf.cam.fora",
    "pdf.cam.conectando",
    "pdf.cam.desligada",
    "pdf.forma.zona",
    "pdf.forma.linha",
    "pdf.forma.regra",
    "pdf.cliente_padrao",
    "pdf.cabecalho",
    "pdf.capa.titulo",
    "pdf.capa.subtitulo",
    "pdf.capa.selo",
    "pdf.capa.cliente",
    "pdf.capa.data",
    "pdf.capa.tecnico",
    "pdf.capa.cameras",
    "pdf.capa.regras",
    "pdf.capa.alertas",
    "pdf.capa.maquina",
    "pdf.capa.nivel",
    "pdf.capa.sem_teste",
    "pdf.observacoes",
    "pdf.resumo.titulo",
    "pdf.resumo.desc",
    "pdf.fonte.rtsp",
    "pdf.fonte.demo",
    "pdf.fonte.webcam",
    "pdf.fonte.arquivo",
    "pdf.nenhuma_camera",
    "pdf.col.camera",
    "pdf.col.fonte",
    "pdf.col.situacao",
    "pdf.col.regras",
    "pdf.modulos.titulo",
    "pdf.modulos.nenhum",
    "pdf.camera.titulo",
    "pdf.camera.legenda_foto",
    "pdf.camera.sem_imagem",
    "pdf.camera.sem_regras",
    "pdf.balao.ocupada",
    "pdf.balao.livre",
    "pdf.balao.contagem",
    "pdf.col.regra",
    "pdf.col.tipo",
    "pdf.col.balao",
    "pdf.col.alertas7",
    "pdf.semana.titulo",
    "pdf.semana.desc",
    "pdf.col.regra_camera",
    "pdf.col.total",
    "pdf.semana.sem_regras",
    "pdf.saude.titulo",
    "pdf.saude.nenhum",
    "pdf.col.problemas7",
    "pdf.col.ultimo_problema",
    "pdf.maquina.titulo",
    "pdf.maquina.sem_medicoes",
    "pdf.maquina.processador",
    "pdf.maquina.memoria",
    "pdf.maquina.temperatura",
    "pdf.maquina.disco",
    "pdf.maquina.alertas",
    "pdf.maquina.media_pct",
    "pdf.maquina.pico_pct",
    "pdf.maquina.pico_ram",
    "pdf.maquina.sem_sensor",
    "pdf.maquina.media_c",
    "pdf.maquina.pico_c",
    "pdf.maquina.limites",
    "pdf.col.medida",
    "pdf.col.media",
    "pdf.col.pico",
    "pdf.bench",
    "pdf.estresse",
    "pdf.proximo.titulo",
    "pdf.proposta.titulo",
    "pdf.proposta.desc",
    "pdf.telegram_legenda",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datas() {
        assert_eq!(data_civil(0, 0), (1970, 1, 1));
        assert_eq!(data_pt(1_700_000_000, -3 * 3600), "14/11/2023");
        // 23:30 UTC de 31/12/2023 ainda é 31/12 em Brasília; 01:00 UTC já virou.
        assert_eq!(data_pt(1_704_065_400, -3 * 3600), "31/12/2023");
        assert_eq!(data_pt(1_704_070_800, 0), "01/01/2024");
    }

    #[test]
    fn literal_winansi() {
        assert_eq!(literal("abc"), "(abc)");
        assert_eq!(literal("a(b)c\\"), "(a\\(b\\)c\\\\)");
        assert_eq!(literal("ção"), "(\\347\\343o)");
        assert_eq!(literal("x — y…"), "(x \\227 y\\205)");
        assert_eq!(literal("漢"), "(?)");
    }

    #[test]
    fn larguras_e_quebra() {
        assert!((largura("a", 10.0) - 5.56).abs() < 0.01);
        assert_eq!(largura("á", 10.0), largura("a", 10.0));
        let l = quebrar("uma frase bem comprida para quebrar em linhas", 10.0, 80.0);
        assert!(l.len() >= 3);
        assert!(l.iter().all(|s| largura(s, 10.0) <= 80.0));
        assert_eq!(quebrar("a\nb", 10.0, 100.0), vec!["a", "b"]);
    }

    #[test]
    fn jpeg_sof() {
        // SOI, APP0 vazio, SOF0 640x360 3 comp.
        let mut j = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x02];
        j.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08, 0x01, 0x68, 0x02, 0x80, 0x03]);
        assert_eq!(jpeg_info(&j), Some((640, 360, 3)));
        assert_eq!(jpeg_info(b"nope"), None);
    }

    #[test]
    fn pdf_bem_formado() {
        let mut pdf = Pdf::new();
        let mut j = vec![0xFF, 0xD8];
        j.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x10, 0x00, 0x10, 0x01]);
        j.extend_from_slice(&[0xFF, 0xD9]);
        let im = pdf.jpeg(j).unwrap();
        {
            let p = pdf.nova_pagina();
            p.texto(50.0, 800.0, 12.0, true, PRETO, "Olá, relatório");
            p.imagem(im, 50.0, 500.0, 200.0, 200.0);
            p.poli(&[(0.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true, 1.0, VERDE);
        }
        pdf.nova_pagina()
            .texto(50.0, 800.0, 10.0, false, CINZA, "2");
        let b = pdf.bytes();
        let s = String::from_utf8_lossy(&b);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("/Count 2"));
        assert!(s.contains("/DCTDecode"));
        assert!(s.contains("/Im0 5 0 R"));
        assert!(s.trim_end().ends_with("%%EOF"));
        // xref aponta para o começo de cada objeto.
        let startxref: usize = s
            .rsplit("startxref\n")
            .next()
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(b[startxref..].starts_with(b"xref"));
        let primeira: usize = String::from_utf8_lossy(&b[startxref..])
            .lines()
            .nth(3)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert!(b[primeira..].starts_with(b"1 0 obj"));
    }

    #[test]
    fn nomes_de_arquivo() {
        assert!(nome_valido("relatorio-20261107-1530-loja.pdf"));
        assert!(!nome_valido("../x.pdf"));
        assert!(!nome_valido("a/b.pdf"));
        assert!(!nome_valido("x.txt"));
    }

    #[test]
    fn textos_do_pdf_no_arquivo_de_traducao() {
        for k in CHAVES_PDF {
            assert!(
                crate::i18n::existe(k),
                "falta a chave {k} em web/i18n/pt-BR.json"
            );
        }
    }
}
