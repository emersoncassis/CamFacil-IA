# Instalação

Um comando e o programa abre sozinho no navegador. Funciona em PC velho, mini PC, Raspberry Pi e TV Box.

=== "Windows 10 e 11"

    Cole no **PowerShell**. O instalador baixa o programa, instala o ffmpeg, cria o atalho (com o ícone do CamFácil IA) e abre:

    ```powershell
    irm https://cfia.ysafe.com.br/instalar.ps1 | iex
    ```

    Se aparecer a tela azul do Windows, clique em **Mais informações → Executar assim mesmo**. O programa é gratuito e não tem certificado pago.

=== "Linux"

    ```sh
    curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh
    ```

=== "Raspberry Pi e TV Box"

    Para ARM de 64 bits:

    ```sh
    curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh
    ```

=== "Android"

    Para celular e TV Box: baixe o [app Android (APK)](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-android.apk), abra no aparelho e permita instalar de fonte desconhecida quando ele pedir. Ele vira o servidor das câmeras ou vê um cliente à distância pelo Tailscale.

=== "Sem comando"

    Baixe direto da [última versão](https://github.com/emersoncassis/CamFacil-IA/releases/latest) e dê dois cliques:

    - [Windows](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-windows-x86_64.exe)
    - [Linux](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-linux-x86_64)
    - [Raspberry Pi / ARM 64](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-linux-arm64)
    - [Android (APK)](https://github.com/emersoncassis/CamFacil-IA/releases/latest/download/camfacil-android.apk)

=== "A partir do código"

    Para quem programa (precisa do Rust):

    ```sh
    git clone https://github.com/emersoncassis/CamFacil-IA && cd CamFacil-IA
    cargo run --release -p camfacil
    ```

## Porta sem conflito

O CamFácil IA escolhe sozinho uma porta livre, começando pela **7070**, e nunca usa portas de PDV, ERP, banco de dados ou NVR do cliente. O endereço aparece na tela ao iniciar.

## O que a máquina aguenta

Ao abrir, o programa testa o computador e libera só os módulos que ele aguenta.

| Nível | Módulos | Roda em |
|:---:|---|---|
| **Leve** | Movimento inteligente · Cerca e linha virtual · Saúde das câmeras | Quase qualquer PC |
| **Médio** | Pessoas e veículos · Contagem · Permanência · Mapa de calor | PC de 4 núcleos, mini PC |
| **Pesado** | Leitura de placas · Reconhecimento facial | Máquinas mais fortes |

!!! note "Próximo passo"
    Programa aberto? Vá para [Primeiros passos](uso.md).
