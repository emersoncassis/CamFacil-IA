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

## Pendências (não deu nesta fase)

- **Criar a keystore de release e os secrets.** Sem isso, cada execução do CI assina com uma chave de debug diferente e o Android **não atualiza por cima** (precisa desinstalar a versão anterior).
- **Perfis técnico/cliente** e **botões personalizáveis** (portão, relé, webhook com PIN/biometria e log): o programa local ainda não tem API de perfis nem de botões; o app só mostra o que o painel oferece.
- **Pareamento com token**: o QR atual da dashboard só tem o endereço Tailscale (sem token), e não existe QR para o endereço da rede local; o programa local não tem autenticação. O app aceita o que vier no QR, então um token entra sem mudar o app quando existir.
- **Alertas com push real** (sem consulta a cada 10 s) e **alertas pelo site**: hoje só pareado com o programa local.
- **Núcleo Rust dentro do APK** (`aarch64-linux-android`, TV Box Android como servidor, início no boot): fora desta fase.
- **Telas nativas de alertas com histórico, módulos e hardware**: cobertas pelo painel no WebView.
- Testado só por compilação e `apksigner verify`; falta teste em aparelho (celular, TV Box com controle remoto/D-pad).
