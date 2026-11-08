package br.com.ysafe.camfacil

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.hardware.Camera
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.view.Gravity
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.widget.FrameLayout
import android.widget.TextView
import android.widget.Toast
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer

/**
 * Leitor de QR leve: API Camera (1) + núcleo do ZXing. Evita CameraX e
 * Play Services, que não existem em muita TV Box e pesam no APK.
 */
@Suppress("DEPRECATION")
class QrActivity : Activity(), SurfaceHolder.Callback, Camera.PreviewCallback {
    private var camera: Camera? = null
    private val fio = HandlerThread("qr").apply { start() }
    private val trabalho = Handler(fio.looper)
    @Volatile private var ocupado = false
    @Volatile private var pronto = false
    private val leitor = MultiFormatReader().apply {
        setHints(mapOf(DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE), DecodeHintType.TRY_HARDER to true))
    }
    private lateinit var superficie: SurfaceView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        superficie = SurfaceView(this)
        val raiz = FrameLayout(this)
        raiz.addView(superficie)
        raiz.addView(TextView(this).apply {
            setText(R.string.qr_dica)
            setTextColor(getColor(R.color.texto))
            setBackgroundColor(0xB3000000.toInt())
            gravity = Gravity.CENTER
            setPadding(24, 24, 24, 24)
        }, FrameLayout.LayoutParams(-1, -2, Gravity.BOTTOM))
        setContentView(raiz)
        if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            superficie.holder.addCallback(this)
        } else {
            requestPermissions(arrayOf(Manifest.permission.CAMERA), 1)
        }
    }

    override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
        if (grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED) {
            superficie.holder.addCallback(this)
            if (superficie.holder.surface?.isValid == true) surfaceCreated(superficie.holder)
        } else {
            Toast.makeText(this, R.string.qr_sem_camera, Toast.LENGTH_LONG).show()
            finish()
        }
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        if (camera != null) return
        try {
            val c = Camera.open() ?: return finish()
            val p = c.parameters
            if (p.supportedFocusModes.contains(Camera.Parameters.FOCUS_MODE_CONTINUOUS_PICTURE)) {
                p.focusMode = Camera.Parameters.FOCUS_MODE_CONTINUOUS_PICTURE
            }
            // Menor tamanho de pré-visualização >= 640 px: decodifica rápido em celular velho.
            p.supportedPreviewSizes.filter { it.width >= 640 }.minByOrNull { it.width * it.height }
                ?.let { p.setPreviewSize(it.width, it.height) }
            c.parameters = p
            c.setDisplayOrientation(90)
            c.setPreviewDisplay(holder)
            c.setPreviewCallback(this)
            c.startPreview()
            camera = c
            pronto = true
        } catch (_: Exception) {
            Toast.makeText(this, R.string.qr_sem_camera, Toast.LENGTH_LONG).show()
            finish()
        }
    }

    override fun onPreviewFrame(data: ByteArray, cam: Camera) {
        if (ocupado || !pronto) return
        ocupado = true
        val tam = try { cam.parameters.previewSize } catch (_: Exception) { ocupado = false; return }
        val copia = data.copyOf()
        trabalho.post {
            val texto = try {
                val fonte = PlanarYUVLuminanceSource(copia, tam.width, tam.height, 0, 0, tam.width, tam.height, false)
                leitor.decodeWithState(BinaryBitmap(HybridBinarizer(fonte))).text
            } catch (_: Exception) {
                null
            } finally {
                leitor.reset()
            }
            if (texto != null) {
                pronto = false
                runOnUiThread {
                    setResult(RESULT_OK, Intent().putExtra(RESULTADO, texto))
                    finish()
                }
            }
            ocupado = false
        }
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {}
    override fun surfaceDestroyed(holder: SurfaceHolder) = soltar()

    private fun soltar() {
        pronto = false
        camera?.let {
            it.setPreviewCallback(null)
            it.stopPreview()
            it.release()
        }
        camera = null
    }

    override fun onPause() {
        super.onPause()
        if (isFinishing) soltar()
    }

    override fun onDestroy() {
        soltar()
        fio.quitSafely()
        super.onDestroy()
    }

    companion object {
        const val RESULTADO = "qr"
    }
}
