---
hide:
  - footer
---

# Apoyadores

CamFácil IA es libre y gratuito, y va a seguir así. Cada apoyo vuelve entero al proyecto: servidor y sitio en línea, cámaras y equipos para probar, nuevas inteligencias y más horas de desarrollo.

=== "Donar"

    <div class="pix">
      <img class="pix__qr" src="../../assets/img/pix-qr.svg" alt="Código QR de Pix para apoyar CamFácil IA" width="200" height="200">
      <div class="pix__dados">
        <p class="pix__rotulo">Pix (Brasil): escanee el código QR con la app del banco</p>
        <p class="pix__nome">Emerson de Carvalho Assis</p>
        <div class="pix__acoes">
          <a class="md-button md-button--primary" href="https://buymeacoffee.com/emersoncarvalho" target="_blank" rel="noopener">Buy Me a Coffee</a>
        </div>
        <p class="pix__nota">Fuera de Brasil, usa Buy Me a Coffee (tarjeta). En Brasil, Pix es lo más rápido.</p>
      </div>
    </div>

=== "Qué libera tu apoyo"

    | Apoyo | Qué recibes |
    |---|---|
    | **Donación libre** · cualquier monto | Tu nombre y tu mensaje en el mural, si quieres |
    | **Hasta R$ 500** · pedido de funcionalidad | Leo y evalúo tu sugerencia de mejora |
    | **Desde R$ 1.000** · conversación | Una hora de conversación conmigo por Google Meet sobre tu proyecto |
    | **Desde R$ 5.000** · desarrollo | Desarrollamos juntos una aplicación para ti, basada en CamFácil IA |

    Los **10 primeros** apoyadores se vuelven **Apoyadores Fundadores**, con sello para siempre en el mural.

    !!! tip "Ideas y pedidos de funcionalidad pasan por aquí"
        ¿Quieres una función nueva? Haz el pedido junto con tu apoyo, en la pestaña **Entrar al mural**. ¿Prefieres hacerla tú? El código es abierto: desarróllala y abre un [pull request](https://github.com/emersoncassis/CamFacil-IA/pulls), que lo evalúo cuando pueda. Los pedidos siguen el alcance del proyecto y no tienen plazo garantizado.

=== "Entrar al mural"

    ¿Ya donaste? Completa, adjunta el comprobante y envía. Me llega directo.

    <form class="apoio-form" data-msg-enviando="Enviando…" data-msg-ok="¡Recibido! Muchas gracias por tu apoyo. Tu nombre entra al mural cuando lo confirme." data-msg-erro="No se pudo enviar ahora. Intenta de nuevo en un momento." data-msg-grande="El comprobante supera 6 MB. Envía una captura más pequeña." data-titulo="Apoyo a CamFácil IA" data-assunto="Apoyo a CamFácil IA" data-anexo="(comprobante adjunto)">
      <div class="apoio-grade">
        <label>Nombre para el mural<input data-campo="Nombre" name="nome" required maxlength="60"></label>
        <label>Instagram o LinkedIn (opcional)<input data-campo="Link" name="link" maxlength="120"></label>
        <label>Monto<input data-campo="Monto" name="valor" inputmode="decimal" maxlength="20"></label>
        <label>Tipo de apoyo
          <select data-campo="Tipo" name="tipo">
            <option>Donación libre</option>
            <option>Pedido de funcionalidad</option>
            <option>Conversación de una hora</option>
            <option>Desarrollar juntos</option>
          </select>
        </label>
        <label class="largo">Mensaje<textarea data-campo="Mensaje" name="recado" rows="1" maxlength="400"></textarea></label>
        <label class="largo">¿Qué quieres? Una función, el tema de la conversación o la aplicación (opcional)<textarea data-campo="Qué quiero" name="pedido" rows="1" maxlength="600"></textarea></label>
        <label class="largo">Comprobante (foto o PDF)<input type="file" name="comprovante" accept="image/*,application/pdf"></label>
        <input class="apoio-isca" name="site" tabindex="-1" autocomplete="off" aria-hidden="true">
      </div>
      <label class="apoio-ok"><input type="checkbox" name="publicar" data-campo="Publicar en el mural" data-sim="sí, autorizo" data-nao="no" checked> Autorizo publicar mi nombre, link y mensaje en el mural</label>
      <div class="apoio-botoes">
        <button class="md-button md-button--primary" type="submit" data-enviar>Enviar</button>
        <span class="apoio-aviso" role="status" aria-live="polite"></span>
      </div>
    </form>

=== "Mural"

    Gracias a quienes creen que la IA de video también es del técnico.

    --8<-- "mural.md"
