# SDD-06 — Módulo TV, app Android e web app

Status: proposto · Fase 5 e 6

## Módulo TV (quiosque)
- Rota `/tv` em tela cheia, sem menus; lê os módulos ativos e monta cartões (ex.: Mesa 1 — vazia).
- Logo do técnico no canto + QR code para o WhatsApp dele.
- Atualização por WebSocket; funciona em navegador de TV Box e Smart TV.
- Reaproveitar o módulo TV de outra aplicação do autor quando possível.

## App Android
- Tecnologia: avaliar Tauri 2 mobile (reuso do front e de Rust) vs. Kotlin nativo (ADR futura). Prioridade: APK pequeno.
- Pareamento: QR code na dashboard (endereço Tailscale + token).
- Perfis: técnico (tudo) e cliente (ver, alertas, botões liberados).
- Telas: câmeras ao vivo (stream secundário), alertas com foto, módulos, hardware, botões.
- **Botões personalizáveis**: o técnico define nome, ícone, posição e ação (HTTP para controlador de acesso, relé de rede, webhook). O comando é executado pelo servidor local; PIN/biometria em ações críticas; log de quem acionou.
- **Conexão fraca**: mede atraso, perda de quadros e banda; após alguns segundos ruins troca para snapshot; volta só quando estabiliza; botão manual.

## Web app (instalação progressiva)
1. Navegador puro: núcleo de IA compilado para WASM + ONNX Runtime Web (WebGPU/WASM) — webcam, arquivo, demo.
2. Gateway: binário mínimo (RTSP → WebRTC, busca ONVIF), base go2rtc ou implementação própria.
3. Instalação completa.
O web app mostra o botão para subir de degrau. Limitações do navegador puro documentadas na tela.

## Marca do técnico
Logo (PNG/SVG até 200 KB) e número de WhatsApp na tela Integrações; aparecem na dashboard, no quiosque, no relatório e na proposta.
