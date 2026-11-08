//! Pasta de dados do CamFácil IA.
//!
//! Portátil: tudo fica na pasta `camfacil-dados`, **ao lado do executável**
//! (nada em pasta escondida do sistema nem no registro). Apagar essa pasta, ou
//! usar "Apagar todos os dados" em Manutenção, zera o programa.
//!
//! A variável `CAMFACIL_DATA` sobrepõe tudo (o app Android usa). Se a pasta do
//! executável não aceitar escrita (ex.: instalado em pasta protegida), cai na
//! pasta antiga: Windows `%PROGRAMDATA%\CamFacil`, Linux
//! `$XDG_DATA_HOME/camfacil` ou `~/.local/share/camfacil`.

use std::path::PathBuf;
use std::sync::OnceLock;

/// Nome da pasta de dados ao lado do executável.
pub const PASTA_PORTATIL: &str = "camfacil-dados";
/// Marca para apagar tudo no próximo início (o banco está aberto agora).
pub const MARCA_APAGAR: &str = ".apagar-tudo";

pub fn data_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        if let Some(p) = std::env::var_os("CAMFACIL_DATA") {
            return PathBuf::from(p);
        }
        ao_lado_do_executavel().unwrap_or_else(pasta_antiga)
    })
    .clone()
}

fn ao_lado_do_executavel() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.join(PASTA_PORTATIL);
    if dir.is_dir() {
        return Some(dir);
    }
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Onde as versões até a 0.7 guardavam os dados (pasta do sistema).
pub fn pasta_antiga() -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(p) = std::env::var_os("PROGRAMDATA") {
            return PathBuf::from(p).join("CamFacil");
        }
    }
    if let Some(p) = std::env::var_os("XDG_DATA_HOME") {
        return PathBuf::from(p).join("camfacil");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".local/share/camfacil");
    }
    PathBuf::from("camfacil-data")
}

/// Pasta antiga, se ainda existir e não for a pasta em uso.
pub fn pasta_antiga_existente() -> Option<PathBuf> {
    let antiga = pasta_antiga();
    (antiga.is_dir() && antiga != data_dir()).then_some(antiga)
}

/// Chamado no início, antes de abrir o banco: se o técnico pediu "Apagar
/// todos os dados", apaga a pasta de dados e a pasta antiga.
pub fn apagar_se_pedido() -> bool {
    let dir = data_dir();
    if !dir.join(MARCA_APAGAR).exists() {
        return false;
    }
    if let Some(antiga) = pasta_antiga_existente() {
        let _ = std::fs::remove_dir_all(antiga);
    }
    let _ = std::fs::remove_dir_all(&dir);
    true
}

/// Tamanho total de uma pasta, em bytes.
pub fn tamanho(dir: &std::path::Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    rd.flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => tamanho(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}
