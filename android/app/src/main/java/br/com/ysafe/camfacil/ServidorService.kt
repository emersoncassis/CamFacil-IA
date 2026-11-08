package br.com.ysafe.camfacil

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import java.io.File

/**
 * Servidor local no próprio aparelho (ADR 0011): roda o núcleo Rust
 * (`libcamfacil.so`, um executável empacotado em jniLibs) como processo filho,
 * em primeiro plano com notificação fixa. O núcleo serve a dashboard em
 * 127.0.0.1, liga as câmeras com o `libffmpeg.so` do próprio APK, roda as
 * regras e manda o resumo ao site, igual ao programa do PC.
 *
 * Se o processo cair, religa com espera crescente (2 s → 60 s).
 */
class ServidorService : Service() {
    @Volatile private var querLigado = false
    private var processo: Process? = null
    private var fio: Thread? = null
    private var wake: PowerManager.WakeLock? = null
    private var wifi: WifiManager.WifiLock? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACAO_DESLIGAR) {
            Config.ligarServidor(this, false)
            stopSelf()
            return START_NOT_STICKY
        }
        criarCanal()
        val n = notificacao(getString(R.string.servidor_ligando))
        if (Build.VERSION.SDK_INT >= 34) {
            startForeground(ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } else {
            startForeground(ID, n)
        }
        if (!querLigado) {
            querLigado = true
            segurarCpuEWifi()
            fio = Thread({ laco() }, "servidor").apply { start() }
        }
        return START_STICKY
    }

    override fun onDestroy() {
        querLigado = false
        fio?.interrupt()
        processo?.destroy()
        estado = Estado()
        wake?.takeIf { it.isHeld }?.release()
        wifi?.takeIf { it.isHeld }?.release()
        super.onDestroy()
    }

    /** Câmeras não podem parar com a tela apagada: CPU e Wi-Fi acordados enquanto o servidor roda. */
    private fun segurarCpuEWifi() {
        wake = getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "camfacil:servidor")
            .apply { setReferenceCounted(false); acquire() }
        @Suppress("DEPRECATION")
        wifi = (applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager)
            .createWifiLock(WifiManager.WIFI_MODE_FULL_HIGH_PERF, "camfacil:servidor")
            .apply { setReferenceCounted(false); acquire() }
    }

    private fun laco() {
        var espera = 2_000L
        while (querLigado) {
            val inicio = System.currentTimeMillis()
            val erro = rodarUmaVez()
            if (!querLigado) break
            estado = estado.copy(rodando = false, pronto = false, erro = erro ?: getString(R.string.servidor_caiu))
            atualizarNotificacao(getString(R.string.servidor_religando))
            // Rodou bastante antes de cair: volta rápido. Caindo em seguida: espaça.
            espera = if (System.currentTimeMillis() - inicio > 60_000) 2_000L else minOf(60_000L, espera * 2)
            try { Thread.sleep(espera) } catch (_: InterruptedException) { return }
        }
    }

    /** Lança o núcleo e lê a saída até ele terminar. Devolve o erro, se houver. */
    private fun rodarUmaVez(): String? {
        val libs = applicationInfo.nativeLibraryDir
        val exe = File(libs, "libcamfacil.so")
        if (!exe.isFile) return getString(R.string.servidor_sem_nucleo)
        val dados = File(filesDir, "camfacil").apply { mkdirs() }
        val args = mutableListOf(exe.absolutePath, "--no-browser")
        if (Config.aceitarRede(this)) args += "--lan"
        val pb = ProcessBuilder(args).directory(dados).redirectErrorStream(true)
        pb.environment().apply {
            put("CAMFACIL_DATA", dados.absolutePath)
            put("HOME", filesDir.absolutePath)
            put("TMPDIR", cacheDir.absolutePath)
            put("CAMFACIL_SEM_PAINEL", "1")
            put("CAMFACIL_BOOT", if (Config.iniciarNoBoot(this@ServidorService)) "1" else "0")
            put("SSL_CERT_DIR", "/system/etc/security/cacerts")
            File(libs, "libffmpeg.so").takeIf { it.isFile }?.let { put("CAMFACIL_FFMPEG", it.absolutePath) }
            File(libs, "libonnxruntime.so").takeIf { it.isFile }?.let { put("CAMFACIL_ORT_LIB", it.absolutePath) }
        }
        val p = try {
            pb.start()
        } catch (e: Exception) {
            return getString(R.string.servidor_nao_abriu, e.message ?: "")
        }
        processo = p
        estado = Estado(rodando = true)
        val ultimas = ArrayDeque<String>()
        p.inputStream.bufferedReader().useLines { linhas ->
            for (l in linhas) {
                ultimas.addLast(l)
                if (ultimas.size > 20) ultimas.removeFirst()
                PORTA.find(l)?.let { m ->
                    val porta = m.groupValues[1].toInt()
                    Config.salvarPortaServidor(this, porta)
                    estado = estado.copy(porta = porta, pronto = true, erro = null)
                    atualizarNotificacao(getString(R.string.servidor_ligado_em, "127.0.0.1:$porta"))
                }
                estado = estado.copy(log = ultimas.joinToString("\n"))
            }
        }
        val codigo = try { p.waitFor() } catch (_: InterruptedException) { return null }
        processo = null
        return if (codigo == 0) null else ultimas.lastOrNull { it.isNotBlank() } ?: getString(R.string.servidor_caiu)
    }

    private fun notificacao(texto: String): Notification {
        val abrir = PendingIntent.getActivity(
            this, 2, Intent(this, ServidorActivity::class.java), PendingIntent.FLAG_IMMUTABLE,
        )
        val desligar = PendingIntent.getService(
            this, 3, Intent(this, ServidorService::class.java).setAction(ACAO_DESLIGAR), PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Builder(this, CANAL)
            .setSmallIcon(R.drawable.ic_notif)
            .setContentTitle(getString(R.string.servidor_notif_titulo))
            .setContentText(texto)
            .setOngoing(true)
            .setContentIntent(abrir)
            .addAction(Notification.Action.Builder(null, getString(R.string.servidor_desligar), desligar).build())
            .build()
    }

    private fun atualizarNotificacao(texto: String) {
        if (querLigado) getSystemService(NotificationManager::class.java).notify(ID, notificacao(texto))
    }

    private fun criarCanal() {
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel(CANAL, getString(R.string.canal_servidor), NotificationManager.IMPORTANCE_LOW),
        )
    }

    /** O que a tela do servidor mostra. Só leitura fora deste serviço. */
    data class Estado(
        val rodando: Boolean = false,
        val pronto: Boolean = false,
        val porta: Int = 0,
        val erro: String? = null,
        val log: String = "",
    )

    companion object {
        private const val ID = Int.MAX_VALUE - 1
        private const val CANAL = "servidor"
        private const val ACAO_DESLIGAR = "br.com.ysafe.camfacil.DESLIGAR"
        /** O núcleo imprime "dashboard em http://localhost:7070" ao subir. */
        private val PORTA = Regex("""dashboard em http://localhost:(\d+)""")

        @Volatile var estado = Estado()
            private set

        fun iniciar(c: Context) {
            Config.ligarServidor(c, true)
            try {
                c.startForegroundService(Intent(c, ServidorService::class.java))
            } catch (_: Exception) {
                // Android 12+ pode recusar em segundo plano; a tela do servidor tenta de novo ao abrir.
            }
        }

        fun parar(c: Context) {
            Config.ligarServidor(c, false)
            c.stopService(Intent(c, ServidorService::class.java))
        }
    }
}
