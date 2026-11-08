//! Painel na janela do terminal: desenho, endereço e um "htop" da máquina.
//!
//! Só aparece quando a saída é um terminal de verdade (dois cliques no
//! Windows, ou rodando no shell). Em serviço ou com saída redirecionada,
//! fica só o texto simples. Redesenha a cada 2 s: custo desprezível.

use crate::Shared;
use std::fmt::Write as _;
use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

const VERDE: &str = "\x1b[92m";
const CINZA: &str = "\x1b[90m";
const BRANCO: &str = "\x1b[97m";
const VERMELHO: &str = "\x1b[91m";
const AMARELO: &str = "\x1b[93m";
const FIM: &str = "\x1b[0m";
const LIMPA_LINHA: &str = "\x1b[K";

/// Liga o painel numa thread própria. Devolve `false` se não há terminal.
pub fn iniciar(app: Shared, url: String, rede: Option<String>) -> bool {
    if std::env::var_os("CAMFACIL_SEM_PAINEL").is_some() || !std::io::stdout().is_terminal() {
        return false;
    }
    if !habilitar_cores() {
        return false;
    }
    std::thread::Builder::new()
        .name("painel".into())
        .stack_size(128 * 1024)
        .spawn(move || laco(app, url, rede))
        .is_ok()
}

fn laco(app: Shared, url: String, rede: Option<String>) {
    let inicio = Instant::now();
    let mut sys = sysinfo::System::new().ok();
    let pid = sysinfo::get_current_pid().ok();
    let mut out = std::io::stdout();
    // Título da janela e tela limpa uma vez; depois só reescreve por cima.
    let _ = write!(out, "\x1b]0;CamFácil IA\x07\x1b[2J\x1b[?25l");
    let mut tique = 0u64;
    loop {
        let (cpu, ram_usada, ram_total, minha) = match sys.as_mut() {
            Some(s) => {
                s.refresh_cpu_usage();
                s.refresh_memory();
                let minha = pid.and_then(|p| {
                    s.refresh_processes_specifics(
                        sysinfo::ProcessesToUpdate::Some(&[p]),
                        true,
                        sysinfo::ProcessRefreshKind::nothing().with_memory(),
                    );
                    s.process(p).map(|p| p.memory())
                });
                (
                    s.global_cpu_usage(),
                    s.used_memory(),
                    s.total_memory(),
                    minha,
                )
            }
            None => (0.0, 0, 0, None),
        };
        let tela = desenhar(&Dados {
            app: &app,
            url: &url,
            rede: rede.as_deref(),
            tique,
            ligado: inicio.elapsed(),
            cpu,
            ram_usada,
            ram_total,
            minha,
        });
        let _ = out.write_all(tela.as_bytes());
        let _ = out.flush();
        tique += 1;
        std::thread::sleep(Duration::from_secs(2));
    }
}

struct Dados<'a> {
    app: &'a Shared,
    url: &'a str,
    rede: Option<&'a str>,
    tique: u64,
    ligado: Duration,
    cpu: f32,
    ram_usada: u64,
    ram_total: u64,
    minha: Option<u64>,
}

fn desenhar(d: &Dados) -> String {
    let mut s = String::with_capacity(4096);
    s.push_str("\x1b[H");
    let rec = if d.tique.is_multiple_of(2) {
        format!("{VERMELHO}●{FIM}")
    } else {
        " ".into()
    };
    let v = cf_core::VERSION;
    let giro = girar(d.tique);
    let linhas_arte = [
        format!("{VERDE}      ________________{FIM}"),
        format!("{VERDE}     |  .--.          |\\{FIM}       {BRANCO}CamFácil IA{FIM}  {CINZA}v{v}{FIM}"),
        format!("{VERDE}     | ( {BRANCO}(){VERDE} )  {rec}{VERDE} REC  | >{FIM}      {CINZA}IA de vídeo para quem instala câmeras{FIM}"),
        format!("{VERDE}     |  '--'          |/{FIM}"),
        format!("{VERDE}     |________________|{FIM}        {VERDE}▸ rodando{FIM} {CINZA}{giro}{FIM}"),
        format!("{VERDE}            ||{FIM}"),
        format!("{VERDE}         ___||___{FIM}"),
    ];
    for l in linhas_arte {
        linha(&mut s, &l);
    }
    linha(&mut s, "");
    linha(&mut s, &format!("  {CINZA}{}{FIM}", "─".repeat(64)));

    linha(
        &mut s,
        &format!("  {CINZA}Dashboard{FIM}   {BRANCO}{}{FIM}", d.url),
    );
    if let Some(r) = d.rede {
        linha(
            &mut s,
            &format!(
                "  {CINZA}Na rede{FIM}     {BRANCO}{r}{FIM}  {CINZA}(celular e outros PCs){FIM}"
            ),
        );
    }
    let pct_ram = if d.ram_total > 0 {
        d.ram_usada as f32 * 100.0 / d.ram_total as f32
    } else {
        0.0
    };
    linha(
        &mut s,
        &format!(
            "  {CINZA}CPU{FIM}         {} {:>3.0}%",
            barra(d.cpu, 24),
            d.cpu
        ),
    );
    linha(
        &mut s,
        &format!(
            "  {CINZA}Memória{FIM}     {} {:.1} de {:.1} GB",
            barra(pct_ram, 24),
            d.ram_usada as f64 / 1_073_741_824.0,
            d.ram_total as f64 / 1_073_741_824.0
        ),
    );
    if let Some(m) = d.minha {
        linha(
            &mut s,
            &format!("  {CINZA}CamFácil IA{FIM} {} MB de memória", m / 1_048_576),
        );
    }
    if let Some(t) = crate::hardware::monitor()
        .and_then(|m| m.atual())
        .and_then(|a| a.temperatura)
    {
        linha(&mut s, &format!("  {CINZA}Temperatura{FIM} {t:.0} °C"));
    }

    // Câmeras
    let cams = d.app.store.cameras().unwrap_or_default();
    let mut on = 0;
    let mut detalhes = Vec::new();
    for c in cams.iter().filter(|c| c.ativa) {
        let e = d.app.video.canal(c.id).map(|k| k.estado());
        let nome: String = c.nome.chars().take(22).collect();
        match e {
            Some(e) if e.online => {
                on += 1;
                detalhes.push(format!(
                    "    {VERDE}●{FIM} {nome:<22} {CINZA}{:.1} q/s{FIM}",
                    e.fps
                ));
            }
            Some(e) => {
                let motivo: String = e
                    .erro
                    .unwrap_or_else(|| "conectando…".into())
                    .chars()
                    .take(40)
                    .collect();
                detalhes.push(format!(
                    "    {AMARELO}○{FIM} {nome:<22} {CINZA}{motivo}{FIM}"
                ));
            }
            None => detalhes.push(format!("    {CINZA}○ {nome:<22} parada{FIM}")),
        }
    }
    let total = detalhes.len();
    let resumo = match total {
        0 => format!("{CINZA}nenhuma ainda: abra a dashboard e clique em Ligar câmera{FIM}"),
        _ => format!(
            "{BRANCO}{on}{FIM} de {total} {}",
            if total == 1 {
                "conectada"
            } else {
                "conectadas"
            }
        ),
    };
    linha(&mut s, &format!("  {CINZA}Câmeras{FIM}     {resumo}"));
    for l in detalhes.iter().take(8) {
        linha(&mut s, l);
    }
    if total > 8 {
        linha(&mut s, &format!("    {CINZA}… e mais {}{FIM}", total - 8));
    }

    let elo = d.app.elo.status();
    let site = if elo.pareado {
        format!(
            "{VERDE}pareado{FIM}{}",
            if elo.conectado {
                ""
            } else {
                " (sem internet agora)"
            }
        )
    } else {
        format!("{CINZA}não pareado (opcional){FIM}")
    };
    linha(&mut s, &format!("  {CINZA}Site{FIM}        {site}"));
    let t = d.ligado.as_secs();
    linha(
        &mut s,
        &format!(
            "  {CINZA}Ligado há{FIM}   {:02}:{:02}:{:02}",
            t / 3600,
            t / 60 % 60,
            t % 60
        ),
    );
    linha(&mut s, &format!("  {CINZA}{}{FIM}", "─".repeat(64)));
    linha(
        &mut s,
        &format!("  {CINZA}Para desligar, feche esta janela ou aperte Ctrl+C.{FIM}"),
    );
    // Apaga o que sobrou de um desenho maior anterior.
    s.push_str("\x1b[J");
    s
}

fn linha(s: &mut String, texto: &str) {
    let _ = writeln!(s, "{texto}{LIMPA_LINHA}");
}

fn barra(pct: f32, largura: usize) -> String {
    let cheio = ((pct.clamp(0.0, 100.0) / 100.0) * largura as f32).round() as usize;
    let cor = if pct >= 85.0 {
        VERMELHO
    } else if pct >= 60.0 {
        AMARELO
    } else {
        VERDE
    };
    format!(
        "{cor}{}{CINZA}{}{FIM}",
        "█".repeat(cheio),
        "░".repeat(largura - cheio)
    )
}

fn girar(t: u64) -> &'static str {
    ["·  ", "·· ", "···", " ··", "  ·", "   "][(t % 6) as usize]
}

/// Mostra o cursor de novo ao sair.
pub fn encerrar() {
    if std::io::stdout().is_terminal() {
        println!("{FIM}\x1b[?25h");
        let _ = std::io::stdout().flush();
    }
}

/// Cores ANSI no console do Windows 10/11 precisam ser ligadas.
#[cfg(windows)]
fn habilitar_cores() -> bool {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(n: u32) -> *mut c_void;
        fn GetConsoleMode(h: *mut c_void, modo: *mut u32) -> i32;
        fn SetConsoleMode(h: *mut c_void, modo: u32) -> i32;
        fn SetConsoleOutputCP(cp: u32) -> i32;
    }
    const SAIDA: u32 = -11i32 as u32;
    const VT: u32 = 0x0004;
    // SAFETY: chamadas simples da API do console com handle do próprio processo.
    unsafe {
        let h = GetStdHandle(SAIDA);
        let mut modo = 0u32;
        if h.is_null() || GetConsoleMode(h, &mut modo) == 0 {
            return false;
        }
        SetConsoleOutputCP(65001);
        SetConsoleMode(h, modo | VT) != 0
    }
}

#[cfg(not(windows))]
fn habilitar_cores() -> bool {
    std::env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn barra_tem_largura_certa() {
        for p in [0.0, 12.5, 50.0, 99.9, 100.0, 150.0] {
            let b = barra(p, 20);
            assert_eq!(b.matches('█').count() + b.matches('░').count(), 20);
        }
    }
}
