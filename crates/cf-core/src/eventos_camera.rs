//! Alertas que a **própria câmera ou gravador** manda (movimento, cruzou a
//! linha, invadiu a área…), lidos pela API de eventos do fabricante:
//!
//! - Intelbras/Dahua: `GET /cgi-bin/eventManager.cgi?action=attach&codes=[All]&heartbeat=5`,
//!   fluxo de linhas `Code=VideoMotion;action=Start;index=0`.
//! - Hikvision: `GET /ISAPI/Event/notification/alertStream`, fluxo multipart
//!   de XML com `<eventType>`, `<eventState>` e `<channelID>`.
//!
//! Aqui só o que é matemática de texto (caminho a pedir, ler as linhas,
//! traduzir o código). A conexão fica em `camfacil::alertas_camera`.

/// Um alerta recebido da câmera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventoCamera {
    /// Canal do gravador (1, 2, …); câmera IP avulsa é 1.
    pub canal: u32,
    /// Código do fabricante (`VideoMotion`, `VMD`…).
    pub codigo: String,
}

impl EventoCamera {
    /// Texto para o técnico.
    pub fn texto(&self) -> String {
        traduzir(&self.codigo)
    }
}

/// Caminho HTTP da escuta de eventos do fabricante (`intelbras` ou `hikvision`).
pub fn caminho(fabricante: &str) -> Option<&'static str> {
    match fabricante {
        "intelbras" => Some("/cgi-bin/eventManager.cgi?action=attach&codes=%5BAll%5D&heartbeat=5"),
        "hikvision" => Some("/ISAPI/Event/notification/alertStream"),
        _ => None,
    }
}

/// Códigos que são só ruído de sistema (não são alerta de cena).
fn ruido(codigo: &str) -> bool {
    matches!(
        codigo,
        "Heartbeat" | "videoloss" | "VideoLoss" | "NTPAdjustTime" | "TimeChange" | "TimeUpdate"
    )
}

/// Dahua/Intelbras: uma linha do fluxo. Só `action=Start` (e `Pulse`) é alerta.
pub fn ler_linha_dahua(linha: &str) -> Option<EventoCamera> {
    let linha = linha.trim();
    if !linha.starts_with("Code=") {
        return None;
    }
    let mut codigo = "";
    let mut acao = "";
    let mut indice: u32 = 0;
    for parte in linha.split(';') {
        let Some((k, v)) = parte.split_once('=') else {
            continue;
        };
        match k.trim() {
            "Code" => codigo = v.trim(),
            "action" => acao = v.trim(),
            "index" => indice = v.trim().parse().unwrap_or(0),
            _ => {}
        }
    }
    if codigo.is_empty() || ruido(codigo) || !(acao == "Start" || acao == "Pulse") {
        return None;
    }
    Some(EventoCamera {
        canal: indice + 1,
        codigo: codigo.to_string(),
    })
}

fn tag<'a>(xml: &'a str, nome: &str) -> Option<&'a str> {
    let ini = format!("<{nome}>");
    let fim = format!("</{nome}>");
    let a = xml.find(&ini)? + ini.len();
    let b = xml[a..].find(&fim)? + a;
    Some(xml[a..b].trim())
}

/// Hikvision: um bloco XML do fluxo. Só `eventState=active` conta.
pub fn ler_xml_hikvision(xml: &str) -> Option<EventoCamera> {
    let tipo = tag(xml, "eventType")?;
    let estado = tag(xml, "eventState").unwrap_or("active");
    if ruido(tipo) || !estado.eq_ignore_ascii_case("active") {
        return None;
    }
    let canal = tag(xml, "channelID")
        .or_else(|| tag(xml, "dynChannelID"))
        .and_then(|c| c.parse().ok())
        .unwrap_or(1);
    Some(EventoCamera {
        canal,
        codigo: tipo.to_string(),
    })
}

/// Nome em português do código do fabricante (o resto aparece como veio).
pub fn traduzir(codigo: &str) -> String {
    match codigo {
        "VideoMotion" | "VMD" => "movimento",
        "CrossLineDetection" | "linedetection" => "cruzou a linha",
        "CrossRegionDetection" | "fielddetection" | "regionEntrance" => "invadiu a área",
        "LeftDetection" | "unattendedBaggage" => "objeto deixado",
        "TakenAwayDetection" | "attendedBaggage" => "objeto retirado",
        "FaceDetection" | "facedetection" => "rosto detectado",
        "VideoBlind" | "tamperdetection" => "câmera tapada",
        "VideoAbnormalDetection" | "scenechangedetection" => "mudança de cena",
        "AlarmLocal" | "IO" => "entrada de alarme",
        "SmartMotionHuman" | "humanDetection" => "pessoa",
        "SmartMotionVehicle" | "vehicleDetection" => "veículo",
        outro => return outro.to_string(),
    }
    .to_string()
}

/// Separa o fluxo em blocos prontos para ler. Dahua: linhas inteiras.
/// Hikvision: de `<EventNotificationAlert` até `</EventNotificationAlert>`.
/// Devolve o que sobrou (parte incompleta) para somar ao próximo pedaço.
pub fn cortar_blocos(buf: &mut String, xml: bool) -> Vec<String> {
    let mut out = Vec::new();
    if xml {
        const FIM: &str = "</EventNotificationAlert>";
        while let Some(i) = buf.find(FIM) {
            let fim = i + FIM.len();
            let bloco: String = buf.drain(..fim).collect();
            out.push(bloco);
        }
        if buf.len() > 64 * 1024 {
            buf.clear();
        }
    } else {
        while let Some(i) = buf.find('\n') {
            let l: String = buf.drain(..=i).collect();
            out.push(l);
        }
        if buf.len() > 64 * 1024 {
            buf.clear();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dahua_so_start_conta_e_canal_comeca_em_1() {
        let e = ler_linha_dahua("Code=VideoMotion;action=Start;index=2\r\n").unwrap();
        assert_eq!((e.canal, e.codigo.as_str()), (3, "VideoMotion"));
        assert_eq!(e.texto(), "movimento");
        assert!(ler_linha_dahua("Code=VideoMotion;action=Stop;index=2").is_none());
        assert!(ler_linha_dahua("Code=Heartbeat;action=Pulse;index=0").is_none());
        assert!(ler_linha_dahua("--myboundary").is_none());
        assert!(ler_linha_dahua("Content-Length:30").is_none());
        let c = ler_linha_dahua("Code=CrossLineDetection;action=Start;index=0;data={\n").unwrap();
        assert_eq!(c.texto(), "cruzou a linha");
    }

    #[test]
    fn hikvision_so_ativo_e_ignora_videoloss() {
        let ativo = "<EventNotificationAlert><channelID>4</channelID><eventType>VMD</eventType><eventState>active</eventState></EventNotificationAlert>";
        let e = ler_xml_hikvision(ativo).unwrap();
        assert_eq!((e.canal, e.texto().as_str()), (4, "movimento"));
        let fim = ativo.replace("active", "inactive");
        assert!(ler_xml_hikvision(&fim).is_none());
        let bat = "<eventType>videoloss</eventType><eventState>inactive</eventState>";
        assert!(ler_xml_hikvision(bat).is_none());
    }

    #[test]
    fn corta_o_fluxo_em_blocos_e_guarda_a_sobra() {
        let mut b = String::from("Code=A;action=Start;index=0\nCode=B;action=St");
        let v = cortar_blocos(&mut b, false);
        assert_eq!(v.len(), 1);
        assert_eq!(b, "Code=B;action=St");
        let mut x = String::from(
            "--b\r\n<EventNotificationAlert><eventType>VMD</eventType></EventNotificationAlert>\r\n--b\r\n<EventNotificationAlert><even",
        );
        let v = cortar_blocos(&mut x, true);
        assert_eq!(v.len(), 1);
        assert!(x.contains("<even"));
    }

    #[test]
    fn caminho_por_fabricante() {
        assert!(caminho("intelbras").unwrap().contains("eventManager.cgi"));
        assert!(caminho("hikvision").unwrap().contains("alertStream"));
        assert!(caminho("outra").is_none());
    }
}
