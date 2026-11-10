# Relatório · Fase 6 · App Android

Branch `fase6-android`. Decisão em [ADR 0009](adr/0009-app-android-kotlin-webview.md).

## Entregue

- **`android/`**: app Kotlin (sem AndroidX), minSdk 24, targetSdk 35, versão lida do `Cargo.toml`.
  - **Painel** em WebView (programa local ou `cfia.ysafe.com.br`), links externos abrem no navegador, tela de erro com "Tentar de novo"/"Parear de novo".
  - **Pareamento**: ler o QR da dashboard (Integrações → Tailscale, `/api/tailscale/qr.svg`), digitar o endereço (`192.168.0.10` vira `http://192.168.0.10:7070`) ou usar o site. Testa `/api/saude` antes de salvar; `/api/estado` diz se é o programa local.
  - **Tailscale**: detecta o app oficial (`com.tailscale.ipn`) e oferece abrir ou instalar.
  - **Alertas**: serviço em primeiro plano (opcional, pelo menu) lê `/api/eventos` a cada 10 s; notificação com o **nome da regra** no título e a foto crua da câmera (`foto.jpg`), nunca caixas da IA. Canais separados para regras e saúde das câmeras. Religa no boot. Menu pede exceção de bateria.
  - **Câmeras ao vivo** (`assets/cameras.html`): MJPEG do programa; mede atraso e banda a cada 5 s, troca para foto a cada 3 s após 2 medições ruins e só volta após 3 boas; botões Automático / Ao vivo / Só foto.
  - **Visual**: preto, texto claro, verde só em detalhes (ícone, indicador, aba ativa), cores espelhando `web/tokens.css`. Ícone adaptativo e banner de Android TV (aparece no launcher da TV Box).
  - Textos em `res/values/strings.xml` (pt-BR).
- **CI** (`.github/workflows/rust.yml`): job `android` compila `assembleRelease` e entrega `camfacil-android.apk`; o job `release` anexa à versão nova e, se a versão atual já existe sem o APK, faz o upload.
  - Assinatura: secrets opcionais `CFIA_KEYSTORE_BASE64`, `CFIA_KEYSTORE_SENHA`, `CFIA_KEY_ALIAS`, `CFIA_KEY_SENHA`. Sem eles, assina com a chave de debug do runner.
- **Site** (`crates/cfia-api/web/index.html`): cartão "App Android (APK)" na seção Baixar, com link para `releases/latest/download/camfacil-android.apk` e instrução de fontes desconhecidas.

## Build local

```sh
export ANDROID_HOME=~/android-sdk   # platforms;android-35 e build-tools;35.0.0
cd android && ./gradlew assembleRelease
# saída: android/app/build/outputs/apk/release/app-release.apk (~136 KB)
```

> **Atualização em 2026-11-10.** O que era pendência desta fase e já foi feito depois: **botões personalizáveis** (portão e relé, com PIN, confirmação e log; SDD-06, aberto pelo menu "Portão e relé" do app), **servidor no próprio aparelho** (ADR 0011) e **IA de pessoas no Android** (ADR 0014). Continuam pendentes: a keystore de release e os secrets, e os perfis técnico/cliente.

## Pendências (não deu nesta fase)

- **Criar a keystore de release e os secrets.** Sem isso, cada execução do CI assina com uma chave de debug diferente e o Android **não atualiza por cima** (precisa desinstalar a versão anterior).
- **Perfis técnico/cliente** e **botões personalizáveis** (portão, relé, webhook com PIN/biometria e log): o programa local ainda não tem API de perfis nem de botões; o app só mostra o que o painel oferece.
- **Pareamento com token**: o QR atual da dashboard só tem o endereço Tailscale (sem token), e não existe QR para o endereço da rede local; o programa local não tem autenticação. O app aceita o que vier no QR, então um token entra sem mudar o app quando existir.
- **Alertas com push real** (sem consulta a cada 10 s) e **alertas pelo site**: hoje só pareado com o programa local.
- ~~Núcleo Rust dentro do APK~~: feito na fase 6b (abaixo).
- **Telas nativas de alertas com histórico, módulos e hardware**: cobertas pelo painel no WebView.
- Testado só por compilação e `apksigner verify`; falta teste em aparelho (celular, TV Box com controle remoto/D-pad).

---

# Fase 6b · Servidor no próprio Android

Branch `fase6b-android-servidor`. Decisão em [ADR 0011](adr/0011-servidor-no-android.md).

## Entregue

- **Tela inicial** (`InicioActivity`, novo lançador, focável pelo controle da TV Box): "Este aparelho é o servidor (ligar câmeras aqui)" ou "Ver um servidor à distância". A escolha fica salva; "Trocar modo" no menu do painel e na tela do servidor volta à escolha. Quem já usava o app (com endereço pareado) segue no modo à distância sem perguntar.
- **Servidor no aparelho** (`ServidorService`): roda o núcleo Rust (`libcamfacil.so`) como processo filho, em primeiro plano com notificação fixa ("Desligar"), segura CPU e Wi-Fi acordados, lê a porta real do núcleo e religa com espera crescente se ele cair. Dados em `files/camfacil` (privado do app). Opções: **Iniciar com o aparelho** (BootReceiver; também religa após atualizar o APK) e **Aceitar acesso pela rede Wi-Fi da loja** (`--lan`; desligado por padrão).
- **Tela do servidor** (`ServidorActivity`): estado (ligando / ligado em 127.0.0.1:porta / erro com as últimas linhas do log), **Abrir o painel** (abre sozinho quando fica pronto, vindo da tela inicial), seção **Acessar de fora** com o endereço Tailscale (via `/api/tailscale`) e o passo a passo para o outro celular, endereço na rede da loja quando `--lan`, exceção de bateria.
- **Painel local**: WebView em `http://127.0.0.1:<porta>`; alertas no próprio aparelho funcionam como antes (vigia lendo o servidor local). Menu ganha "Servidor deste aparelho".
- **Vídeo**: ffmpeg 7.1.2 mínimo, LGPL, compilado pelo `scripts/android-ffmpeg.sh` (RTSP/RTP/HTTP, H.264/H.265/MJPEG, fps+scale, MJPEG no stdout, lavfi da demo) e empacotado como `libffmpeg.so`. O núcleo o acha por `CAMFACIL_FFMPEG`, sem mudar o código de vídeo.
- **Rust (só `cfg(target_os = "android")`)**: HTTPS com as autoridades do sistema Android (`cf_notify::tls`), IP Tailscale pela rota (o app oficial não tem o comando `tailscale`), início automático e mensagens de ffmpeg/Tailscale próprias do Android. Windows/Linux sem mudança de comportamento.
- **Scripts**: `scripts/android-nucleo.sh` (cargo-ndk → `jniLibs/<abi>/libcamfacil.so`, e o ffmpeg se faltar). `jniLibs/` fica fora do git.
- **CI**: job `android` instala `aarch64-linux-android` e `armv7-linux-androideabi`, `cargo-ndk`, compila núcleo e ffmpeg (ffmpeg em cache pela hash do script) com o NDK do runner, e só então o Gradle. A release traz o APK completo.

## Tamanho

| | arm64-v8a | armeabi-v7a |
|---|---|---|
| `libcamfacil.so` | 4,3 MB (2,5 MB no APK) | 3,3 MB (2,0 MB no APK) |
| `libffmpeg.so` | 3,0 MB (1,5 MB no APK) | 2,8 MB (1,3 MB no APK) |

**APK de release: 7,1 MB** (era 136 KB). Justificativa na ADR 0011: é o servidor inteiro, offline, sem download depois; ainda abaixo da meta de 10 MB. Se precisar, `splits.abi` gera um APK por arquitetura (~4 MB cada).

## Build local

```sh
export ANDROID_HOME=~/android-sdk ANDROID_NDK_HOME=$ANDROID_HOME/ndk/29.0.14206865
rustup target add aarch64-linux-android armv7-linux-androideabi
cargo install cargo-ndk
scripts/android-nucleo.sh            # núcleo + ffmpeg em android/app/src/main/jniLibs/
cd android && ./gradlew assembleRelease
# saída: android/app/build/outputs/apk/release/app-release.apk (~7,1 MB)
```

Sem o passo `android-nucleo.sh` o APK sai só visualizador (~136 KB) e a tela do servidor avisa que o núcleo não veio.

## Pendências

- **Teste em aparelho**: não há emulador nesta máquina (sem KVM). Verificado por compilação (Rust para as duas ABIs sem avisos no clippy, Gradle `assembleRelease`, `apksigner verify`) e pela mesma configuração do ffmpeg mínimo compilada para Linux: gera MJPEG da cena demo (lavfi), de arquivo H.264 e H.265 e de RTSP H.264 (o teste pegou a falta do decoder `wrapped_avframe`, já incluído). Falta rodar numa TV Box/celular: subida do núcleo, RTSP real, Tailscale, boot.
- **Nível médio da IA (YOLO/ONNX) no Android**: o núcleo baixa o ONNX Runtime oficial, que não tem pacote Android nesse formato; só o nível leve (linha/zona) roda no aparelho.
- **Webcam do próprio aparelho** como câmera: não suportada (seria Camera2/MediaCodec no Kotlin).
- **Phantom process killer** (Android 12+): pode matar núcleo/ffmpeg em aparelhos com muitos processos; o app religa, mas em TV Box dedicada vale desligar a restrição (ver ADR 0011).
- **x86_64** (emulador/TV Box Intel): o script compila (`scripts/android-nucleo.sh x86_64`), mas não vai no APK de release.
- **Painel em `--lan` sem autenticação**: igual ao PC; por isso a rede da loja vem desligada por padrão.
- Continua valendo a pendência da **keystore de release** (fase 6).
