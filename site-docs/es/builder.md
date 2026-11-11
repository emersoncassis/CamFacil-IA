---
title: CamFácil IA Builder
description: El técnico robot que te ayuda a montar tu versión de CamFácil IA, aunque no sepas programar, dentro de la ley de privacidad y de la licencia.
---

# CamFácil IA Builder

<div class="builder" markdown>
<div class="builder__arte">
  <img src="../../assets/img/builder.svg" alt="CamFácil IA Builder: un robot técnico de CCTV, con una cámara domo en la cabeza y un destornillador en la mano, escribe código en un portátil." width="640" height="440">
</div>
<div class="builder__texto" markdown>
<span class="manifesto__kicker">El maestro de obras del proyecto</span>

<p class="builder__titulo">Tú dices lo que quieres. <em>Él construye.</em></p>

El Builder es una skill para asistentes de código (Claude Code, Codex y otros). Conversas como hablarías con un compañero de instalación, y él cambia pantallas y crea funciones e inteligencias en **tu versión** de CamFácil IA. Sin que escribas una línea de código. Por defecto trabaja en portugués de Brasil; pídele que responda en tu idioma.

[:fontawesome-brands-github: Hacer el fork](https://github.com/emersoncassis/CamFacil-IA/fork){ .md-button .md-button--primary }
[Ver la skill](https://github.com/emersoncassis/CamFacil-IA/blob/main/.claude/skills/camfacil-ia-builder/SKILL.md){ .md-button }
</div>
</div>

## Empieza en tres pasos

1. **Haz el fork** del repositorio en GitHub y descarga tu copia:

    ```sh
    git clone https://github.com/TU-USUARIO/CamFacil-IA && cd CamFacil-IA
    ```

2. **Abre la carpeta en tu asistente de código.** En Claude Code, basta con ejecutar `claude` dentro de ella. Otros asistentes leen el archivo `AGENTS.md`, que apunta al Builder.

3. **Llama al Builder** y di lo que quieres:

    ```text
    /camfacil-ia-builder quiero un aviso cuando alguien se quede más de 2 minutos parado frente al portón
    ```

Él revisa las herramientas de tu computadora, instala lo que falte, abre el programa original para que veas el "antes" y solo entonces empieza.

## Lo que puedes pedir

??? example "Cambiar el aspecto del programa"
    Cambiar nombre, logo, colores y textos por los de tu empresa, reorganizar pantallas, crear un modo para la TV del cliente. Los colores salen de un solo archivo, así que el cambio vale para todo el programa.

??? example "Crear una inteligencia nueva"
    Un tipo nuevo de alerta sobre personas y vehículos (línea, zona, permanencia, conteo), o un modelo entrenado por ti en el modo estudio.

??? example "Conectar con otros sistemas"
    Una marca nueva de cámara o grabador, un aviso en otro canal, un webhook para la automatización del cliente.

??? example "Informes y mantenimiento"
    Un informe con tu marca, un diagnóstico de red más, una pantalla de salud de las cámaras como tú se la explicas al cliente.

## Lo que no hace, y por qué

El Builder rechaza el pedido que infringe la ley, la licencia o la seguridad. Dice **qué regla** y ofrece **qué se puede hacer en su lugar**.

| Tú pides | Él responde |
|---|---|
| Reconocer quién es la persona por el rostro | No. La biometría es dato sensible (LGPD, art. 11). En su lugar: detectar presencia, permanencia y cruce de línea, sin identificar a nadie. |
| Quitar el cartel de aviso o los términos | No. La LGPD exige transparencia con quien es filmado. Personaliza el texto con tu marca, pero no lo quita. |
| Acceder a una cámara que no es del cliente | No. Invadir un dispositivo es delito (Código Penal de Brasil, art. 154-A). |
| Cerrar el código de tu versión | No. La licencia AGPL-3.0 exige el código abierto, también para quien la usa por la red. |
| Quitar el PIN del acceso por la red | No. Una brecha en tu versión es una puerta abierta en la tienda del cliente. |
| Panel multicliente, grabación continua | Queda fuera del proyecto oficial. En tu versión lo hace, después de avisarte qué cambia en tus responsabilidades. |

Las reglas citan la ley de Brasil, donde nació el proyecto. Si trabajas en otro país, también se aplica tu ley local de privacidad. Lista completa (en portugués): [límites del Builder](https://github.com/emersoncassis/CamFacil-IA/blob/main/.claude/skills/camfacil-ia-builder/referencias/limites.md). Es orientación técnica, no asesoría jurídica.

## Cómo trabaja

El Builder sigue el método de **Reversa**: primero lo acordado, después el código.

1. **Entiende** el pedido y lo repite con sus palabras, para que confirmes.
2. **Revisa** contra la ley, la licencia, la seguridad y las reglas del proyecto.
3. **Especifica** la función y muestra el plan en pocas líneas. Solo sigue con tu visto bueno.
4. **Construye** en pasos pequeños, en una copia de prueba. El original nunca se pierde.
5. **Prueba**: ejecuta los tests, abre el programa y lo muestra funcionando.
6. **Documenta** lo que cambió, para que la próxima persona (o la próxima IA) continúe.

## Tu versión, tus obligaciones

- **Sigue siendo libre.** Tu versión continúa bajo AGPL-3.0, con el código abierto y los créditos preservados.
- **Di que es una versión modificada**, con tu nombre y la fecha. La marca que ve el cliente es la tuya, nunca la de un fabricante.
- **La privacidad es responsabilidad tuya.** Usa los [modelos de término, autorización y cartel](privacidade.md).
- **¿Mejoraste algo que sirve para todos?** Envíalo de vuelta: mira [Contribuir](contribuindo.md).

<footer class="builder__creditos" markdown>
**Créditos.** CamFácil IA Builder está construido sobre [Reversa](https://github.com/sandeco/reversa), el framework de ingeniería inversa de especificaciones creado por el profesor **Sandeco Macedo** ([@sandeco](https://github.com/sandeco)), bajo licencia MIT. Todo el método de desarrollo del Builder viene de él. [Documentación de Reversa](https://sandeco.github.io/reversa/es/) · [Artículo](https://arxiv.org/abs/2605.18684)
</footer>
