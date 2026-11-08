package br.com.ysafe.camfacil

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import android.view.View
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Switch
import android.widget.TextView
import org.json.JSONObject
import java.net.Inet4Address
import java.net.NetworkInterface

/**
 * Tela do servidor deste aparelho: estado do núcleo, botão para o painel local
 * (http://127.0.0.1:porta) e como acessar de fora pelo Tailscale.
 */
class ServidorActivity : Activity() {
    private val d by lazy { resources.displayMetrics.density }
    private val loop = Handler(Looper.getMainLooper())
    private lateinit var situacao: TextView
    private lateinit var abrir: Button
    private lateinit var ligarDesligar: Button
    private lateinit var fora: TextView
    private lateinit var rede: TextView
    private lateinit var tsBotao: Button
    private lateinit var log: TextView
    /** Abre o painel sozinho quando o servidor ficar pronto (vindo da tela inicial). */
    private var abrirAoFicarPronto = false
    private var tailscaleIp: String? = null
    private var ultimaConsultaTs = 0L

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        abrirAoFicarPronto = savedInstanceState == null && intent.getBooleanExtra(ABRIR_PAINEL, false)
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(px(20), px(24), px(20), px(24))
        }
        col.addView(Ui.texto(this, getString(R.string.servidor_titulo), 22f, R.color.texto))
        situacao = Ui.texto(this, "", 14f, R.color.texto_2).apply { setPadding(0, px(6), 0, px(8)) }
        col.addView(situacao)
        abrir = Ui.botao(this, getString(R.string.servidor_abrir_painel), primario = true) { abrirPainel() }
        col.addView(abrir)

        col.addView(secao(R.string.servidor_fora_titulo))
        fora = Ui.texto(this, "", 14f, R.color.texto_2).apply { setTextIsSelectable(true) }
        col.addView(fora)
        tsBotao = Ui.botao(this, "") { abrirTailscale() }
        col.addView(tsBotao)
        rede = Ui.texto(this, "", 13f, R.color.texto_3).apply { setPadding(0, px(10), 0, 0); setTextIsSelectable(true) }
        col.addView(rede)

        col.addView(secao(R.string.servidor_ajustes))
        col.addView(chave(R.string.servidor_boot, Config.iniciarNoBoot(this)) { v ->
            Config.definirIniciarNoBoot(this, v)
        })
        col.addView(chave(R.string.servidor_lan, Config.aceitarRede(this)) { v ->
            Config.definirAceitarRede(this, v)
            // A escuta muda só ao religar o núcleo.
            if (Config.servidorLigado(this)) {
                ServidorService.parar(this)
                ServidorService.iniciar(this)
            }
        })
        col.addView(Ui.botao(this, getString(R.string.menu_bateria)) { pedirBateria() })
        ligarDesligar = Ui.botao(this, "") { alternar() }
        col.addView(ligarDesligar)
        col.addView(Ui.botao(this, getString(R.string.menu_trocar_modo)) { InicioActivity.trocarModo(this) })

        log = TextView(this).apply {
            textSize = 11f
            typeface = Typeface.MONOSPACE
            setTextColor(getColor(R.color.texto_3))
            setPadding(0, px(16), 0, 0)
            setTextIsSelectable(true)
        }
        col.addView(log)
        setContentView(ScrollView(this).apply { addView(col); setBackgroundColor(getColor(R.color.preto)) })
        abrir.requestFocus()

        // Abriu a tela e o servidor deveria estar ligado (ex.: o sistema o fechou): liga de novo.
        if (Config.servidorLigado(this) && !ServidorService.estado.rodando) ServidorService.iniciar(this)
    }

    override fun onResume() {
        super.onResume()
        loop.post(tique)
    }

    override fun onPause() {
        loop.removeCallbacks(tique)
        super.onPause()
    }

    private val tique = object : Runnable {
        override fun run() {
            atualizar()
            loop.postDelayed(this, 1000)
        }
    }

    private fun atualizar() {
        val e = ServidorService.estado
        val ligado = Config.servidorLigado(this)
        val porta = if (e.porta > 0) e.porta else Config.portaServidor(this)
        situacao.text = when {
            !ligado -> getString(R.string.servidor_parado)
            e.pronto -> getString(R.string.servidor_ligado_em, "127.0.0.1:$porta")
            e.erro != null -> getString(R.string.servidor_erro, e.erro)
            else -> getString(R.string.servidor_ligando)
        }
        situacao.setTextColor(getColor(if (e.pronto && ligado) R.color.verde else R.color.texto_2))
        abrir.isEnabled = e.pronto && ligado
        abrir.alpha = if (abrir.isEnabled) 1f else 0.5f
        ligarDesligar.text = getString(if (ligado) R.string.servidor_desligar else R.string.servidor_ligar)
        log.text = if (e.erro != null && e.log.isNotBlank()) e.log else ""
        log.visibility = if (log.text.isEmpty()) View.GONE else View.VISIBLE

        if (e.pronto && ligado && System.currentTimeMillis() - ultimaConsultaTs > 10_000) {
            ultimaConsultaTs = System.currentTimeMillis()
            Thread {
                val ip = try {
                    JSONObject(Config.get("http://127.0.0.1:$porta/api/tailscale", 3000) ?: "{}").optString("ip").takeIf { it.isNotBlank() && it != "null" }
                } catch (_: Exception) { null }
                runOnUiThread { tailscaleIp = ip }
            }.start()
        }
        val temTs = tailscaleInstalado()
        fora.text = when {
            tailscaleIp != null -> getString(R.string.servidor_fora_ok, "http://$tailscaleIp:$porta")
            temTs -> getString(R.string.servidor_fora_ligar)
            else -> getString(R.string.servidor_fora_instalar)
        }
        tsBotao.text = getString(if (temTs) R.string.parear_tailscale_abrir else R.string.parear_tailscale_instalar)
        rede.text = if (Config.aceitarRede(this)) {
            ipDaRede()?.let { getString(R.string.servidor_rede_ok, "http://$it:$porta") } ?: ""
        } else getString(R.string.servidor_rede_so_aqui)

        if (abrirAoFicarPronto && e.pronto && ligado) {
            abrirAoFicarPronto = false
            abrirPainel()
        }
    }

    private fun abrirPainel() {
        Config.salvar(this, Config.enderecoLocal(this), local = true)
        startActivity(Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP))
    }

    private fun alternar() {
        if (Config.servidorLigado(this)) ServidorService.parar(this) else ServidorService.iniciar(this)
        atualizar()
    }

    private fun tailscaleInstalado(): Boolean = try {
        packageManager.getPackageInfo(ParearActivity.TAILSCALE, 0); true
    } catch (_: Exception) {
        false
    }

    private fun abrirTailscale() {
        val i = if (tailscaleInstalado()) packageManager.getLaunchIntentForPackage(ParearActivity.TAILSCALE)
        else Intent(Intent.ACTION_VIEW, Uri.parse("https://play.google.com/store/apps/details?id=${ParearActivity.TAILSCALE}"))
        try { startActivity(i) } catch (_: Exception) {}
    }

    /** IPv4 da rede local (Wi-Fi/cabo), para quem abrir pela rede da loja. */
    private fun ipDaRede(): String? = try {
        NetworkInterface.getNetworkInterfaces().toList()
            .filter { it.isUp && !it.isLoopback && !it.name.startsWith("tun") }
            .flatMap { it.inetAddresses.toList() }
            .filterIsInstance<Inet4Address>()
            .firstOrNull { it.isSiteLocalAddress }?.hostAddress
    } catch (_: Exception) {
        null
    }

    @SuppressLint("BatteryLife")
    private fun pedirBateria() {
        val pm = getSystemService(PowerManager::class.java)
        if (pm.isIgnoringBatteryOptimizations(packageName)) return
        try {
            startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName")))
        } catch (_: Exception) {
            startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
        }
    }

    private fun secao(texto: Int) = Ui.texto(this, getString(texto), 13f, R.color.texto_3).apply {
        setPadding(0, px(28), 0, px(4))
        isAllCaps = true
        letterSpacing = 0.06f
    }

    @Suppress("DEPRECATION")
    private fun chave(texto: Int, valor: Boolean, mudou: (Boolean) -> Unit) = Switch(this).apply {
        text = getString(texto)
        textSize = 15f
        setTextColor(getColor(R.color.texto))
        isChecked = valor
        minHeight = px(48)
        setOnCheckedChangeListener { _, v -> mudou(v) }
    }

    private fun px(dp: Int) = (dp * d).toInt()

    companion object {
        const val ABRIR_PAINEL = "abrir_painel"
    }
}
