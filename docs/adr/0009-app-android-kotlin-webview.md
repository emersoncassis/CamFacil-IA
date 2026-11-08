# ADR 0009 · App Android em Kotlin com WebView

**Situação:** aceita · 2026-11-07

## Contexto

O SDD-06 deixou em aberto Tauri 2 mobile vs. Kotlin nativo, com prioridade para APK pequeno. O painel já existe em HTML + CSS + JS puro (programa local e site); o que falta no celular é o que o navegador não faz bem: ler QR, avisar com notificação em segundo plano e cair para foto quando a rede piora.

## Decisão

- App em **Kotlin, sem AndroidX** (só `Activity`, `WebView` e `Notification` da plataforma) em `android/`. Única dependência: `com.google.zxing:core` para o QR. R8 e encolhimento de recursos ligados.
- O painel abre num **WebView** (programa local pareado ou `https://cfia.ysafe.com.br`). O nativo cuida de: pareamento (QR/endereço/Tailscale), vigia de alertas (serviço em primeiro plano lendo `/api/eventos`), câmeras ao vivo com troca automática para foto.
- Leitor de QR com a API Camera 1 + ZXing, sem CameraX nem Play Services (muita TV Box não tem).
- Versão do app = versão do `Cargo.toml`. O APK sai no mesmo job de release do GitHub (`camfacil-android.apk`).
- Tauri fica descartado por ora: puxaria o runtime Rust + WebView wrapper e passaria fácil de 10 MB, sem ganho para um app que só exibe o painel.

## Consequências

- APK de ~140 KB (meta era < 10 MB), minSdk 24 (Android 7).
- O núcleo Rust **não** roda dentro do APK ainda (`aarch64-linux-android`, ver SDD-06 · Dispositivos); fica para uma ADR futura se a TV Box Android virar servidor. **Atualização:** resolvido na [ADR 0010](0010-servidor-no-android.md) (núcleo + ffmpeg mínimo no APK, ~7 MB).
- Alertas por consulta a cada 10 s (sem push): simples e offline-first, mas gastam um pouco de bateria; o app pede a exceção de bateria pelo menu.
