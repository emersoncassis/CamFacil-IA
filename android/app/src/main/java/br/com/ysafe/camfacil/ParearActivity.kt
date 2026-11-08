package br.com.ysafe.camfacil

import android.app.Activity
import android.content.Intent
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Bundle
import android.text.InputType
import android.view.inputmethod.EditorInfo
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/** Pareamento: QR code da dashboard, endereço digitado ou o painel do site. */
class ParearActivity : Activity() {
    private lateinit var campo: EditText
    private lateinit var aviso: TextView
    private val d by lazy { resources.displayMetrics.density }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val col = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(px(20), px(24), px(20), px(24))
        }
        col.addView(texto(getString(R.string.app_nome), 22f, R.color.texto))
        col.addView(texto(getString(R.string.parear_lede), 14f, R.color.texto_3).apply { setPadding(0, px(4), 0, px(20)) })

        col.addView(Ui.botao(this, getString(R.string.parear_qr), primario = true) {
            startActivityForResult(Intent(this, QrActivity::class.java), 1)
        })
        col.addView(texto(getString(R.string.parear_qr_dica), 12f, R.color.texto_3).apply { setPadding(0, px(6), 0, px(20)) })

        col.addView(texto(getString(R.string.parear_ou), 13f, R.color.texto_2))
        campo = EditText(this).apply {
            hint = getString(R.string.parear_campo)
            setHintTextColor(getColor(R.color.texto_3))
            setTextColor(getColor(R.color.texto))
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
            imeOptions = EditorInfo.IME_ACTION_GO
            isSingleLine = true
            setPadding(px(12), px(12), px(12), px(12))
            background = GradientDrawable().apply {
                cornerRadius = 6 * d
                setColor(getColor(R.color.campo))
                setStroke(px(1), getColor(R.color.linha_forte))
            }
            setText(Config.endereco(this@ParearActivity) ?: "")
            setOnEditorActionListener { _, _, _ -> conectar(text.toString()); true }
        }
        col.addView(campo, LinearLayout.LayoutParams(-1, -2).apply { topMargin = px(8) })
        col.addView(Ui.botao(this, getString(R.string.parear_conectar)) { conectar(campo.text.toString()) })
        aviso = texto("", 13f, R.color.texto_2).apply { setPadding(0, px(10), 0, 0) }
        col.addView(aviso)

        // Tailscale: o app não embute a VPN; aponta para o app oficial.
        val temTs = try {
            packageManager.getPackageInfo(TAILSCALE, 0); true
        } catch (_: Exception) {
            false
        }
        col.addView(texto(getString(if (temTs) R.string.parear_tailscale_ok else R.string.parear_tailscale_falta), 12f, R.color.texto_3)
            .apply { setPadding(0, px(28), 0, 0) })
        col.addView(Ui.botao(this, getString(if (temTs) R.string.parear_tailscale_abrir else R.string.parear_tailscale_instalar)) {
            val i = if (temTs) packageManager.getLaunchIntentForPackage(TAILSCALE)
            else Intent(Intent.ACTION_VIEW, Uri.parse("https://play.google.com/store/apps/details?id=$TAILSCALE"))
            try { startActivity(i) } catch (_: Exception) {}
        })

        col.addView(Ui.botao(this, getString(R.string.parear_nuvem)) {
            Config.salvar(this, Config.SITE, local = false)
            abrirPainel()
        }.apply { (layoutParams as LinearLayout.LayoutParams).topMargin = px(24) })

        setContentView(ScrollView(this).apply { addView(col); setBackgroundColor(getColor(R.color.preto)) })
    }

    @Deprecated("Activity sem AndroidX")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val lido = data?.getStringExtra(QrActivity.RESULTADO) ?: return
        if (resultCode == RESULT_OK) {
            campo.setText(lido)
            conectar(lido)
        }
    }

    private fun conectar(bruto: String) {
        val end = Config.normalizar(bruto)
        if (end == null) {
            aviso.setText(R.string.parear_invalido)
            return
        }
        aviso.setText(R.string.parear_testando)
        Thread {
            val saude = Config.get("$end/api/saude", 6000)
            val local = saude != null && Config.get("$end/api/estado", 6000) != null
            runOnUiThread {
                if (saude == null) {
                    aviso.text = getString(R.string.parear_falhou, end)
                } else {
                    Config.salvar(this, end, local)
                    abrirPainel()
                }
            }
        }.start()
    }

    private fun abrirPainel() {
        startActivity(Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP))
        finish()
    }

    private fun texto(t: String, sp: Float, cor: Int) = TextView(this).apply {
        text = t
        textSize = sp
        setTextColor(getColor(cor))
    }

    private fun px(dp: Int) = (dp * d).toInt()

    companion object {
        const val TAILSCALE = "com.tailscale.ipn"
    }
}
