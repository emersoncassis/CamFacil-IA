//! Leitura mínima de `.tgz` e `.zip` para tirar um único arquivo (a biblioteca
//! do ONNX Runtime) de dentro do pacote oficial, sem crate de tar/zip.

use miniz_oxide::inflate::decompress_to_vec;

/// Descompacta gzip (RFC 1952) inteiro em memória.
pub fn gunzip(dados: &[u8]) -> Result<Vec<u8>, String> {
    if dados.len() < 18 || dados[0] != 0x1f || dados[1] != 0x8b || dados[2] != 8 {
        return Err("não é um arquivo gzip".into());
    }
    let flags = dados[3];
    let mut i = 10;
    if flags & 0x04 != 0 {
        let xlen = u16::from_le_bytes([dados[i], dados[i + 1]]) as usize;
        i += 2 + xlen;
    }
    if flags & 0x08 != 0 {
        i += dados[i..]
            .iter()
            .position(|&b| b == 0)
            .ok_or("gzip truncado")?
            + 1;
    }
    if flags & 0x10 != 0 {
        i += dados[i..]
            .iter()
            .position(|&b| b == 0)
            .ok_or("gzip truncado")?
            + 1;
    }
    if flags & 0x02 != 0 {
        i += 2;
    }
    let fim = dados.len().saturating_sub(8);
    if i >= fim {
        return Err("gzip truncado".into());
    }
    decompress_to_vec(&dados[i..fim]).map_err(|e| format!("gzip inválido: {e:?}"))
}

/// Procura num tar (ustar/GNU) o primeiro arquivo cujo nome termina com
/// `sufixo` e devolve seu conteúdo.
pub fn tar_extrair(tar: &[u8], sufixo: &str) -> Option<Vec<u8>> {
    let mut pos = 0usize;
    let mut nome_longo: Option<String> = None;
    while pos + 512 <= tar.len() {
        let h = &tar[pos..pos + 512];
        if h.iter().all(|&b| b == 0) {
            break;
        }
        let nome_curto = texto(&h[0..100]);
        let prefixo = texto(&h[345..500]);
        let tipo = h[156];
        let tam = octal(&h[124..136]);
        let ini = pos + 512;
        let fim = ini.checked_add(tam)?.min(tar.len());
        let nome = nome_longo.take().unwrap_or_else(|| {
            if prefixo.is_empty() {
                nome_curto.clone()
            } else {
                format!("{prefixo}/{nome_curto}")
            }
        });
        match tipo {
            b'L' => {
                // Cabeçalho GNU com nome longo: vale para a próxima entrada.
                nome_longo = Some(texto(&tar[ini..fim]));
            }
            b'0' | 0 if nome.ends_with(sufixo) && !nome.contains(".dSYM") => {
                return Some(tar[ini..fim].to_vec());
            }
            _ => {}
        }
        pos = ini + tam.div_ceil(512) * 512;
    }
    None
}

fn texto(b: &[u8]) -> String {
    let fim = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..fim]).trim().to_string()
}

fn octal(b: &[u8]) -> usize {
    texto(b)
        .chars()
        .filter(|c| c.is_ascii_digit())
        .fold(0usize, |a, c| a * 8 + (c as u8 - b'0') as usize)
}

/// Procura num zip o primeiro arquivo cujo nome termina com `sufixo`
/// (métodos `store` e `deflate`) e devolve seu conteúdo.
pub fn zip_extrair(zip: &[u8], sufixo: &str) -> Option<Vec<u8>> {
    // Diretório central (fim do arquivo) dá tamanhos corretos mesmo quando os
    // cabeçalhos locais usam descritor de dados.
    let eocd = zip
        .windows(4)
        .rposition(|w| w == [0x50, 0x4b, 0x05, 0x06])?;
    let n = u16::from_le_bytes([zip[eocd + 10], zip[eocd + 11]]) as usize;
    let mut p = u32::from_le_bytes(zip[eocd + 16..eocd + 20].try_into().ok()?) as usize;
    for _ in 0..n {
        if zip.get(p..p + 4)? != [0x50, 0x4b, 0x01, 0x02] {
            return None;
        }
        let metodo = u16::from_le_bytes([zip[p + 10], zip[p + 11]]);
        let comp = u32::from_le_bytes(zip[p + 20..p + 24].try_into().ok()?) as usize;
        let nlen = u16::from_le_bytes([zip[p + 28], zip[p + 29]]) as usize;
        let elen = u16::from_le_bytes([zip[p + 30], zip[p + 31]]) as usize;
        let clen = u16::from_le_bytes([zip[p + 32], zip[p + 33]]) as usize;
        let local = u32::from_le_bytes(zip[p + 42..p + 46].try_into().ok()?) as usize;
        let nome = String::from_utf8_lossy(zip.get(p + 46..p + 46 + nlen)?).to_string();
        if nome.ends_with(sufixo) {
            if zip.get(local..local + 4)? != [0x50, 0x4b, 0x03, 0x04] {
                return None;
            }
            let ln = u16::from_le_bytes([zip[local + 26], zip[local + 27]]) as usize;
            let le = u16::from_le_bytes([zip[local + 28], zip[local + 29]]) as usize;
            let ini = local + 30 + ln + le;
            let dados = zip.get(ini..ini + comp)?;
            return match metodo {
                0 => Some(dados.to_vec()),
                8 => decompress_to_vec(dados).ok(),
                _ => None,
            };
        }
        p += 46 + nlen + elen + clen;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tar_simples(nome: &str, conteudo: &[u8]) -> Vec<u8> {
        let mut h = vec![0u8; 512];
        h[..nome.len()].copy_from_slice(nome.as_bytes());
        let tam = format!("{:011o}\0", conteudo.len());
        h[124..136].copy_from_slice(tam.as_bytes());
        h[156] = b'0';
        let mut t = h;
        t.extend_from_slice(conteudo);
        t.resize(t.len().div_ceil(512) * 512 + 1024, 0);
        t
    }

    #[test]
    fn le_tar_e_gzip() {
        let tar = tar_simples("x/lib/libonnxruntime.so.1.20.1", b"ELF-fake");
        assert_eq!(
            tar_extrair(&tar, "libonnxruntime.so.1.20.1").as_deref(),
            Some(&b"ELF-fake"[..])
        );
        assert!(tar_extrair(&tar, "onnxruntime.dll").is_none());
        // gzip "store" montado à mão: cabeçalho + bloco deflate não comprimido.
        let mut gz = vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3];
        let n = tar.len() as u16;
        gz.push(1); // bloco final, sem compressão
        gz.extend_from_slice(&n.to_le_bytes());
        gz.extend_from_slice(&(!n).to_le_bytes());
        gz.extend_from_slice(&tar);
        gz.extend_from_slice(&[0; 8]);
        assert_eq!(gunzip(&gz).unwrap(), tar);
        assert!(gunzip(b"nada").is_err());
    }

    #[test]
    fn le_zip_store() {
        let nome = b"onnxruntime-win-x64/lib/onnxruntime.dll";
        let dados = b"MZ-fake";
        let mut z = Vec::new();
        // Cabeçalho local
        z.extend_from_slice(&[
            0x50, 0x4b, 0x03, 0x04, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        z.extend_from_slice(&(dados.len() as u32).to_le_bytes());
        z.extend_from_slice(&(dados.len() as u32).to_le_bytes());
        z.extend_from_slice(&(nome.len() as u16).to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        z.extend_from_slice(nome);
        z.extend_from_slice(dados);
        let cd = z.len();
        // Diretório central
        z.extend_from_slice(&[
            0x50, 0x4b, 0x01, 0x02, 20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        z.extend_from_slice(&(dados.len() as u32).to_le_bytes());
        z.extend_from_slice(&(dados.len() as u32).to_le_bytes());
        z.extend_from_slice(&(nome.len() as u16).to_le_bytes());
        z.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        z.extend_from_slice(&0u32.to_le_bytes()); // offset do cabeçalho local
        z.extend_from_slice(nome);
        let cd_len = z.len() - cd;
        z.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 1, 0, 1, 0]);
        z.extend_from_slice(&(cd_len as u32).to_le_bytes());
        z.extend_from_slice(&(cd as u32).to_le_bytes());
        z.extend_from_slice(&[0, 0]);
        assert_eq!(
            zip_extrair(&z, "onnxruntime.dll").as_deref(),
            Some(&dados[..])
        );
        assert!(zip_extrair(&z, "outro.dll").is_none());
    }
}
