//! Resumo do que este computador está vendo, para o site (Agente-Cam-Elo-IA).
//! Sem imagem, sem senha, sem endereço RTSP: só nomes, situação e números.
//! O único endereço é o do Tailscale (100.x), que só abre dentro da rede
//! Tailscale do técnico.

use crate::App;
use serde_json::{json, Value};
use std::sync::{Mutex, Weak};
use std::time::Instant;

pub fn montar(app: Weak<App>) -> crate::elo::Resumo {
    let inicio = Instant::now();
    let sys = Mutex::new(sysinfo::System::new().ok());
    let pid = sysinfo::get_current_pid().ok();
    Box::new(move || {
        let Some(app) = app.upgrade() else {
            return Value::Null;
        };
        let (cpu, usada, total, proprio) = match sys.lock().unwrap().as_mut() {
            Some(s) => {
                s.refresh_cpu_usage();
                s.refresh_memory();
                let proprio = pid.and_then(|p| {
                    s.refresh_processes_specifics(
                        sysinfo::ProcessesToUpdate::Some(&[p]),
                        true,
                        sysinfo::ProcessRefreshKind::nothing().with_memory(),
                    );
                    s.process(p).map(|p| p.memory() / 1_048_576)
                });
                (
                    s.global_cpu_usage(),
                    s.used_memory() / 1_048_576,
                    s.total_memory() / 1_048_576,
                    proprio,
                )
            }
            None => (0.0, 0, 0, None),
        };
        let cams = app.store.cameras().unwrap_or_default();
        let nome_de = |id: i64| {
            cams.iter()
                .find(|c| c.id == id)
                .map(|c| c.nome.clone())
                .unwrap_or_default()
        };
        let cameras: Vec<Value> = cams
            .iter()
            .filter(|c| c.ativa)
            .map(|c| {
                let e = app
                    .video
                    .canal(c.id)
                    .map(|k| k.estado())
                    .unwrap_or_default();
                json!({
                    "id": c.id,
                    "nome": c.nome,
                    "online": e.online,
                    "fps": (e.fps * 10.0).round() / 10.0,
                    "erro": e.erro,
                    "saude": e.saude,
                    "regras": app.store.regras(c.id).map(|r| r.len()).unwrap_or(0),
                })
            })
            .collect();
        let regras_nome = |cam: i64, regra: Option<i64>| -> String {
            regra
                .and_then(|r| {
                    app.store
                        .regras(cam)
                        .ok()?
                        .into_iter()
                        .find(|x| x.id == r)
                        .map(|x| x.nome)
                })
                .unwrap_or_default()
        };
        let alertas: Vec<Value> = app
            .store
            .eventos(10)
            .unwrap_or_default()
            .into_iter()
            .map(|ev| {
                json!({
                    "camera": nome_de(ev.camera_id),
                    "regra": regras_nome(ev.camera_id, ev.regra_id),
                    "tipo": ev.tipo,
                    "quando": ev.quando,
                })
            })
            .collect();
        let m = crate::video::modulos(&app.store);
        let modulos: Vec<&str> = [
            ("movimento", m.movimento),
            ("cerca", m.cerca),
            ("saude", m.saude),
        ]
        .into_iter()
        .filter_map(|(n, on)| on.then_some(n))
        .collect();
        let tailscale = app
            .tailscale
            .escutando()
            .map(|ip| json!({ "url": crate::api::url_tailscale(&app, ip) }));
        json!({
            "cameras": cameras,
            "alertas": alertas,
            "cpu": cpu,
            "ram_usada_mb": usada,
            "ram_total_mb": total,
            "proprio_mb": proprio,
            "sistema": sysinfo::System::long_os_version().unwrap_or_default(),
            "maquina": sysinfo::System::host_name().unwrap_or_default(),
            "versao": cf_core::VERSION,
            "ligado_s": inicio.elapsed().as_secs(),
            "modulos": modulos,
            // Endereço para ver ao vivo de fora, só quando já está escutando no
            // IP Tailscale. O vídeo vai direto do cliente ao celular, sem nuvem.
            "tailscale": tailscale,
        })
    })
}
