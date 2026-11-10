package br.com.ysafe.camfacil

import android.content.Context
import android.content.SharedPreferences
import java.net.HttpURLConnection
import java.net.URL

/** Preferências do app e acesso HTTP mínimo ao programa local (sem bibliotecas). */
object Config {
    const val SITE = "https://cfia.ysafe.com.br"
    const val PORTA_PADRAO = 7070
    const val MODO_SERVIDOR = "servidor"
    const val MODO_REMOTO = "remoto"

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

    /** Modo escolhido na tela inicial: [MODO_SERVIDOR], [MODO_REMOTO] ou null (ainda não escolheu). */
    fun modo(c: Context): String? = prefs(c).getString("modo", null)
        // Quem já usava o app (só visualizador) e atualizou continua no modo remoto.
        ?: if (endereco(c) != null) MODO_REMOTO else null
    fun definirModo(c: Context, m: String?) = prefs(c).edit().putString("modo", m).apply()
    fun servidor(c: Context): Boolean = modo(c) == MODO_SERVIDOR

    /** O usuário quer o servidor deste aparelho ligado (religa no boot/atualização). */
    fun servidorLigado(c: Context): Boolean = prefs(c).getBoolean("servidor_ligado", false)
    fun ligarServidor(c: Context, v: Boolean) = prefs(c).edit().putBoolean("servidor_ligado", v).apply()

    fun iniciarNoBoot(c: Context): Boolean = prefs(c).getBoolean("servidor_boot", true)
    fun definirIniciarNoBoot(c: Context, v: Boolean) = prefs(c).edit().putBoolean("servidor_boot", v).apply()

    /** Aceitar acesso pela rede Wi-Fi da loja (`--lan`); sem isso, só este aparelho e o Tailscale. */
    fun aceitarRede(c: Context): Boolean = prefs(c).getBoolean("servidor_lan", true)
    fun definirAceitarRede(c: Context, v: Boolean) = prefs(c).edit().putBoolean("servidor_lan", v).apply()

    /** Porta em que o servidor deste aparelho respondeu da última vez. */
    fun portaServidor(c: Context): Int = prefs(c).getInt("servidor_porta", PORTA_PADRAO)
    fun salvarPortaServidor(c: Context, p: Int) = prefs(c).edit().putInt("servidor_porta", p).apply()

    fun enderecoLocal(c: Context): String = "http://127.0.0.1:${portaServidor(c)}"

    fun esquecerEndereco(c: Context) = prefs(c).edit().remove("endereco").remove("local").apply()

    /** O endereço salvo é o servidor deste próprio aparelho? */
    fun enderecoEhLocal(c: Context): Boolean = endereco(c)?.startsWith("http://127.0.0.1:") == true

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

    /**
     * Cookie do aparelho liberado (ADR 0017): a WebView o recebe ao entrar com o
     * PIN ou ao ser autorizada na máquina; as chamadas em segundo plano
     * (alertas, câmeras) mandam o mesmo cookie.
     */
    private fun acesso(c: HttpURLConnection, url: String) {
        val cookie = try { android.webkit.CookieManager.getInstance().getCookie(url) } catch (_: Throwable) { null }
        if (!cookie.isNullOrBlank()) c.setRequestProperty("Cookie", cookie)
    }

    /** Código HTTP de um GET (ou -1 sem resposta). 401 = programa no ar pedindo PIN. */
    fun codigo(url: String, timeoutMs: Int = 5000): Int = try {
        val c = URL(url).openConnection() as HttpURLConnection
        c.connectTimeout = timeoutMs
        c.readTimeout = timeoutMs
        acesso(c, url)
        try { c.responseCode } finally { c.disconnect() }
    } catch (_: Exception) {
        -1
    }

    /** GET simples; devolve o corpo ou null. Roda fora da thread principal. */
    fun get(url: String, timeoutMs: Int = 5000): String? = try {
        val c = URL(url).openConnection() as HttpURLConnection
        c.connectTimeout = timeoutMs
        c.readTimeout = timeoutMs
        c.setRequestProperty("Accept", "application/json")
        acesso(c, url)
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
        acesso(c, url)
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
