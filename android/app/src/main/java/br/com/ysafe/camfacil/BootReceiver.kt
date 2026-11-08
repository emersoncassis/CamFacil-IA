package br.com.ysafe.camfacil

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Religa a vigia de alertas depois de reiniciar o aparelho (TV Box, celular velho). */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(c: Context, intent: Intent) {
        if (Config.alertasLigados(c) && Config.local(c)) AlertasService.iniciar(c)
    }
}
