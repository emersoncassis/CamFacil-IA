//! Raízes de confiança do HTTPS por sistema.
//!
//! No PC o reqwest usa o verificador da plataforma. No Android isso exige a
//! JVM (JNI), que o núcleo não tem: ele roda como processo à parte, lançado
//! pelo app (ADR 0010). Lá usamos as autoridades do próprio sistema, em
//! `/system/etc/security/cacerts` (e a cópia atualizável do APEX, no Android 14+).

/// Ajusta o construtor para este sistema. Fora do Android não muda nada.
pub fn raizes_do_sistema(b: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    #[cfg(target_os = "android")]
    {
        let certs = certificados_android();
        if !certs.is_empty() {
            return b.tls_certs_only(certs);
        }
    }
    b
}

#[cfg(target_os = "android")]
fn certificados_android() -> Vec<reqwest::Certificate> {
    for pasta in [
        "/apex/com.android.conscrypt/cacerts",
        "/system/etc/security/cacerts",
    ] {
        let Ok(dir) = std::fs::read_dir(pasta) else {
            continue;
        };
        let certs: Vec<_> = dir
            .flatten()
            .filter_map(|e| std::fs::read(e.path()).ok())
            .filter_map(|pem| reqwest::Certificate::from_pem(&pem).ok())
            .collect();
        if !certs.is_empty() {
            return certs;
        }
    }
    Vec::new()
}
