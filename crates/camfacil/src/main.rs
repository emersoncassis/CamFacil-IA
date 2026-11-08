//! CamFácil IA — binário principal.
//!
//! Sobe a dashboard local numa porta livre (sem conflitar com PDV/NVR do cliente)
//! e expõe a API da fase 1. Ver `docs/sdd/SDD-01-nucleo.md`.

mod alertas_camera;
mod api;
mod arquivo;
mod assets;
mod autoinicio;
mod autoip;
mod backup;
mod botoes;
mod elo;
mod estresse;
mod estudio;
mod filhos;
mod hardware;
mod i18n;
mod ia;
mod integracoes;
mod mcp;
mod mcp_https;
mod ngrok;
mod notificacoes;
mod painel;
mod rede;
mod relatorio;
mod resumo;
mod tailscale;
mod termos;
mod tv;
mod video;
mod visor;

use cf_core::{paths, ports, store::Store};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

pub struct App {
    pub store: Arc<Store>,
    pub elo: Arc<elo::Elo>,
    pub video: Arc<video::Video>,
    pub tailscale: tailscale::Estado,
    pub telegram: std::sync::OnceLock<Arc<cf_notify::Telegram>>,
    pub whatsapp: std::sync::OnceLock<Arc<cf_notify::WhatsApp>>,
    pub webhook: std::sync::OnceLock<Arc<cf_notify::Webhook>>,
    pub arquivo: std::sync::OnceLock<Arc<arquivo::Arquivo>>,
    pub ia: Arc<ia::Ia>,
    pub estudio: estudio::Estudio,
    pub coleta: Arc<estudio::Coleta>,
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
    if args.iter().any(|a| a == "--mcp") {
        // Ponte MCP por stdio para o programa já aberto (ADR 0012).
        return mcp::ponte_stdio().await;
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

    // ngrok e ffmpeg morrem junto com o programa, mesmo fechado à força.
    filhos::iniciar();
    let dir = paths::data_dir();
    if paths::apagar_se_pedido() {
        println!("Todos os dados do CamFácil IA foram apagados. Começando do zero.");
    }
    std::fs::create_dir_all(&dir)?;
    filhos::limpar_orfaos(&dir);
    let store = Arc::new(Store::open(&dir.join("camfacil.db"))?);
    api::tirar_ip_dos_nomes(&store);

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
        whatsapp: std::sync::OnceLock::new(),
        webhook: std::sync::OnceLock::new(),
        arquivo: std::sync::OnceLock::new(),
        ia: Arc::new(ia::Ia::new(&dir)),
        estudio: estudio::Estudio::new(&dir),
        coleta: Arc::default(),
    });
    // Modelo de estudo ligado antes de fechar o programa volta ligado (SDD-07).
    estudio::sincronizar(&app);
    // Nível médio: a thread do detector dorme até haver módulo médio ligado e
    // runtime+modelo baixados (SDD-02).
    ia::detector::iniciar(app.ia.clone(), app.video.clone(), app.store.clone());
    // Notificações (SDD-04): Telegram e WhatsApp recebem o mesmo alerta; cada
    // serviço só age quando está configurado.
    let ponte: Arc<dyn cf_notify::Anfitriao> =
        Arc::new(notificacoes::Ponte::new(Arc::downgrade(&app)));
    let tg = cf_notify::Telegram::iniciar(app.store.clone(), ponte.clone())
        .map_err(|e| eprintln!("Telegram desligado: {e}"))
        .ok();
    let wa = cf_notify::WhatsApp::iniciar(app.store.clone(), ponte)
        .map_err(|e| eprintln!("WhatsApp desligado: {e}"))
        .ok();
    if let Some(t) = &tg {
        let _ = app.telegram.set(t.clone());
    }
    if let Some(w) = &wa {
        let _ = app.whatsapp.set(w.clone());
    }
    let wh = cf_notify::Webhook::iniciar(app.store.clone())
        .map_err(|e| eprintln!("Webhook desligado: {e}"))
        .ok();
    if let Some(w) = &wh {
        let _ = app.webhook.set(w.clone());
    }
    // Arquivo de alertas: foto guardada no PC (+ canal do Telegram, se houver);
    // o webhook recebe o link dela.
    let arq = arquivo::Arquivo::iniciar(app.store.clone());
    if let Some(t) = &tg {
        arq.definir_telegram(t.clone());
    }
    let _ = app.arquivo.set(arq.clone());
    let app_fraco = Arc::downgrade(&app);
    video::definir_notificador(Arc::new(move |a: cf_notify::Alerta| {
        let foto_url = arq.registrar(&a).and_then(|id| {
            let app = app_fraco.upgrade()?;
            Some(format!(
                "{}/api/arquivo/{id}.jpg",
                integracoes::url_base_local(&app)
            ))
        });
        if let Some(w) = &wh {
            w.notificar(&a, foto_url);
        }
        if let Some(w) = &wa {
            w.notificar(a.clone());
        }
        if let Some(t) = &tg {
            t.notificar(a);
        }
    }));
    app.elo.definir_resumo(resumo::montar(Arc::downgrade(&app)));
    // Monitor de hardware com log (SDD-05): uma amostra por minuto.
    hardware::iniciar(Arc::downgrade(&app));
    // Alertas da própria câmera/gravador (só escuta com o módulo ligado).
    alertas_camera::iniciar(Arc::downgrade(&app));
    // Rede (SDD-05): foto da rede, diagnóstico e diagrama, sob demanda.
    rede::iniciar(app.clone());
    let rede = lan
        .then(|| cf_core::conectar::ip_local().map(|ip| format!("http://{ip}:{}", chosen.port)))
        .flatten();
    painel::iniciar(app.clone(), url, rede);
    let router = api::router(app.clone());
    // Acesso de fora: escuta também no IP Tailscale quando ele existir (ADR-0005).
    tailscale::vigiar(app, router.clone(), chosen.port, lan);
    let r = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await;
    painel::encerrar();
    // O túnel do ngrok cai junto com o programa.
    ngrok::parar();
    r?;
    Ok(())
}

/// Abre o navegador padrão sem depender de crates extras.
fn open_browser(url: &str) {
    #[cfg(windows)]
    let cmd = {
        use std::os::windows::process::CommandExt;
        // O navegador continua aberto depois que o programa fecha.
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .creation_flags(filhos::FORA_DO_JOB | 0x0800_0000)
            .spawn()
    };
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
