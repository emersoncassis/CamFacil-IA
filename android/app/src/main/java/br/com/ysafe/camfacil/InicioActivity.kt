package br.com.ysafe.camfacil

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Toast

/**
 * Tela inicial: o mesmo APK é servidor (câmeras ligadas aqui, como o programa
 * do PC) ou visualizador de um servidor à distância. Depois da escolha, abre
 * direto no modo salvo; "Trocar modo" no menu volta aqui.
 */
class InicioActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (!intent.getBooleanExtra(ESCOLHER, false) && seguirModoSalvo()) {
            finish()
            return
        }
        val d = resources.displayMetrics.density
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding((20 * d).toInt(), (32 * d).toInt(), (20 * d).toInt(), (24 * d).toInt())
        }
        col.addView(Ui.texto(this, getString(R.string.app_nome), 24f, R.color.texto))
        col.addView(Ui.texto(this, getString(R.string.inicio_lede), 14f, R.color.texto_3).apply {
            setPadding(0, (6 * d).toInt(), 0, (12 * d).toInt())
        })
        val servidor = Ui.cartao(this, getString(R.string.inicio_servidor), getString(R.string.inicio_servidor_texto)) {
            escolherServidor()
        }
        col.addView(servidor)
        col.addView(Ui.cartao(this, getString(R.string.inicio_remoto), getString(R.string.inicio_remoto_texto)) {
            escolherRemoto()
        })
        setContentView(ScrollView(this).apply { addView(col); setBackgroundColor(getColor(R.color.preto)) })
        servidor.requestFocus()
    }

    /** Abre a tela do modo já escolhido. Devolve false se ainda não há escolha. */
    private fun seguirModoSalvo(): Boolean = when (Config.modo(this)) {
        Config.MODO_SERVIDOR -> {
            startActivity(Intent(this, ServidorActivity::class.java).putExtra(ServidorActivity.ABRIR_PAINEL, true))
            true
        }
        Config.MODO_REMOTO -> {
            abrirRemoto(this)
            true
        }
        else -> false
    }

    private fun escolherServidor() {
        Config.definirModo(this, Config.MODO_SERVIDOR)
        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 11)
        }
        ServidorService.iniciar(this)
        startActivity(Intent(this, ServidorActivity::class.java).putExtra(ServidorActivity.ABRIR_PAINEL, true))
        finish()
    }

    private fun escolherRemoto() {
        if (Config.servidorLigado(this)) {
            ServidorService.parar(this)
            Toast.makeText(this, R.string.inicio_servidor_desligado, Toast.LENGTH_LONG).show()
        }
        Config.definirModo(this, Config.MODO_REMOTO)
        if (Config.enderecoEhLocal(this)) Config.esquecerEndereco(this)
        abrirRemoto(this)
        finish()
    }

    companion object {
        const val ESCOLHER = "escolher"

        fun abrirRemoto(c: Context) {
            val alvo = if (Config.endereco(c) == null) ParearActivity::class.java else MainActivity::class.java
            c.startActivity(Intent(c, alvo))
        }

        fun trocarModo(c: Context) {
            c.startActivity(
                Intent(c, InicioActivity::class.java).putExtra(ESCOLHER, true)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK),
            )
        }
    }
}
