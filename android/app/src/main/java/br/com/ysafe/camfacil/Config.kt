package br.com.ysafe.camfacil

import android.content.Context
import android.content.SharedPreferences
import java.net.HttpURLConnection
import java.net.URL

/** Preferências do app e acesso HTTP mínimo ao programa local (sem bibliotecas). */
object Config {
    const val SITE = "https://cfia.ysafe.com.br"
    const val PORTA_PADRAO = 7070

    private fun prefs(c: Context): SharedPreferences =
        c.getSharedPreferences("camfacil", Context.MODE_PRIVATE)

    /** Endereço base pareado (ex.: http://100.64.1.2:7070), ou null. */
    fun endereco(c: Context): String? = prefs(c).getString("endereco", null)

    /** true quando o endereço é o programa local (tem /api/estado); false no site. */
    fun local(c: Context): Boolean = prefs(c).getBoolean("local", false)

    fun salvar(c: Context, endereco: String, local: Boolean) {
        prefs(c).edit().putString("endereco", endereco).putBoolean("local", local)
            .remove("ultimo_evento").apply()
    }

    fun alertasLigados(c: Context): Boolean = prefs(c).getBoolean("alertas", false)
    fun ligarAlertas(c: Context, v: Boolean) = prefs(c).edit().putBoolean("alertas", v).apply()

    fun ultimoEvento(c: Context): Long = prefs(c).getLong("ultimo_evento", -1)
    fun salvarUltimoEvento(c: Context, id: Long) = prefs(c).edit().putLong("ultimo_evento", id).apply()

    /**
     * Normaliza o que veio do QR ou foi digitado:
     * "192.168.0.10" → http://192.168.0.10:7070 · "cfia.ysafe.com.br" → https://cfia.ysafe.com.br.
     */
    fun normalizar(bruto: String): String? {
        var t = bruto.trim().trimEnd('/')
        if (t.isEmpty() || t.any { it.isWhitespace() }) return null
        if (!t.contains("://")) {
            val host = t.substringBefore('/').substringBefore(':')
            val rede = host.all { it.isDigit() || it == '.' } || !host.contains('.') ||
                host.endsWith(".local") || host.endsWith(".ts.net")
            t = if (rede) "http://$t" else "https://$t"
        }
        return try {
            val u = URL(t)
            if (u.protocol != "http" && u.protocol != "https") return null
            if (u.host.isNullOrEmpty()) return null
            val porta = when {
                u.port > 0 -> ":${u.port}"
                u.protocol == "http" -> ":$PORTA_PADRAO"
                else -> ""
            }
            "${u.protocol}://${u.host}$porta"
        } catch (_: Exception) {
            null
        }
    }

    /** GET simples; devolve o corpo ou null. Roda fora da thread principal. */
    fun get(url: String, timeoutMs: Int = 5000): String? = try {
        val c = URL(url).openConnection() as HttpURLConnection
        c.connectTimeout = timeoutMs
        c.readTimeout = timeoutMs
        c.setRequestProperty("Accept", "application/json")
        try {
            if (c.responseCode in 200..299) c.inputStream.bufferedReader().use { it.readText() } else null
        } finally {
            c.disconnect()
        }
    } catch (_: Exception) {
        null
    }

    fun getBytes(url: String, timeoutMs: Int = 5000, limite: Int = 2_000_000): ByteArray? = try {
        val c = URL(url).openConnection() as HttpURLConnection
        c.connectTimeout = timeoutMs
        c.readTimeout = timeoutMs
        try {
            if (c.responseCode in 200..299 && c.contentLength <= limite) {
                c.inputStream.use { it.readBytes() }.takeIf { it.size <= limite }
            } else null
        } finally {
            c.disconnect()
        }
    } catch (_: Exception) {
        null
    }
}
