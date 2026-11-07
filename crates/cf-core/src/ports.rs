//! Escolha de porta sem conflito (ADR-0005).
//!
//! O CamFácil IA nunca assume uma porta fixa: a máquina do cliente costuma rodar
//! PDV, ERP, banco de dados e NVR. Antes de ocupar uma porta, verificamos se ela
//! está livre e evitamos portas conhecidas desses sistemas.

use std::net::{IpAddr, SocketAddr, TcpListener};

/// Porta preferida da dashboard.
pub const PREFERRED: u16 = 7070;
/// Faixa de reserva caso a preferida esteja ocupada.
pub const FALLBACK: std::ops::RangeInclusive<u16> = 7071..=7099;

/// Portas comuns de PDV, ERP, bancos de dados, NVR e serviços web que nunca usamos.
pub const BLOCKLIST: &[u16] = &[
    21, 22, 23, 25, 53, 80, 110, 135, 139, 143, 443, 445, 554, 1433, 1434, 1521, 1883, 3050, 3306,
    3389, 5000, 5432, 5900, 6379, 8000, 8001, 8008, 8080, 8081, 8443, 8554, 8888, 9000, 27017,
    34567, 37777, 37778,
];

/// Diz se a porta está na lista de portas que nunca usamos.
pub fn is_blocked(port: u16) -> bool {
    BLOCKLIST.contains(&port)
}

/// Tenta ocupar uma porta de verdade. Retorna o listener já aberto, para não haver
/// corrida entre "verificar" e "usar".
fn try_bind(ip: IpAddr, port: u16) -> Option<TcpListener> {
    if port != 0 && is_blocked(port) {
        return None;
    }
    TcpListener::bind(SocketAddr::new(ip, port)).ok()
}

/// Resultado da escolha de porta.
#[derive(Debug)]
pub struct Chosen {
    pub listener: TcpListener,
    pub port: u16,
    /// Verdadeiro quando não foi possível usar a porta salva nem a preferida.
    pub moved: bool,
}

/// Ordem: porta salva → 7070 → 7071..=7099 → porta livre escolhida pelo sistema.
pub fn choose(ip: IpAddr, saved: Option<u16>) -> std::io::Result<Chosen> {
    let mut order: Vec<u16> = Vec::with_capacity(32);
    if let Some(p) = saved {
        order.push(p);
    }
    order.push(PREFERRED);
    order.extend(FALLBACK);

    let first = order[0];
    for port in order {
        if let Some(listener) = try_bind(ip, port) {
            return Ok(Chosen {
                listener,
                port,
                moved: port != first,
            });
        }
    }
    // Último recurso: o sistema escolhe uma porta livre.
    let listener = TcpListener::bind(SocketAddr::new(ip, 0))?;
    let port = listener.local_addr()?.port();
    Ok(Chosen {
        listener,
        port,
        moved: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const LO: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    #[test]
    fn nunca_usa_portas_bloqueadas() {
        for p in [80, 443, 554, 3306, 5432, 8080, 37777] {
            assert!(is_blocked(p), "{p} deveria estar bloqueada");
        }
        assert!(!is_blocked(PREFERRED));
        for p in FALLBACK {
            assert!(!is_blocked(p));
        }
    }

    #[test]
    fn porta_salva_bloqueada_e_ignorada() {
        let c = choose(LO, Some(3306)).unwrap();
        assert_ne!(c.port, 3306);
    }

    #[test]
    fn pula_porta_ocupada() {
        // Ocupa uma porta e pede exatamente ela.
        let busy = TcpListener::bind((LO, 0)).unwrap();
        let busy_port = busy.local_addr().unwrap().port();
        let c = choose(LO, Some(busy_port)).unwrap();
        assert_ne!(c.port, busy_port);
        assert!(c.moved);
    }

    #[test]
    fn reaproveita_porta_salva_livre() {
        let tmp = TcpListener::bind((LO, 0)).unwrap();
        let free = tmp.local_addr().unwrap().port();
        drop(tmp);
        if is_blocked(free) {
            return;
        }
        let c = choose(LO, Some(free)).unwrap();
        assert_eq!(c.port, free);
        assert!(!c.moved);
    }
}
