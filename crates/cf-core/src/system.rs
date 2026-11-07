//! Informações da máquina para a tela Início.

use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Serialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: &'static str,
    pub hostname: String,
    pub cpu_brand: String,
    pub cpu_cores: usize,
    pub cpu_usage: f32,
    pub ram_total_mb: u64,
    pub ram_used_mb: u64,
    pub version: &'static str,
    /// RAM usada pelo próprio CamFácil IA, para acompanhar o orçamento.
    pub self_ram_mb: Option<u64>,
}

/// Lê o estado atual. Faz duas leituras de CPU separadas por um intervalo curto,
/// porque a primeira leitura do sysinfo é sempre zero.
pub async fn read() -> SystemInfo {
    let Ok(mut sys) = System::new() else {
        return SystemInfo::unknown();
    };
    sys.refresh_cpu_usage();
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    sys.refresh_cpu_all();
    sys.refresh_memory();

    let self_ram_mb = sysinfo::get_current_pid().ok().and_then(|pid| {
        sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[pid]),
            true,
            sysinfo::ProcessRefreshKind::nothing().with_memory(),
        );
        sys.process(pid).map(|p| p.memory() / 1_048_576)
    });

    let cpus = sys.cpus();
    SystemInfo {
        os: System::long_os_version().unwrap_or_else(|_| "desconhecido".into()),
        arch: std::env::consts::ARCH,
        hostname: System::host_name().unwrap_or_default(),
        cpu_brand: cpus
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_default(),
        cpu_cores: cpus.len(),
        cpu_usage: sys.global_cpu_usage(),
        ram_total_mb: sys.total_memory() / 1_048_576,
        ram_used_mb: sys.used_memory() / 1_048_576,
        version: crate::VERSION,
        self_ram_mb,
    }
}

impl SystemInfo {
    fn unknown() -> Self {
        Self {
            os: "desconhecido".into(),
            arch: std::env::consts::ARCH,
            hostname: String::new(),
            cpu_brand: String::new(),
            cpu_cores: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            cpu_usage: 0.0,
            ram_total_mb: 0,
            ram_used_mb: 0,
            version: crate::VERSION,
            self_ram_mb: None,
        }
    }
}
