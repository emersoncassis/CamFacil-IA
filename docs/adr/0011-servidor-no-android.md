# ADR 0011 · Servidor no próprio Android (núcleo Rust + ffmpeg mínimo no APK)

**Situação:** aceita · 2026-11-07 · complementa a [ADR 0009](0009-app-android-kotlin-webview.md)

## Contexto

O app Android (ADR 0009) só via um servidor que roda em outro lugar. O dono do produto quer que o **mesmo APK** faça tudo o que o programa do PC faz: ligar câmeras RTSP, rodar as regras de linha/zona, servir a dashboard e mandar o resumo ao site, numa TV Box ou num celular velho ligado na tomada. Na tela inicial o usuário escolhe "Este aparelho é o servidor" ou "Ver um servidor à distância".

O núcleo (`camfacil`) já é um binário Rust sem dependências de sistema além da libc, e compila limpo para `aarch64-linux-android` e `armv7-linux-androideabi`. O ponto difícil é o vídeo: no PC um processo **ffmpeg** externo por câmera entrega MJPEG (640 px, 5 q/s) no stdout (`crates/camfacil/src/video.rs`, ADR 0003). Android não tem ffmpeg.

## Opções para o vídeo

| Opção | Custo no APK | Paridade com o PC | Trabalho / risco |
|---|---|---|---|
| **A. ffmpeg mínimo estático (LGPL) empacotado** | ~2,8 MB por ABI comprimido no APK (3,0 MB arm64 / 2,8 MB armv7 sem compressão) | Total: mesmos argumentos, mesmo RTSP/H.264/H.265, demo e arquivo | Baixo: script de build; nenhum código novo de vídeo |
| B. MediaCodec no Kotlin entregando quadros | ~0 | Parcial: precisaria de cliente RTSP/RTP próprio em Kotlin (Android não tem), depacketizer H.264/H.265, YUV→JPEG e um canal novo até o Rust | Alto: centenas de linhas novas só para Android, mais um caminho de vídeo para manter |
| C. Foto HTTP da câmera (snapshot) | ~0 | Baixa: 1–2 q/s, URL e autenticação Digest diferentes por fabricante, perde cruzamento de linha rápido | Médio: código novo de fonte de vídeo no núcleo |

## Decisão

1. **Vídeo: opção A.** `scripts/android-ffmpeg.sh` compila o ffmpeg 7.1.2 (SHA-256 conferido) com `--disable-everything` e liga só o que o núcleo usa: protocolos `file,pipe,tcp,udp,rtp,http`; demuxers `rtsp,rtp,sdp,mov,h264,hevc,mjpeg,mpjpeg`; decoders `h264,hevc,mjpeg` (+ `wrapped_avframe`, exigido pelo lavfi); encoder/muxer `mjpeg`; filtros `fps,scale,format,null,buffer,buffersink` e `color,drawbox,overlay` (cena demo, via `lavfi`). `--enable-small`, sem `--enable-gpl` (LGPL 2.1+, compatível com distribuir junto do AGPL; fonte e script públicos). Páginas de 16 KB (`-z max-page-size=16384`) para Android 15+.
   - O núcleo **não muda**: o app passa `CAMFACIL_FFMPEG=<nativeLibraryDir>/libffmpeg.so`, que `achar_ffmpeg()` já respeita.
   - B e C ficam descartadas por ora: B duplicaria o caminho de vídeo; C degrada a função principal (regras de linha). C pode voltar como fonte extra para câmeras sem RTSP.
2. **Núcleo no APK:** `scripts/android-nucleo.sh` compila `camfacil` com `cargo-ndk` (API 24) para `arm64-v8a` e `armeabi-v7a` e copia como `jniLibs/<abi>/libcamfacil.so` (e o ffmpeg como `libffmpeg.so`). Com `packaging.jniLibs.useLegacyPackaging = true` o Android extrai os dois para o `nativeLibraryDir`, o único lugar de onde um app pode executar binário (W^X do Android 10+). `x86_64` (emulador) é opcional no script e não vai no APK de release.
3. **Execução:** `ServidorService` (serviço em primeiro plano `specialUse`, notificação fixa com "Desligar") lança o núcleo com `ProcessBuilder` (`--no-browser`, `--lan` só se o usuário ligar "Aceitar acesso pela rede Wi-Fi da loja"), com `CAMFACIL_DATA` na pasta privada do app, lê a porta real da linha "dashboard em http://localhost:N" e religa com espera crescente se o processo cair. Segura `PARTIAL_WAKE_LOCK` e `WifiLock` enquanto ligado: câmera não pode parar com a tela apagada. "Iniciar com o aparelho" religa pelo `BootReceiver`.
4. **Ajustes no Rust, só com `cfg(target_os = "android")`:**
   - **HTTPS:** o reqwest usa `rustls-platform-verifier`, que no Android exige a JVM (JNI), ausente num processo à parte. `cf_notify::tls::raizes_do_sistema` carrega as autoridades de `/apex/com.android.conscrypt/cacerts` ou `/system/etc/security/cacerts` (no PC não faz nada). O elo com o site (tungstenite) já tinha `webpki-roots`.
   - **Tailscale:** o app oficial não expõe o comando `tailscale`. O IP vem da tabela de rotas (`ip_pela_rota`: `connect` UDP para 100.100.100.100 sem mandar pacote; aceita só 100.64.0.0/10 diferente da saída para a internet, para não confundir com CGNAT da operadora). O núcleo passa a escutar nesse IP, como no PC.
   - **Início automático e mensagens do ffmpeg/Tailscale** com textos de Android (o boot é do app; `CAMFACIL_BOOT` informa o estado à dashboard).
5. **Visualizador:** no modo servidor o WebView abre `http://127.0.0.1:<porta>`; a tela do servidor mostra o endereço Tailscale (via `/api/tailscale`) e como abrir de outro celular ("Ver um servidor à distância").

## Consequências

- **APK de ~136 KB para ~7,1 MB** (arm64 + armv7 no mesmo APK; instalado ocupa ~7,4 MB num aparelho arm64). Continua abaixo da meta de 10 MB do SDD-06 e é o preço de fazer tudo o que o PC faz sem baixar nada depois (offline-first). Se crescer, a saída é dividir por ABI (`splits.abi`) — cada APK cairia para ~4 MB.
- RAM/CPU no aparelho: as mesmas do PC (núcleo ocioso < 50 MB) mais um ffmpeg por câmera; decodificar H.264 em software no subfluxo (D1/640 px) é leve, mas **use o subfluxo** em TV Box fraca.
- **Nível médio da IA (YOLO/ONNX) não roda no Android ainda:** não há pacote oficial do ONNX Runtime para Android no formato que o núcleo baixa; o painel mostra o aviso que já existe ("não há ONNX Runtime oficial para android"). Regras de linha/zona (nível leve) funcionam.
- **Webcam** (`v4l2`/`dshow`) não existe no Android; fontes válidas lá: RTSP, arquivo e demo.
- **Android 12+ "phantom process killer"**: o sistema pode matar processos filhos (núcleo e ffmpeg) se passarem de 32 no total ou gastarem muita CPU. O serviço religa o núcleo e o núcleo religa o ffmpeg; em aparelho dedicado, desligar em Opções do desenvolvedor → "Desativar restrições de processos filhos" (Android 14+) ou `adb shell settings put global settings_enable_monitor_phantom_procs false`.
- H.264/H.265 têm patentes; o ffmpeg só decodifica (como qualquer player). Mantemos `--disable-gpl` e sem codecs de terceiros.
- CI: o job `android` instala os alvos Rust e o `cargo-ndk`, compila núcleo e ffmpeg (cache do ffmpeg pela hash do script) com o NDK do runner, e só então roda o Gradle.
