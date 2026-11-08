//! Textos de interface do lado do servidor (diagrama, relatório PDF), lidos
//! do mesmo `web/i18n/pt-BR.json` embutido que a dashboard usa (regra 8).

use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

fn traducoes() -> &'static HashMap<String, String> {
    static T: OnceLock<HashMap<String, String>> = OnceLock::new();
    T.get_or_init(|| {
        crate::assets::find("/i18n/pt-BR.json")
            .and_then(|a| serde_json::from_slice::<serde_json::Map<String, Value>>(a.body).ok())
            .map(|m| {
                // Só as chaves de texto (há listas, como `isencao.itens`).
                m.into_iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_string())))
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Texto da chave (ou a própria chave, se faltar).
pub fn t(k: &str) -> String {
    traducoes().get(k).cloned().unwrap_or_else(|| k.to_string())
}

/// Texto com `{nome}` trocados pelos valores.
pub fn tf(k: &str, params: &[(&str, &str)]) -> String {
    let mut s = t(k);
    for (n, v) in params {
        s = s.replace(&format!("{{{n}}}"), v);
    }
    s
}

/// A chave existe no arquivo de tradução?
#[cfg(test)]
pub fn existe(k: &str) -> bool {
    traducoes().contains_key(k)
}
