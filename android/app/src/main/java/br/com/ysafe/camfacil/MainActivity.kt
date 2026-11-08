package br.com.ysafe.camfacil

import android.Manifest
import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.PowerManager
import android.provider.Settings
import android.view.Gravity
import android.view.Menu
import android.view.MenuItem
import android.view.View
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast

/** Tela principal: o painel (dashboard local ou site) num WebView. */
class MainActivity : Activity() {
    private lateinit var web: WebView
    private lateinit var erro: View
    private lateinit var erroTexto: TextView
    private var base: String = ""

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val end = Config.endereco(this)
        if (end == null) {
            startActivity(Intent(this, ParearActivity::class.java))
            finish()
            return
        }
        base = end
        montar()
        if (savedInstanceState == null) web.loadUrl(base) else web.restoreState(savedInstanceState)
        if (Config.alertasLigados(this) && Config.local(this)) AlertasService.iniciar(this)
    }

    @SuppressLint("SetJavaScriptEnabled")
    private fun montar() {
        val raiz = FrameLayout(this)
        raiz.setBackgroundColor(getColor(R.color.preto))
        web = WebView(this).apply {
            setBackgroundColor(getColor(R.color.preto))
            settings.javaScriptEnabled = true
            settings.domStorageEnabled = true
            settings.mediaPlaybackRequiresUserGesture = false
            settings.userAgentString = settings.userAgentString + " CamFacilAndroid/" + versao()
            webViewClient = Cliente()
            setDownloadListener { url, _, _, _, _ -> abrirFora(url) }
        }
        raiz.addView(web)
        erro = montarErro()
        raiz.addView(erro)
        setContentView(raiz)
    }

    private fun montarErro(): View {
        val d = resources.displayMetrics.density
        val caixa = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setPadding((24 * d).toInt(), 0, (24 * d).toInt(), 0)
            setBackgroundColor(getColor(R.color.preto))
            visibility = View.GONE
        }
        caixa.addView(TextView(this).apply {
            setText(R.string.erro_titulo)
            textSize = 20f
            setTextColor(getColor(R.color.texto))
        })
        erroTexto = TextView(this).apply {
            textSize = 14f
            setTextColor(getColor(R.color.texto_3))
            setPadding(0, (12 * d).toInt(), 0, (24 * d).toInt())
        }
        caixa.addView(erroTexto)
        caixa.addView(Ui.botao(this, getString(R.string.erro_tentar), primario = true) {
            erro.visibility = View.GONE
            web.reload()
        })
        caixa.addView(Ui.botao(this, getString(R.string.erro_parear)) { parear() })
        return caixa
    }

    private inner class Cliente : WebViewClient() {
        override fun shouldOverrideUrlLoading(view: WebView, req: WebResourceRequest): Boolean {
            val u = req.url
            val mesmo = u.scheme + "://" + u.authority == Uri.parse(base).let { it.scheme + "://" + it.authority }
            if (mesmo) return false
            abrirFora(u.toString())
            return true
        }

        override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
            erro.visibility = View.GONE
        }

        override fun onReceivedError(view: WebView, req: WebResourceRequest, e: WebResourceError) {
            if (!req.isForMainFrame) return
            erroTexto.text = getString(R.string.erro_texto, base)
            erro.visibility = View.VISIBLE
        }
    }

    private fun abrirFora(url: String) {
        try {
            startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
        } catch (_: Exception) {
        }
    }

    private fun parear() {
        startActivity(Intent(this, ParearActivity::class.java))
    }

    private fun versao(): String = try {
        packageManager.getPackageInfo(packageName, 0).versionName ?: ""
    } catch (_: Exception) {
        ""
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        val end = Config.endereco(this) ?: return
        if (end != base) {
            base = end
            web.loadUrl(base)
        }
    }

    override fun onCreateOptionsMenu(menu: Menu): Boolean {
        val local = Config.local(this)
        if (local) {
            menu.add(0, 1, 0, R.string.menu_cameras)
            menu.add(0, 2, 1, if (Config.alertasLigados(this)) R.string.menu_alertas_desligar else R.string.menu_alertas_ligar)
            menu.add(0, 6, 2, R.string.menu_bateria)
        }
        menu.add(0, 3, 3, R.string.menu_recarregar)
        menu.add(0, 4, 4, R.string.menu_parear)
        if (local) menu.add(0, 5, 5, R.string.menu_site)
        return true
    }

    override fun onPrepareOptionsMenu(menu: Menu): Boolean {
        menu.clear()
        return onCreateOptionsMenu(menu)
    }

    override fun onOptionsItemSelected(item: MenuItem): Boolean {
        when (item.itemId) {
            1 -> startActivity(Intent(this, CamerasActivity::class.java))
            2 -> alternarAlertas()
            3 -> web.reload()
            4 -> parear()
            5 -> abrirFora(Config.SITE)
            6 -> pedirBateria()
            else -> return super.onOptionsItemSelected(item)
        }
        return true
    }

    private fun alternarAlertas() {
        if (!Config.local(this)) {
            Toast.makeText(this, R.string.alertas_so_local, Toast.LENGTH_LONG).show()
            return
        }
        if (Config.alertasLigados(this)) {
            Config.ligarAlertas(this, false)
            AlertasService.parar(this)
            Toast.makeText(this, R.string.alertas_desligados, Toast.LENGTH_SHORT).show()
        } else {
            if (Build.VERSION.SDK_INT >= 33 &&
                checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
            ) {
                requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 10)
            }
            Config.ligarAlertas(this, true)
            AlertasService.iniciar(this)
            Toast.makeText(this, R.string.alertas_ligados, Toast.LENGTH_LONG).show()
        }
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

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        if (::web.isInitialized) web.saveState(outState)
    }

    @Deprecated("Activity sem AndroidX: o voltar clássico basta até a API 35.")
    override fun onBackPressed() {
        if (::web.isInitialized && web.canGoBack()) web.goBack() else super.onBackPressed()
    }

    override fun onDestroy() {
        if (::web.isInitialized) web.destroy()
        super.onDestroy()
    }
}
