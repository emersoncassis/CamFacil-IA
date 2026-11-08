package br.com.ysafe.camfacil

import android.content.Context
import android.graphics.drawable.GradientDrawable
import android.graphics.drawable.StateListDrawable
import android.view.View
import android.widget.Button
import android.widget.LinearLayout
import android.widget.TextView

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

    /**
     * Cartão de escolha (título + explicação), focável pelo controle remoto da
     * TV Box: borda verde só quando focado (verde só em detalhe).
     */
    fun cartao(c: Context, titulo: String, texto: String, onClick: () -> Unit): View {
        val d = c.resources.displayMetrics.density
        fun fundo(cor: Int) = GradientDrawable().apply {
            cornerRadius = 8 * d
            setColor(c.getColor(R.color.superficie))
            setStroke((if (cor == R.color.verde) 2 * d else d).toInt().coerceAtLeast(1), c.getColor(cor))
        }
        return LinearLayout(c).apply {
            orientation = LinearLayout.VERTICAL
            val p = (18 * d).toInt()
            setPadding(p, p, p, p)
            isFocusable = true
            isClickable = true
            background = StateListDrawable().apply {
                addState(intArrayOf(android.R.attr.state_focused), fundo(R.color.verde))
                addState(intArrayOf(android.R.attr.state_pressed), fundo(R.color.texto_3))
                addState(intArrayOf(), fundo(R.color.linha_forte))
            }
            addView(TextView(c).apply {
                text = titulo
                textSize = 18f
                setTextColor(c.getColor(R.color.texto))
            })
            addView(TextView(c).apply {
                text = texto
                textSize = 14f
                setTextColor(c.getColor(R.color.texto_3))
                setPadding(0, (6 * d).toInt(), 0, 0)
            })
            layoutParams = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT,
            ).apply { topMargin = (12 * d).toInt() }
            setOnClickListener { onClick() }
        }
    }

    fun texto(c: Context, t: CharSequence, sp: Float, cor: Int) = TextView(c).apply {
        text = t
        textSize = sp
        setTextColor(c.getColor(cor))
    }
}
