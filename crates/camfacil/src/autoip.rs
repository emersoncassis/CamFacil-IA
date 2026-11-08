//! Autocorreção de IP da câmera pelo MAC (SDD-05).
//!
//! Quando a vigília da rede acha o MAC de uma câmera noutro IP e a câmera
//! tem a autocorreção ligada, o programa troca sozinho só o host da URL RTSP
//! (usuário, senha, porta e caminho iguais), reinicia o vídeo e confere se a
//! imagem volta. Se algo não fecha (porta fechada, MAC diferente, conflito de
//! IP, imagem que não volta, câmera oscilando), desfaz e cai no fluxo com
//! confirmação de sempre.
//!
//! A rede, o vídeo e o aviso ficam atrás do trait [`Ambiente`], para os
//! testes simularem a rede sem câmera de verdade.

use crate::rede::MudancaIp;
use cf_core::rede;
use cf_core::store::Store;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::Duration;

/// No máximo uma correção automática por câmera nesta janela (segundos).
pub const JANELA_ANTIRREPIQUE: i64 = 10 * 60;
/// Quanto esperar a imagem voltar depois de trocar o IP.
pub const ESPERA_IMAGEM: Duration = Duration::from_secs(15);
/// Chave em `settings` com o registro das correções automáticas (desta máquina).
pub const CHAVE_LOG: &str = "rede.autoip_log";
/// Quantos registros guardar.
const LOG_MAX: usize = 50;
/// Por quanto tempo uma correção aparece como aviso no Início e no Ao vivo.
pub const AVISO_S: i64 = 24 * 3600;

/// Uma linha do registro: data, câmera, IP antigo, novo e resultado.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegistroAuto {
    pub quando: i64,
    pub camera_id: i64,
    pub nome: String,
    pub de: String,
    pub para: String,
    /// `corrigido`, `falhou`, `conflito` ou `oscilou`.
    pub resultado: String,
    /// Motivo da falha, em pt-BR (vazio quando corrigiu).
    #[serde(default)]
    pub motivo: String,
}

/// O que a autocorreção precisa do mundo lá fora.
pub trait Ambiente: Sync {
    /// O IP responde na porta RTSP?
    fn porta_abre(&self, ip: &str, porta: u16) -> impl Future<Output = bool> + Send;
    /// MAC que a tabela ARP mostra agora para esse IP.
    fn mac_no_ip(&self, ip: &str) -> impl Future<Output = Option<String>> + Send;
    /// Reinicia o vídeo da câmera com o endereço dado.
    fn reiniciar_video(&self, id: i64, url: &str);
    /// Esperou até `limite` e a imagem voltou?
    fn imagem_voltou(&self, id: i64, limite: Duration) -> impl Future<Output = bool> + Send;
    /// Manda o aviso pelos canais ligados (Telegram, WhatsApp, webhook).
    fn notificar(&self, chave: String, texto: String);
}

/// Como terminou uma mudança de IP detectada.
#[derive(Debug, PartialEq)]
pub enum Desfecho {
    /// Trocou sozinho e a imagem voltou.
    Corrigida(RegistroAuto),
    /// Fica para o técnico confirmar (aviso + botão "Corrigir para…").
    /// Traz o registro quando a autocorreção tentou e não aplicou.
    Manual(Option<RegistroAuto>),
}

// ---------------- Registro ----------------

pub fn ler_log(store: &Store) -> Vec<RegistroAuto> {
    store
        .get(CHAVE_LOG)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn gravar_log(store: &Store, r: &RegistroAuto) {
    let mut log = ler_log(store);
    log.insert(0, r.clone());
    log.truncate(LOG_MAX);
    let _ = store.set(CHAVE_LOG, &serde_json::to_string(&log).unwrap_or_default());
}

/// Antirrepique: pode corrigir esta câmera agora? Não, se já houve uma
/// correção automática dela há menos de [`JANELA_ANTIRREPIQUE`].
pub fn pode_corrigir(log: &[RegistroAuto], camera_id: i64, agora: i64) -> bool {
    !log.iter().any(|r| {
        r.camera_id == camera_id
            && r.resultado == "corrigido"
            && agora.saturating_sub(r.quando) < JANELA_ANTIRREPIQUE
    })
}

/// Avisos recentes para o Início e o Ao vivo: correções e desligamentos por
/// oscilação nas últimas 24 h.
pub fn avisos_recentes(log: &[RegistroAuto], agora: i64) -> Vec<RegistroAuto> {
    log.iter()
        .filter(|r| matches!(r.resultado.as_str(), "corrigido" | "oscilou"))
        .filter(|r| agora.saturating_sub(r.quando) < AVISO_S)
        .cloned()
        .collect()
}

// ---------------- A correção ----------------

/// Tenta corrigir sozinho uma mudança de IP. `conflito`: o IP novo está em
/// conflito (dois MACs) ou já é de outra câmera cadastrada.
pub async fn tentar<A: Ambiente>(
    amb: &A,
    store: &Store,
    m: &MudancaIp,
    conflito: bool,
    agora: i64,
) -> Desfecho {
    let Some(cam) = store.camera(m.camera_id).ok().flatten() else {
        return Desfecho::Manual(None);
    };
    if cam.fonte != "rtsp" || !cam.auto_ip || !cam.mac_conhecido() || !cam.ativa {
        return Desfecho::Manual(None);
    }
    let reg = |resultado: &str, motivo: String| RegistroAuto {
        quando: agora,
        camera_id: cam.id,
        nome: cam.nome.clone(),
        de: m.de.clone(),
        para: m.para.clone(),
        resultado: resultado.into(),
        motivo,
    };
    let falhar = |r: RegistroAuto, alerta: Option<String>| {
        let _ = store.registrar_evento(
            cam.id,
            None,
            "rede",
            &format!(
                "IP não corrigido sozinho: {} → {} ({})",
                r.de, r.para, r.motivo
            ),
            agora,
        );
        gravar_log(store, &r);
        if let Some(texto) = alerta {
            amb.notificar(format!("rede:{}", cam.id), texto);
        }
        Desfecho::Manual(Some(r))
    };

    if conflito {
        let r = reg(
            "conflito",
            format!(
                "conflito de IP em {}: confirme na tela Rede antes de trocar",
                m.para
            ),
        );
        return falhar(r, None);
    }

    if !pode_corrigir(&ler_log(store), cam.id, agora) {
        let _ = store.definir_auto_ip(cam.id, false);
        let r = reg(
            "oscilou",
            "a câmera trocou de IP de novo em menos de 10 min; desliguei a correção automática dela"
                .into(),
        );
        let texto = format!(
            "Câmera {} está trocando de IP sem parar ({} → {}). Desliguei a correção automática dela: fixe o IP na câmera ou reserve no roteador.",
            cam.nome, m.de, m.para
        );
        return falhar(r, Some(texto));
    }

    let Some((_, porta)) = rede::host_da_url(&cam.url) else {
        return Desfecho::Manual(None);
    };
    let alerta = |motivo: &str| {
        format!(
            "Câmera {} mudou de IP ({} → {}), mas não deu para corrigir sozinho: {motivo}. Confira na tela Rede.",
            cam.nome, m.de, m.para
        )
    };

    // (a) O IP novo responde na porta RTSP e o MAC continua o mesmo lá?
    if !amb.porta_abre(&m.para, porta).await {
        let motivo = format!("{} não responde na porta {porta}", m.para);
        return falhar(reg("falhou", motivo.clone()), Some(alerta(&motivo)));
    }
    let mac_agora = amb
        .mac_no_ip(&m.para)
        .await
        .and_then(|x| rede::normalizar_mac(&x));
    if mac_agora.as_deref() != Some(cam.mac.as_str()) {
        let motivo = format!("o MAC em {} não é mais o da câmera", m.para);
        return falhar(reg("falhou", motivo.clone()), Some(alerta(&motivo)));
    }

    // (b) Troca só o host e (c) reinicia o vídeo.
    let Some(nova) = rede::trocar_host(&cam.url, &m.para) else {
        return Desfecho::Manual(None);
    };
    if store.atualizar_url_camera(cam.id, &nova).is_err() {
        return Desfecho::Manual(None);
    }
    amb.reiniciar_video(cam.id, &nova);
    if amb.imagem_voltou(cam.id, ESPERA_IMAGEM).await {
        // (d) Voltou: evento, registro e aviso curto.
        let _ = store.registrar_evento(
            cam.id,
            None,
            "rede",
            &format!("IP corrigido sozinho: {} → {}", m.de, m.para),
            agora,
        );
        let r = reg("corrigido", String::new());
        gravar_log(store, &r);
        amb.notificar(
            format!("rede:{}", cam.id),
            format!(
                "Câmera {} mudou de IP e foi corrigida sozinha ({}).",
                cam.nome, m.para
            ),
        );
        return Desfecho::Corrigida(r);
    }

    // (e) Não voltou: desfaz e devolve para o técnico confirmar.
    let _ = store.atualizar_url_camera(cam.id, &cam.url);
    amb.reiniciar_video(cam.id, &cam.url);
    let motivo = format!(
        "a imagem não voltou em {} s; voltei para {}",
        ESPERA_IMAGEM.as_secs(),
        m.de
    );
    falhar(reg("falhou", motivo.clone()), Some(alerta(&motivo)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    /// Rede simulada: quem responde na porta, ARP e se a imagem volta.
    #[derive(Default)]
    struct RedeFalsa {
        portas: Vec<(String, u16)>,
        arp: BTreeMap<String, String>,
        imagem: bool,
        reinicios: Mutex<Vec<String>>,
        avisos: Mutex<Vec<String>>,
    }

    impl Ambiente for RedeFalsa {
        async fn porta_abre(&self, ip: &str, porta: u16) -> bool {
            self.portas.iter().any(|(i, p)| i == ip && *p == porta)
        }
        async fn mac_no_ip(&self, ip: &str) -> Option<String> {
            self.arp.get(ip).cloned()
        }
        fn reiniciar_video(&self, _id: i64, url: &str) {
            self.reinicios.lock().unwrap().push(url.to_string());
        }
        async fn imagem_voltou(&self, _id: i64, _limite: Duration) -> bool {
            self.imagem
        }
        fn notificar(&self, _chave: String, texto: String) {
            self.avisos.lock().unwrap().push(texto);
        }
    }

    const MAC: &str = "58:10:8C:00:00:01";
    const URL: &str =
        "rtsp://admin:S3nh@:%23!@192.168.1.108:8554/cam/realmonitor?channel=1&subtype=0";

    fn cenario(auto: bool) -> (Store, i64, MudancaIp) {
        let s = Store::memory().unwrap();
        let id = s.criar_camera("Portão", "rtsp", URL).unwrap();
        s.definir_mac_camera(id, MAC).unwrap();
        s.definir_auto_ip(id, auto).unwrap();
        let m = MudancaIp {
            camera_id: id,
            nome: "Portão".into(),
            de: "192.168.1.108".into(),
            para: "192.168.1.120".into(),
            mac: MAC.into(),
            quando: 1000,
        };
        (s, id, m)
    }

    fn rede_ok() -> RedeFalsa {
        RedeFalsa {
            portas: vec![("192.168.1.120".into(), 8554)],
            arp: [("192.168.1.120".to_string(), "58-10-8c-00-00-01".to_string())].into(),
            imagem: true,
            ..Default::default()
        }
    }

    fn eventos(s: &Store) -> Vec<String> {
        s.eventos_tipo("rede", 20)
            .unwrap()
            .into_iter()
            .map(|e| e.detalhe)
            .collect()
    }

    #[test]
    fn troca_so_o_host_preservando_usuario_senha_porta_e_caminho() {
        let nova = rede::trocar_host(URL, "192.168.1.120").unwrap();
        assert_eq!(
            nova,
            "rtsp://admin:S3nh@:%23!@192.168.1.120:8554/cam/realmonitor?channel=1&subtype=0"
        );
        // Sem porta e sem credencial: só o host muda.
        assert_eq!(
            rede::trocar_host("rtsp://10.0.0.5/live", "10.0.0.9").as_deref(),
            Some("rtsp://10.0.0.9/live")
        );
        // Senha com ':' e '@' no meio continua inteira.
        assert_eq!(
            rede::trocar_host("rtsp://u:a@b:c@10.0.0.5:554/s", "10.0.0.6").as_deref(),
            Some("rtsp://u:a@b:c@10.0.0.6:554/s")
        );
    }

    #[test]
    fn antirrepique_uma_correcao_a_cada_10_min() {
        let r = |q, cam, res: &str| RegistroAuto {
            quando: q,
            camera_id: cam,
            nome: String::new(),
            de: String::new(),
            para: String::new(),
            resultado: res.into(),
            motivo: String::new(),
        };
        let log = vec![r(1000, 1, "corrigido"), r(1000, 2, "falhou")];
        assert!(!pode_corrigir(&log, 1, 1000 + 599));
        assert!(pode_corrigir(&log, 1, 1000 + 600));
        // Falha não conta como correção; outra câmera não interfere.
        assert!(pode_corrigir(&log, 2, 1001));
        assert!(pode_corrigir(&log, 3, 1001));
        assert!(pode_corrigir(&[], 1, 0));
    }

    #[tokio::test]
    async fn oscilacao_desliga_a_autocorrecao() {
        let (s, id, m) = cenario(true);
        let amb = rede_ok();
        assert!(matches!(
            tentar(&amb, &s, &m, false, 1000).await,
            Desfecho::Corrigida(_)
        ));
        // Volta para o IP antigo 5 min depois: oscilou.
        let volta = MudancaIp {
            de: m.para.clone(),
            para: m.de.clone(),
            quando: 1300,
            ..m.clone()
        };
        let d = tentar(&amb, &s, &volta, false, 1300).await;
        match d {
            Desfecho::Manual(Some(r)) => assert_eq!(r.resultado, "oscilou"),
            outro => panic!("esperava manual por oscilação: {outro:?}"),
        }
        let cam = s.camera(id).unwrap().unwrap();
        assert!(!cam.auto_ip, "autocorreção tem de desligar");
        assert!(cam.url.contains("@192.168.1.120:"), "não trocou de novo");
        assert!(amb.avisos.lock().unwrap()[1].contains("trocando de IP sem parar"));
        // Desligada, a próxima mudança vai direto para o fluxo com confirmação.
        assert_eq!(
            tentar(&amb, &s, &volta, false, 5000).await,
            Desfecho::Manual(None)
        );
    }

    #[tokio::test]
    async fn ligada_corrige_e_grava_evento() {
        let (s, id, m) = cenario(true);
        let amb = rede_ok();
        let d = tentar(&amb, &s, &m, false, 1000).await;
        assert!(matches!(d, Desfecho::Corrigida(ref r) if r.resultado == "corrigido"));
        let cam = s.camera(id).unwrap().unwrap();
        assert_eq!(
            cam.url,
            "rtsp://admin:S3nh@:%23!@192.168.1.120:8554/cam/realmonitor?channel=1&subtype=0"
        );
        assert_eq!(
            eventos(&s),
            vec!["IP corrigido sozinho: 192.168.1.108 → 192.168.1.120"]
        );
        assert_eq!(*amb.reinicios.lock().unwrap(), vec![cam.url.clone()]);
        assert_eq!(
            amb.avisos.lock().unwrap().as_slice(),
            ["Câmera Portão mudou de IP e foi corrigida sozinha (192.168.1.120).".to_string()]
        );
        let log = ler_log(&s);
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].de, "192.168.1.108");
        assert_eq!(avisos_recentes(&log, 1000 + 60).len(), 1);
        assert!(avisos_recentes(&log, 1000 + AVISO_S).is_empty());
    }

    #[tokio::test]
    async fn desligada_so_avisa() {
        let (s, id, m) = cenario(false);
        let amb = rede_ok();
        assert_eq!(
            tentar(&amb, &s, &m, false, 1000).await,
            Desfecho::Manual(None)
        );
        assert_eq!(s.camera(id).unwrap().unwrap().url, URL);
        assert!(amb.reinicios.lock().unwrap().is_empty());
        assert!(amb.avisos.lock().unwrap().is_empty());
        assert!(ler_log(&s).is_empty());
    }

    #[tokio::test]
    async fn sem_mac_aprendido_nao_corrige() {
        let (s, id, m) = cenario(true);
        s.definir_mac_camera(id, "").unwrap();
        let amb = rede_ok();
        assert_eq!(
            tentar(&amb, &s, &m, false, 1000).await,
            Desfecho::Manual(None)
        );
        assert_eq!(s.camera(id).unwrap().unwrap().url, URL);
    }

    #[tokio::test]
    async fn ip_novo_sem_rtsp_nao_troca() {
        let (s, id, m) = cenario(true);
        let amb = RedeFalsa {
            portas: vec![],
            ..rede_ok()
        };
        let d = tentar(&amb, &s, &m, false, 1000).await;
        assert!(matches!(d, Desfecho::Manual(Some(ref r)) if r.resultado == "falhou"));
        assert_eq!(s.camera(id).unwrap().unwrap().url, URL);
        assert!(amb.reinicios.lock().unwrap().is_empty());
        assert!(eventos(&s)[0].contains("não responde na porta 8554"));
        assert_eq!(amb.avisos.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn mac_diferente_no_ip_novo_nao_troca() {
        let (s, id, m) = cenario(true);
        let amb = RedeFalsa {
            arp: [("192.168.1.120".to_string(), "3C:EF:8C:00:00:09".to_string())].into(),
            ..rede_ok()
        };
        let d = tentar(&amb, &s, &m, false, 1000).await;
        assert!(matches!(d, Desfecho::Manual(Some(_))));
        assert_eq!(s.camera(id).unwrap().unwrap().url, URL);
    }

    #[tokio::test]
    async fn imagem_nao_volta_desfaz() {
        let (s, id, m) = cenario(true);
        let amb = RedeFalsa {
            imagem: false,
            ..rede_ok()
        };
        let d = tentar(&amb, &s, &m, false, 1000).await;
        assert!(matches!(d, Desfecho::Manual(Some(ref r)) if r.resultado == "falhou"));
        assert_eq!(s.camera(id).unwrap().unwrap().url, URL, "desfez");
        let r = amb.reinicios.lock().unwrap().clone();
        assert_eq!(r.len(), 2);
        assert_eq!(r[1], URL, "voltou o vídeo para o endereço antigo");
        assert!(eventos(&s)[0].contains("a imagem não voltou em 15 s"));
        // Falha não conta no antirrepique.
        assert!(pode_corrigir(&ler_log(&s), id, 1001));
    }

    #[tokio::test]
    async fn conflito_nao_corrige() {
        let (s, id, m) = cenario(true);
        let amb = rede_ok();
        let d = tentar(&amb, &s, &m, true, 1000).await;
        assert!(matches!(d, Desfecho::Manual(Some(ref r)) if r.resultado == "conflito"));
        assert_eq!(s.camera(id).unwrap().unwrap().url, URL);
        assert!(amb.reinicios.lock().unwrap().is_empty());
        assert!(eventos(&s)[0].contains("conflito de IP"));
    }
}
