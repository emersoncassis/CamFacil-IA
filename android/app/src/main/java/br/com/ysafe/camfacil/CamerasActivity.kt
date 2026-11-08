package br.com.ysafe.camfacil

import android.annotation.SuppressLint
import android.app.Activity
import android.os.Bundle
import android.os.SystemClock
import android.webkit.JavascriptInterface
import android.webkit.WebView
import org.json.JSONObject

/**
 * Câmeras ao vivo (stream do programa local) com troca automática para foto
 * quando a conexão fica fraca. A tela é HTML em assets/cameras.html; a ponte
 * faz as requisições (evita CORS entre file:// e o programa).
 */
class CamerasActivity : Activity() {
    private lateinit var web: WebView

    @SuppressLint("SetJavaScriptEnabled", "AddJavascriptInterface")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val base = Config.endereco(this) ?: return finish()
        web = WebView(this).apply {
            setBackgroundColor(getColor(R.color.preto))
            settings.javaScriptEnabled = true
            settings.allowFileAccess = false
            addJavascriptInterface(Ponte(this, base), "CF")
            loadUrl("file:///android_asset/cameras.html")
        }
        setContentView(web)
    }

    override fun onPause() {
        super.onPause()
        web.evaluateJavascript("window.pausar&&pausar()", null)
    }

    override fun onResume() {
        super.onResume()
        web.evaluateJavascript("window.retomar&&retomar()", null)
    }

    override fun onDestroy() {
        web.destroy()
        super.onDestroy()
    }
}

/** Ponte JS → Kotlin. Respostas voltam por `CF_resposta(id, json)`. */
class Ponte(private val web: WebView, private val base: String) {
    @JavascriptInterface
    fun base(): String = base

    /** GET em `base + caminho`; mede o tempo e os bytes para avaliar a conexão. */
    @JavascriptInterface
    fun pedir(id: Int, caminho: String) {
        if (!caminho.startsWith("/api/")) return
        Thread {
            val t0 = SystemClock.elapsedRealtime()
            val corpo = Config.getBytes(base + caminho, 4000)
            val ms = SystemClock.elapsedRealtime() - t0
            val r = JSONObject()
                .put("ok", corpo != null)
                .put("ms", ms)
                .put("bytes", corpo?.size ?: 0)
            if (corpo != null && !caminho.endsWith(".jpg")) r.put("corpo", String(corpo))
            val js = "CF_resposta($id, ${JSONObject.quote(r.toString())})"
            web.post { web.evaluateJavascript(js, null) }
        }.start()
    }
}
