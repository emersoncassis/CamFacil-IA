package br.com.ysafe.camfacil

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Build
import android.os.IBinder
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Vigia leve de alertas: a cada 10 s lê /api/eventos do programa local e
 * mostra uma notificação com o nome da regra (o "balão") e a foto da câmera,
 * sem nenhuma caixa da IA. Uma thread dormindo; sem WorkManager nem push.
 */
class AlertasService : Service() {
    @Volatile private var rodando = false
    private var fio: Thread? = null
    private val nomesRegra = HashMap<Long, String>()
    private val nomesCamera = HashMap<Long, String>()

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val base = Config.endereco(this)
        if (base == null || !Config.local(this) || !Config.alertasLigados(this)) {
            stopSelf()
            return START_NOT_STICKY
        }
        criarCanais()
        val n = vigia(getString(R.string.vigia_texto, base))
        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(ID_VIGIA, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(ID_VIGIA, n)
        }
        if (!rodando) {
            rodando = true
            fio = Thread({ laco(base) }, "alertas").apply { start() }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        rodando = false
        fio?.interrupt()
        super.onDestroy()
    }

    private fun laco(base: String) {
        var falhas = 0
        while (rodando) {
            val ok = verificar(base)
            if (ok != (falhas == 0)) {
                val texto = if (ok) getString(R.string.vigia_texto, base) else getString(R.string.vigia_sem_conexao, base)
                getSystemService(NotificationManager::class.java).notify(ID_VIGIA, vigia(texto))
            }
            falhas = if (ok) 0 else falhas + 1
            // Sem conexão, espaça as tentativas até 1 min para poupar bateria.
            val pausa = if (ok) 10_000L else minOf(60_000L, 10_000L * (falhas + 1))
            try { Thread.sleep(pausa) } catch (_: InterruptedException) { return }
        }
    }

    /** Busca eventos novos e notifica. Retorna false se o programa não respondeu. */
    private fun verificar(base: String): Boolean {
        val corpo = Config.get("$base/api/eventos?limite=30") ?: return false
        val lista = try { JSONObject(corpo).getJSONArray("eventos") } catch (_: Exception) { return false }
        val ultimo = Config.ultimoEvento(this)
        var maior = ultimo
        val novos = ArrayList<JSONObject>()
        for (i in 0 until lista.length()) {
            val e = lista.getJSONObject(i)
            val id = e.optLong("id")
            if (id > maior) maior = id
            if (ultimo >= 0 && id > ultimo) novos.add(e)
        }
        if (maior != ultimo) Config.salvarUltimoEvento(this, maior)
        // Primeira leitura só marca o ponto de partida (não despeja o histórico).
        if (ultimo < 0) return true
        novos.sortBy { it.optLong("id") }
        for (e in novos.takeLast(5)) notificar(base, e)
        return true
    }

    private fun notificar(base: String, e: JSONObject) {
        val tipo = e.optString("tipo")
        val cam = e.optLong("camera_id")
        val regra = if (e.isNull("regra_id")) null else e.optLong("regra_id")
        val detalhe = e.optString("detalhe")
        val nomeCam = nomeCamera(base, cam)
        val hora = SimpleDateFormat("HH:mm", Locale("pt", "BR")).format(Date(e.optLong("quando") * 1000))
        val (titulo, texto, canal) = when (tipo) {
            "linha_cruzada" -> Triple(nomeRegra(base, cam, regra), getString(R.string.alerta_linha, detalhe, nomeCam), CANAL_ALERTAS)
            "zona_ocupada" -> Triple(nomeRegra(base, cam, regra), getString(R.string.alerta_zona, nomeCam), CANAL_ALERTAS)
            "permanencia" -> Triple(nomeRegra(base, cam, regra), getString(R.string.alerta_permanencia, detalhe, nomeCam), CANAL_ALERTAS)
            "saude" -> if (detalhe == "ok") return else Triple(nomeCam, getString(R.string.alerta_saude, nomeCam, detalhe), CANAL_SAUDE)
            else -> return
        }
        val abrir = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val b = Notification.Builder(this, canal)
            .setSmallIcon(R.drawable.ic_notif)
            .setColor(getColor(R.color.verde))
            .setContentTitle(titulo)
            .setContentText("$texto · $hora")
            .setWhen(e.optLong("quando") * 1000)
            .setShowWhen(true)
            .setAutoCancel(true)
            .setContentIntent(abrir)
            .setCategory(Notification.CATEGORY_EVENT)
        // Foto crua da câmera no momento (o programa nunca desenha caixas).
        if (canal == CANAL_ALERTAS) foto(base, cam)?.let {
            b.setLargeIcon(it).setStyle(Notification.BigPictureStyle().bigPicture(it).setSummaryText("$texto · $hora"))
        }
        getSystemService(NotificationManager::class.java).notify(e.optLong("id").toInt(), b.build())
    }

    private fun foto(base: String, cam: Long): Bitmap? {
        val bytes = Config.getBytes("$base/api/cameras/$cam/foto.jpg", 4000) ?: return null
        val op = BitmapFactory.Options().apply { inSampleSize = 2 }
        return try { BitmapFactory.decodeByteArray(bytes, 0, bytes.size, op) } catch (_: Throwable) { null }
    }

    private fun nomeCamera(base: String, cam: Long): String = nomesCamera.getOrPut(cam) {
        val corpo = Config.get("$base/api/cameras")
        var nome = "Câmera $cam"
        try {
            val l = JSONObject(corpo ?: "{}").getJSONArray("cameras")
            for (i in 0 until l.length()) l.getJSONObject(i).let { if (it.optLong("id") == cam) nome = it.optString("nome", nome) }
        } catch (_: Exception) {
        }
        nome
    }

    private fun nomeRegra(base: String, cam: Long, regra: Long?): String {
        if (regra == null) return getString(R.string.app_nome)
        nomesRegra[regra]?.let { return it }
        try {
            val l = JSONObject(Config.get("$base/api/cameras/$cam/regras") ?: "{}").getJSONArray("regras")
            for (i in 0 until l.length()) l.getJSONObject(i).let { nomesRegra[it.optLong("id")] = it.optString("nome") }
        } catch (_: Exception) {
        }
        return nomesRegra[regra]?.takeIf { it.isNotBlank() } ?: "Regra $regra"
    }

    private fun vigia(texto: String): Notification {
        val abrir = PendingIntent.getActivity(
            this, 1, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Builder(this, CANAL_VIGIA)
            .setSmallIcon(R.drawable.ic_notif)
            .setContentTitle(getString(R.string.app_nome))
            .setContentText(texto)
            .setOngoing(true)
            .setContentIntent(abrir)
            .build()
    }

    private fun criarCanais() {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CANAL_ALERTAS, getString(R.string.canal_alertas), NotificationManager.IMPORTANCE_HIGH)
                .apply { description = getString(R.string.canal_alertas_desc) },
        )
        nm.createNotificationChannel(NotificationChannel(CANAL_SAUDE, getString(R.string.canal_saude), NotificationManager.IMPORTANCE_DEFAULT))
        nm.createNotificationChannel(NotificationChannel(CANAL_VIGIA, getString(R.string.canal_vigia), NotificationManager.IMPORTANCE_MIN))
    }

    companion object {
        private const val ID_VIGIA = Int.MAX_VALUE
        private const val CANAL_ALERTAS = "alertas"
        private const val CANAL_SAUDE = "saude"
        private const val CANAL_VIGIA = "vigia"

        fun iniciar(c: Context) {
            try {
                c.startForegroundService(Intent(c, AlertasService::class.java))
            } catch (_: Exception) {
                // Android 12+ bloqueia iniciar em segundo plano fora de exceções; o app tenta de novo ao abrir.
            }
        }

        fun parar(c: Context) {
            c.stopService(Intent(c, AlertasService::class.java))
        }
    }
}
