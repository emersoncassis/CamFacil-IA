# Installation

One command and the program opens by itself in the browser. Works on old PCs, mini PCs, Raspberry Pi and TV boxes.

=== "Windows 10 and 11"

    Paste into **PowerShell**. The installer downloads the program, installs ffmpeg, creates the shortcut (with the CamFácil IA icon) and opens it:

    ```powershell
    irm https://cfia.ysafe.com.br/instalar.ps1 | iex
    ```

    If the blue Windows screen shows up, click **More info → Run anyway**. The program is free and has no paid certificate.

=== "Linux"

    ```sh
    curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh
    ```

=== "Raspberry Pi and TV Box"

    For 64-bit ARM:

    ```sh
    curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh
    ```

=== "Android"

    For phones and TV boxes: download the [Android app (APK)](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-android.apk), open it on the device and allow installing from unknown sources when asked. It becomes the camera server or watches a customer remotely through Tailscale.

=== "No command"

    Download straight from the [latest release](https://github.com/emersoncassis/CamFacil-IA/releases/latest) and double-click:

    - [Windows](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-windows-x86_64.exe)
    - [Linux](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-linux-x86_64)
    - [Raspberry Pi / ARM 64](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-linux-arm64)
    - [Android (APK)](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-android.apk)

=== "From source"

    For developers (requires Rust):

    ```sh
    git clone https://github.com/emersoncassis/CamFacil-IA && cd CamFacil-IA
    cargo run --release -p camfacil
    ```

## No port conflicts

CamFácil IA picks a free port by itself, starting at **7070**, and never uses the customer's POS, ERP, database or NVR ports. The address is shown on screen at startup.

## What the machine can handle

When it opens, the program benchmarks the computer and only enables the modules it can run.

| Level | Modules | Runs on |
|:---:|---|---|
| **Light** | Smart motion · Virtual fence and line · Camera health | Almost any PC |
| **Medium** | People and vehicles · Counting · Loitering · Heat map | 4-core PC, mini PC |
| **Heavy** | License plate reading · Face recognition | Stronger machines |

!!! note "Next step"
    Program running? Go to [Getting started](uso.md).
