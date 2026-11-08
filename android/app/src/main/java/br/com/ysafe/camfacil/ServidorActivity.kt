package br.com.ysafe.camfacil

import android.annotation.SuppressLint
import android.app.Activity
import android.app.AlertDialog
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.widget.ImageView
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.qrcode.QRCodeWriter
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
    private lateinit var qr: ImageView
    private lateinit var redeDica: TextView
    private lateinit var liberar: Button
    private var qrTexto: String? = "-"
    /** Abre o painel sozinho quando o servidor ficar pronto (vindo da tela inicial). */
    private var abrirAoFicarPronto = false
    private var tailscaleIp: String? = null
    private var ultimaConsultaTs = 0L

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        abrirAoFicarPronto = savedInstanceState == null && intent.getBooleanExtra(ABRIR_PAINEL, false)
        // Tela única, sem rolagem (regra 9): topo, dois cartões e a barra de ações.
        val raiz = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(px(20), px(20), px(20), px(16))
            setBackgroundColor(getColor(R.color.preto))
        }
        raiz.addView(Ui.texto(this, getString(R.string.servidor_titulo), 22f, R.color.texto))
        situacao = Ui.texto(this, "", 14f, R.color.texto_2).apply { setPadding(0, px(4), 0, px(4)) }
        raiz.addView(situacao)
        log = TextView(this).apply {
            textSize = 11f
            typeface = Typeface.MONOSPACE
            maxLines = 3
            setTextColor(getColor(R.color.texto_3))
            setTextIsSelectable(true)
        }
        raiz.addView(log)

        val deitado = resources.configuration.orientation == Configuration.ORIENTATION_LANDSCAPE
        val corpo = LinearLayout(this).apply {
            orientation = if (deitado) LinearLayout.HORIZONTAL else LinearLayout.VERTICAL
        }
        raiz.addView(corpo, LinearLayout.LayoutParams(-1, 0, 1f).apply { topMargin = px(12) })

        // Cartão 1: rede da loja, com o endereço grande e o QR para outro celular ler.
        val cRede = cartao()
        cRede.addView(secao(R.string.servidor_rede_titulo))
        rede = Ui.texto(this, "", 22f, R.color.verde).apply {
            typeface = Typeface.MONOSPACE
            setTextIsSelectable(true)
            setPadding(0, px(6), 0, px(6))
        }
        cRede.addView(rede)
        qr = ImageView(this).apply { adjustViewBounds = true; scaleType = ImageView.ScaleType.FIT_START }
        cRede.addView(qr, LinearLayout.LayoutParams(-2, 0, 1f).apply { bottomMargin = px(6) })
        redeDica = Ui.texto(this, "", 13f, R.color.texto_3)
        cRede.addView(redeDica)
        liberar = Ui.botao(this, getString(R.string.servidor_rede_liberar)) {
            Config.definirAceitarRede(this, true)
            if (Config.servidorLigado(this)) { ServidorService.parar(this); ServidorService.iniciar(this) }
            atualizar()
        }
        cRede.addView(liberar)
        corpo.addView(cRede, peso(deitado, primeiro = true))

        // Cartão 2: de longe, pelo Tailscale.
        val cFora = cartao()
        cFora.addView(secao(R.string.servidor_fora_titulo))
        fora = Ui.texto(this, "", 15f, R.color.texto_2).apply { setTextIsSelectable(true); setPadding(0, px(6), 0, px(10)) }
        cFora.addView(fora, LinearLayout.LayoutParams(-1, 0, 1f))
        tsBotao = Ui.botao(this, "", primario = false) { abrirTailscale() }
        cFora.addView(tsBotao)
        corpo.addView(cFora, peso(deitado, primeiro = false))

        // Barra de ações.
        val acoes = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; setPadding(0, px(12), 0, 0) }
        abrir = Ui.botao(this, getString(R.string.servidor_abrir_painel), primario = true) { abrirPainel() }
        ligarDesligar = Ui.botao(this, "") { alternar() }
        val ajustes = Ui.botao(this, getString(R.string.servidor_ajustes)) { mostrarAjustes() }
        for ((i, v) in listOf(abrir, ligarDesligar, ajustes).withIndex()) {
            acoes.addView(v, LinearLayout.LayoutParams(0, -2, if (i == 0) 1.4f else 1f).apply { if (i > 0) marginStart = px(8) })
        }
        raiz.addView(acoes)
        setContentView(raiz)
        abrir.requestFocus()

        // Abriu a tela e o servidor deveria estar ligado (ex.: o sistema o fechou): liga de novo.
        if (Config.servidorLigado(this) && !ServidorService.estado.rodando) ServidorService.iniciar(this)
    }

    private fun cartao() = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(px(16), px(12), px(16), px(14))
        background = GradientDrawable().apply {
            setColor(getColor(R.color.superficie))
            setStroke(px(1), getColor(R.color.linha))
            cornerRadius = 12 * d
        }
    }

    private fun peso(deitado: Boolean, primeiro: Boolean) =
        (if (deitado) LinearLayout.LayoutParams(0, -1, 1f) else LinearLayout.LayoutParams(-1, 0, if (primeiro) 1.3f else 1f)).apply {
            if (!primeiro) { if (deitado) marginStart = px(12) else topMargin = px(12) }
        }

    /** Ajustes num diálogo, para a tela principal caber sem rolar. */
    private fun mostrarAjustes() {
        val col = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(px(20), px(8), px(20), 0) }
        col.addView(chave(R.string.servidor_boot, Config.iniciarNoBoot(this)) { v -> Config.definirIniciarNoBoot(this, v) })
        col.addView(chave(R.string.servidor_lan, Config.aceitarRede(this)) { v ->
            Config.definirAceitarRede(this, v)
            // A escuta muda só ao religar o núcleo.
            if (Config.servidorLigado(this)) { ServidorService.parar(this); ServidorService.iniciar(this) }
        })
        col.addView(Ui.botao(this, getString(R.string.menu_bateria)) { pedirBateria() })
        col.addView(Ui.botao(this, getString(R.string.menu_trocar_modo)) { InicioActivity.trocarModo(this) })
        AlertDialog.Builder(this, android.R.style.Theme_DeviceDefault_Dialog_Alert)
            .setTitle(R.string.servidor_ajustes)
            .setView(col)
            .setPositiveButton(R.string.ok) { _, _ -> atualizar() }
            .show()
    }

    /** QR do endereço da rede, para outro celular abrir o painel na hora. */
    private fun desenharQr(texto: String?) {
        if (texto == qrTexto) return
        qrTexto = texto
        if (texto == null) { qr.setImageDrawable(null); return }
        try {
            val lado = 400
            val m = QRCodeWriter().encode(texto, BarcodeFormat.QR_CODE, lado, lado, mapOf(EncodeHintType.MARGIN to 2))
            val bmp = Bitmap.createBitmap(lado, lado, Bitmap.Config.RGB_565)
            for (y in 0 until lado) for (x in 0 until lado) bmp.setPixel(x, y, if (m[x, y]) Color.BLACK else Color.WHITE)
            qr.setImageBitmap(bmp)
        } catch (_: Exception) {
            qr.setImageDrawable(null)
        }
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
        log.text = if (e.erro != null && e.log.isNotBlank()) e.log.lines().takeLast(3).joinToString("\n") else ""
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
        val lan = Config.aceitarRede(this)
        val ipLan = ipDaRede()
        val endLan = if (lan && ipLan != null) "http://$ipLan:$porta" else null
        rede.text = when {
            !lan -> getString(R.string.servidor_rede_desligada)
            ipLan == null -> getString(R.string.servidor_rede_sem_wifi)
            else -> endLan
        }
        rede.setTextColor(getColor(if (endLan != null) R.color.verde else R.color.texto_2))
        redeDica.text = getString(if (endLan != null) R.string.servidor_rede_dica else R.string.servidor_rede_so_aqui)
        liberar.visibility = if (lan) View.GONE else View.VISIBLE
        desenharQr(if (e.pronto && ligado) endLan else null)

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
        setPadding(0, 0, 0, px(4))
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
