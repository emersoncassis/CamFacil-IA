//! "Abrir junto com o computador".
//!
//! Windows: chave Run do usuário (sem administrador). Linux com tela: arquivo
//! em ~/.config/autostart. Linux sem tela (Raspberry, TV Box, servidor):
//! serviço systemd do usuário, já com `--lan` para abrir pelo celular.
//! Abre sempre com `--no-browser` para não abrir o navegador a cada boot.

#[cfg(not(target_os = "android"))]
use std::path::PathBuf;
#[cfg(not(target_os = "android"))]
use std::process::Command;

#[cfg(windows)]
const CHAVE: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const NOME: &str = "CamFacilIA";

#[cfg(not(target_os = "android"))]
fn exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("não achei o próprio programa: {e}"))
}

#[cfg(windows)]
pub fn ligado() -> bool {
    Command::new("reg")
        .args(["query", CHAVE, "/v", NOME])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
pub fn definir(ligar: bool) -> Result<(), String> {
    let st = if ligar {
        let valor = format!("\"{}\" --no-browser", exe()?.display());
        Command::new("reg")
            .args(["add", CHAVE, "/v", NOME, "/t", "REG_SZ", "/d", &valor, "/f"])
            .output()
    } else {
        Command::new("reg")
            .args(["delete", CHAVE, "/v", NOME, "/f"])
            .output()
    };
    match st {
        Ok(o) if o.status.success() || !ligar => Ok(()),
        _ => Err("o Windows não deixou gravar a inicialização automática".into()),
    }
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| "pasta do usuário não encontrada".into())
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn com_tela() -> bool {
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
fn arquivos() -> Result<(PathBuf, PathBuf), String> {
    let h = home()?;
    Ok((
        h.join(".config/autostart/camfacil.desktop"),
        h.join(".config/systemd/user/camfacil.service"),
    ))
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn ligado() -> bool {
    arquivos()
        .map(|(a, s)| a.is_file() || s.is_file())
        .unwrap_or(false)
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
pub fn definir(ligar: bool) -> Result<(), String> {
    let (desktop, servico) = arquivos()?;
    if !ligar {
        let _ = std::fs::remove_file(&desktop);
        if servico.is_file() {
            let _ = Command::new("systemctl")
                .args(["--user", "disable", "camfacil.service"])
                .output();
            let _ = std::fs::remove_file(&servico);
        }
        return Ok(());
    }
    let exe = exe()?;
    let grava = |p: &PathBuf, texto: String| -> Result<(), String> {
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        std::fs::write(p, texto).map_err(|e| e.to_string())
    };
    if com_tela() {
        grava(
            &desktop,
            format!(
                "[Desktop Entry]\nType=Application\nName=CamFácil IA\nComment=IA de vídeo para quem instala câmeras\nExec=\"{}\" --no-browser\nTerminal=true\nX-GNOME-Autostart-enabled=true\n",
                exe.display()
            ),
        )
    } else {
        grava(
            &servico,
            format!(
                "[Unit]\nDescription=CamFácil IA\nAfter=network-online.target\n\n[Service]\nExecStart=\"{}\" --lan --no-browser\nEnvironment=CAMFACIL_SEM_PAINEL=1\nRestart=on-failure\nRestartSec=5\n\n[Install]\nWantedBy=default.target\n",
                exe.display()
            ),
        )?;
        let ok = Command::new("systemctl")
            .args(["--user", "enable", "camfacil.service"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        // Sem login aberto (TV Box, Raspberry sem tela) o serviço do usuário
        // só sobe com "linger"; em muitas distros o próprio usuário pode ligar.
        let _ = Command::new("loginctl").arg("enable-linger").output();
        if ok {
            Ok(())
        } else {
            Err("não consegui ativar o serviço (systemd do usuário indisponível)".into())
        }
    }
}

#[cfg(target_os = "macos")]
pub fn ligado() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn definir(_ligar: bool) -> Result<(), String> {
    Err("no Mac ainda não; adicione em Ajustes → Itens de Início".into())
}

// No Android quem liga no boot é o app (BootReceiver), que conta a escolha
// ao núcleo pela variável `CAMFACIL_BOOT` (ADR 0011).
#[cfg(target_os = "android")]
pub fn ligado() -> bool {
    std::env::var("CAMFACIL_BOOT").is_ok_and(|v| v == "1")
}

#[cfg(target_os = "android")]
pub fn definir(_ligar: bool) -> Result<(), String> {
    Err("no Android, ligue \"Iniciar com o aparelho\" na tela inicial do app CamFácil IA".into())
}
