//! CamFácil IA — binário principal.
//!
//! Sobe a dashboard local numa porta livre (sem conflitar com PDV/NVR do cliente)
//! e expõe a API da fase 1. Ver `docs/sdd/SDD-01-nucleo.md`.

mod api;
mod assets;
mod autoinicio;
mod elo;
mod ia;
mod notificacoes;
mod painel;
mod resumo;
mod tailscale;
mod tv;
mod video;

use cf_core::{paths, ports, store::Store};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

pub struct App {
    pub store: Arc<Store>,
    pub elo: Arc<elo::Elo>,
    pub video: Arc<video::Video>,
    pub tailscale: tailscale::Estado,
    pub telegram: std::sync::OnceLock<Arc<cf_notify::Telegram>>,
    pub ia: Arc<ia::Ia>,
}

pub type Shared = Arc<App>;

fn main() {
    // Runtime de uma thread: o núcleo ocioso precisa ser levíssimo. Trabalho pesado
    // (benchmark, IA) roda em threads próprias.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("não foi possível iniciar o runtime");
    if let Err(e) = rt.block_on(run()) {
        eprintln!("CamFácil IA: erro: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Provedor de criptografia do TLS (conexão segura com o site).
    let _ = rustls::crypto::ring::default_provider().install_default();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("camfacil {}", cf_core::VERSION);
        return Ok(());
    }
    if args.iter().any(|a| a == "--autoiniciar") {
        match autoinicio::definir(true) {
            Ok(()) => println!("O CamFácil IA vai abrir junto com o computador."),
            Err(e) => println!("Não deu para ligar a inicialização automática: {e}"),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--testar-site") {
        let site =
            std::env::var("CAMFACIL_SITE").unwrap_or_else(|_| cf_core::elo::URL_PADRAO.into());
        println!("Testando conexão com {site}…");
        match elo::testar(&site).await {
            Ok(m) => println!("OK: {m}"),
            Err(e) => println!("Falhou: {e}"),
        }
        return Ok(());
    }

    let dir = paths::data_dir();
    std::fs::create_dir_all(&dir)?;
    let store = Arc::new(Store::open(&dir.join("camfacil.db"))?);

    // Só na própria máquina, por padrão. Expor na rede é opção explícita.
    let lan = args.iter().any(|a| a == "--lan");
    let ip: IpAddr = if lan {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    };

    let saved = store.get("port")?.and_then(|p| p.parse::<u16>().ok());
    let chosen = ports::choose(ip, saved)?;
    store.set("port", &chosen.port.to_string())?;
    chosen.listener.set_nonblocking(true)?;
    let listener = tokio::net::TcpListener::from_std(chosen.listener)?;

    let url = format!("http://localhost:{}", chosen.port);
    println!("CamFácil IA {} — dashboard em {url}", cf_core::VERSION);
    if chosen.moved {
        println!(
            "A porta preferida estava ocupada por outro programa. Usando a {} para não conflitar.",
            chosen.port
        );
    }
    println!("Dados em {}", dir.display());

    if !args.iter().any(|a| a == "--no-browser") {
        open_browser(&url);
    }

    let elo = elo::Elo::carregar(store.clone());
    elo.iniciar_se_ativo();
    let video = video::Video::new(store.clone());
    video.iniciar_todas();
    let app: Shared = Arc::new(App {
        store,
        elo,
        video,
        tailscale: tailscale::Estado::default(),
        telegram: std::sync::OnceLock::new(),
        ia: Arc::new(ia::Ia::new(&dir)),
    });
    // Nível médio: a thread do detector dorme até haver módulo médio ligado e
    // runtime+modelo baixados (SDD-02).
    ia::detector::iniciar(app.ia.clone(), app.video.clone(), app.store.clone());
    // Telegram (SDD-04): alertas com foto e comandos, só quando há token salvo.
    match cf_notify::Telegram::iniciar(
        app.store.clone(),
        Arc::new(notificacoes::Ponte::new(Arc::downgrade(&app))),
    ) {
        Ok(tg) => {
            let n = tg.clone();
            video::definir_notificador(Arc::new(move |a| n.notificar(a)));
            let _ = app.telegram.set(tg);
        }
        Err(e) => eprintln!("Telegram desligado: {e}"),
    }
    app.elo.definir_resumo(resumo::montar(Arc::downgrade(&app)));
    let rede = lan
        .then(|| cf_core::conectar::ip_local().map(|ip| format!("http://{ip}:{}", chosen.port)))
        .flatten();
    painel::iniciar(app.clone(), url, rede);
    let router = api::router(app.clone());
    // Acesso de fora: escuta também no IP Tailscale quando ele existir (ADR-0005).
    tailscale::vigiar(app, router.clone(), chosen.port, lan);
    let r = axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
    painel::encerrar();
    r?;
    Ok(())
}

/// Abre o navegador padrão sem depender de crates extras.
fn open_browser(url: &str) {
    #[cfg(windows)]
    let cmd = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = std::process::Command::new("xdg-open")
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    if cmd.is_err() {
        println!("Abra no navegador: {url}");
    }
}
