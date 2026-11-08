# Instalación

Un comando y el programa se abre solo en el navegador. Funciona en PC vieja, mini PC, Raspberry Pi y TV box.

=== "Windows 10 y 11"

    Pega en **PowerShell**. El instalador descarga el programa, instala ffmpeg, crea el acceso directo y lo abre:

    ```powershell
    irm https://cfia.ysafe.com.br/instalar.ps1 | iex
    ```

    Si aparece la pantalla azul de Windows, haz clic en **Más información → Ejecutar de todas formas**. El programa es gratuito y no tiene certificado pago.

=== "Linux"

    ```sh
    curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh
    ```

=== "Raspberry Pi y TV Box"

    Para ARM de 64 bits:

    ```sh
    curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh
    ```

=== "Android"

    Para celular y TV box: descarga la [app Android (APK)](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-android.apk), ábrela en el equipo y permite instalar de fuentes desconocidas cuando lo pida. Se vuelve el servidor de las cámaras o mira a un cliente a distancia con Tailscale.

=== "Sin comando"

    Descarga directo de la [última versión](https://github.com/emersoncassis/CamFacil-IA/releases/latest) y haz doble clic:

    - [Windows](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-windows-x86_64.exe)
    - [Linux](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-linux-x86_64)
    - [Raspberry Pi / ARM 64](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-linux-arm64)
    - [Android (APK)](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-android.apk)

=== "Desde el código"

    Para quien programa (requiere Rust):

    ```sh
    git clone https://github.com/emersoncassis/CamFacil-IA && cd CamFacil-IA
    cargo run --release -p camfacil
    ```

## Puerto sin conflicto

CamFácil IA elige solo un puerto libre, empezando por el **7070**, y nunca usa puertos del POS, ERP, base de datos o NVR del cliente. La dirección aparece en pantalla al iniciar.

## Lo que la máquina aguanta

Al abrir, el programa prueba la computadora y habilita solo los módulos que puede correr.

| Nivel | Módulos | Funciona en |
|:---:|---|---|
| **Liviano** | Movimiento inteligente · Cerca y línea virtual · Salud de las cámaras | Casi cualquier PC |
| **Medio** | Personas y vehículos · Conteo · Permanencia · Mapa de calor | PC de 4 núcleos, mini PC |
| **Pesado** | Lectura de patentes · Reconocimiento facial | Máquinas más potentes |

!!! note "Siguiente paso"
    ¿Programa abierto? Ve a [Primeros pasos](uso.md).
