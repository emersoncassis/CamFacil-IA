# Primeros pasos

El programa en la computadora del cliente es solo para **operación**. Si quieres aprender desde cero, empieza por la [Formación de IA para CCTV](formacao.md), en el navegador.

## Conectar la primera cámara

1. **Abre el programa.** Arranca en **Cámaras** y busca solo en la red (búsqueda ONVIF).
2. **Elige la cámara**, escribe usuario y contraseña y haz clic en conectar. La imagen aparece al instante.
3. **Enciende la IA cuando quieras**: dibuja la línea o la zona, o pide *"IA, sugiérelo por mí"*.

¿No tienes cámara cerca? Usa la **webcam**, un **archivo de video** o la **escena de demostración**.

!!! tip "Login guardado"
    Las cámaras con el mismo usuario y contraseña entran de una vez. La contraseña queda cifrada en la máquina.

## Lo que ve el cliente

El cliente nunca ve cajas ni trazos de la IA. Solo ve el **globo con el nombre de la regla**, que parpadea en la alerta, con sonido y pantalla parpadeando.

## Qué trae

| Recurso | Qué hace |
|---|---|
| **En vivo** | Mosaico de 1 a 4 cámaras con el estado abajo: equipo, IA, alertas y servicios |
| **Diagnóstico de red** | Encuentra la cámara que cambió de IP por la MAC y conflictos de IP, e indica qué hacer |
| **Diagrama de red** | Dibuja el unifilar de la instalación (SVG para descargar) |
| **Telegram y WhatsApp** | Alertas y comandos con código QR (WhatsApp vía Evolution API, con un chip dedicado) |
| **Modo TV** | Pantalla de quiosco para el cliente, con tu logo |
| **App Android** | Se vuelve el servidor de las cámaras o mira a un cliente a distancia con Tailscale |
| **Informe de validación** | PDF con tu nombre y el del cliente, fotos y alertas de la semana para cerrar la venta |
| **MCP** | Configura todo conversando con una IA (apagado por defecto) |
| **Modo estudio** *(básico)* | Describe una inteligencia nueva, etiqueta, entrena en Colab y compártela (solo estudio) |

??? question "¿El programa queda abierto todo el tiempo?"
    Puedes hacer que arranque con la computadora. La ventana de la terminal muestra el resumen en vivo: CPU, memoria y cámaras.
