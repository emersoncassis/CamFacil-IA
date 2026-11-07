//! Pasta de dados do CamFácil IA.
//!
//! Windows: `%PROGRAMDATA%\CamFacil` · Linux: `$XDG_DATA_HOME/camfacil` ou
//! `~/.local/share/camfacil`. A variável `CAMFACIL_DATA` sobrepõe tudo.

use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("CAMFACIL_DATA") {
        return PathBuf::from(p);
    }
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
