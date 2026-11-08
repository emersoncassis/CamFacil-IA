package br.com.ysafe.camfacil

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/**
 * Depois de reiniciar o aparelho (TV Box, celular velho) ou atualizar o app:
 * religa o servidor deste aparelho (se o usuário quis "Iniciar com o aparelho")
 * e a vigia de alertas.
 */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(c: Context, intent: Intent) {
        val atualizou = intent.action == Intent.ACTION_MY_PACKAGE_REPLACED
        if (Config.servidor(c) && Config.servidorLigado(c) && (atualizou || Config.iniciarNoBoot(c))) {
            ServidorService.iniciar(c)
        }
        if (Config.alertasLigados(c) && Config.local(c)) AlertasService.iniciar(c)
    }
}
