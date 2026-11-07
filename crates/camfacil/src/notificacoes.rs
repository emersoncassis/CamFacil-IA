//! O que o bot do Telegram enxerga do programa (SDD-04): status, resumo do
//! dia, foto da câmera e liga/desliga de módulos. Nunca senha nem RTSP.

use crate::App;
use cf_notify::servico::{hora_com_offset, offset_local_s};
use cf_notify::Anfitriao;
use std::collections::BTreeMap;
use std::sync::Weak;

pub struct Ponte {
    app: Weak<App>,
}

impl Ponte {
    pub fn new(app: Weak<App>) -> Self {
        Self { app }
    }
}

fn agora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Nome do estado de saúde em pt-BR (mesmas chaves do `saude.rs`).
pub fn saude_pt(chave: &str) -> &'static str {
    match chave {
        "ok" => "imagem ok",
        "escura" => "imagem escura",
        "coberta" => "câmera coberta",
        "desfocada" => "imagem desfocada",
        "sem_sinal" => "sem sinal",
        _ => "—",
    }
}

impl Anfitriao for Ponte {
    fn status(&self) -> String {
        let Some(app) = self.app.upgrade() else {
            return "Programa encerrando.".into();
        };
        let cams = app.store.cameras().unwrap_or_default();
        let mut linhas = vec![format!(
            "CamFácil IA {} · {}",
            cf_core::VERSION,
            hora_local(&app)
        )];
        if cams.is_empty() {
            linhas.push("Nenhuma câmera ligada.".into());
        }
        for (i, c) in cams.iter().filter(|c| c.ativa).enumerate() {
            let e = app
                .video
                .canal(c.id)
                .map(|k| k.estado())
                .unwrap_or_default();
            let sit = if e.online {
                format!("ao vivo · {} q/s · {}", e.fps, saude_pt(&e.saude))
            } else {
                format!(
                    "fora do ar · {}",
                    e.erro.unwrap_or_else(|| "conectando".into())
                )
            };
            linhas.push(format!("{}. {} — {sit}", i + 1, c.nome));
        }
        let m = crate::video::modulos(&app.store);
        let ligados: Vec<&str> = crate::video::MODULOS
            .iter()
            .filter(|(id, _, _, _)| m.ligado(id))
            .map(|(_, nome, _, _)| *nome)
            .collect();
        linhas.push(format!(
            "Módulos ligados: {}",
            if ligados.is_empty() {
                "nenhum".to_string()
            } else {
                ligados.join(", ")
            }
        ));
        let s = app.telegram.get().map(|t| t.situacao());
        if let Some(s) = s {
            if s.na_fila > 0 {
                linhas.push(format!("Alertas aguardando envio: {}", s.na_fila));
            }
        }
        linhas.join("\n")
    }

    fn resumo(&self) -> String {
        let Some(app) = self.app.upgrade() else {
            return "Programa encerrando.".into();
        };
        let off = offset_local_s(&app.store);
        let meia_noite = (agora() + off).div_euclid(86_400) * 86_400 - off;
        let ev = app.store.eventos_desde(meia_noite).unwrap_or_default();
        let cams = app.store.cameras().unwrap_or_default();
        let nome_cam = |id: i64| {
            cams.iter()
                .find(|c| c.id == id)
                .map(|c| c.nome.clone())
                .unwrap_or_else(|| format!("câmera {id}"))
        };
        let regras: BTreeMap<i64, String> = cams
            .iter()
            .flat_map(|c| app.store.regras(c.id).unwrap_or_default())
            .map(|r| (r.id, format!("{} ({})", r.nome, nome_cam(r.camera_id))))
            .collect();
        let mut por_regra: BTreeMap<String, u32> = BTreeMap::new();
        let mut saude: Vec<String> = Vec::new();
        for e in ev.iter().rev() {
            match e.tipo.as_str() {
                "linha_cruzada" | "zona_ocupada" => {
                    let nome = e
                        .regra_id
                        .and_then(|r| regras.get(&r).cloned())
                        .unwrap_or_else(|| "regra apagada".into());
                    *por_regra.entry(nome).or_default() += 1;
                }
                "saude" if e.detalhe != "ok" => saude.push(format!(
                    "{} {} — {}",
                    hora_com_offset(e.quando, off),
                    nome_cam(e.camera_id),
                    saude_pt(&e.detalhe)
                )),
                _ => {}
            }
        }
        let mut linhas = vec!["Resumo de hoje".to_string()];
        if por_regra.is_empty() {
            linhas.push("Nenhum alerta de regra.".into());
        } else {
            for (r, n) in &por_regra {
                linhas.push(format!("• {r}: {n}"));
            }
        }
        if !saude.is_empty() {
            linhas.push(String::new());
            linhas.push("Saúde das câmeras:".into());
            let n = saude.len();
            linhas.extend(saude.into_iter().rev().take(10));
            if n > 10 {
                linhas.push(format!("… e mais {}", n - 10));
            }
        }
        linhas.join("\n")
    }

    fn cameras(&self) -> Vec<(i64, String)> {
        self.app
            .upgrade()
            .and_then(|a| a.store.cameras().ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|c| c.ativa)
            .map(|c| (c.id, c.nome))
            .collect()
    }

    fn foto(&self, camera_id: i64) -> Option<Vec<u8>> {
        let app = self.app.upgrade()?;
        let canal = app.video.canal(camera_id)?;
        let j = canal.ultimo.lock().unwrap().clone()?;
        Some(j.as_ref().clone())
    }

    fn modulo(&self, nome: &str, ligado: bool) -> Result<String, String> {
        let app = self.app.upgrade().ok_or("Programa encerrando.")?;
        let (id, nome_bonito) = crate::video::definir_modulo(&app.store, nome, ligado)?;
        let _ = id;
        Ok(format!(
            "{nome_bonito}: {}.",
            if ligado { "ligado" } else { "desligado" }
        ))
    }
}

fn hora_local(app: &App) -> String {
    cf_notify::servico::hora_local(&app.store)
}
