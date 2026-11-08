package br.com.ysafe.camfacil

import android.content.Context
import android.graphics.drawable.GradientDrawable
import android.widget.Button
import android.widget.LinearLayout

/** Botões no visual do DESIGN.md: primário claro sobre preto, secundário com borda. */
object Ui {
    fun botao(c: Context, texto: String, primario: Boolean = false, onClick: () -> Unit): Button {
        val d = c.resources.displayMetrics.density
        return Button(c).apply {
            text = texto
            isAllCaps = false
            textSize = 15f
            stateListAnimator = null
            setTextColor(c.getColor(if (primario) R.color.preto else R.color.texto))
            background = GradientDrawable().apply {
                cornerRadius = 6 * d
                setColor(c.getColor(if (primario) R.color.texto else R.color.superficie))
                if (!primario) setStroke(d.toInt().coerceAtLeast(1), c.getColor(R.color.linha_forte))
            }
            minHeight = (48 * d).toInt()
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = (8 * d).toInt() }
            setOnClickListener { onClick() }
        }
    }
}
