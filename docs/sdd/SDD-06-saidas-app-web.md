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
- **Botões de portão e relé — implementado (fase 6):**
  - Cadastro em Integrações → Portão e relé: nome, ícone (portão, relé, luz), URL `http(s)://`, método (GET/POST/PUT), usuário/senha opcionais (HTTP Basic) e corpo opcional. A ação inteira fica **cifrada** pelo cofre (`botao_acao_<id>`); a lista só mostra o endereço sem usuário, senha nem parâmetros. Máximo de 12 botões.
  - **PIN** de 4 a 8 números (PBKDF2-SHA256, 60 mil iterações, sal próprio; nunca em claro). 5 erros travam os botões por 5 min.
  - **Confirmação**: `POST /api/botoes/{id}/preparar` (PIN + nome de quem aciona) devolve um código de uso único válido por 20 s; só `POST /api/botoes/{id}/acionar` com esse código executa a chamada (timeout 8 s), feita pelo servidor local.
  - **Log**: eventos `acionamento` (botão · quem · de onde [painel/app, IP] · resultado) e `acionamento_negado` (PIN errado); `GET /api/botoes/log`. Cadastro, remoção e troca de PIN viram `botao_config`.
  - Tela para o celular: `GET /botoes` (HTML/CSS/JS puro embutido). O app Android abre essa rota no WebView pelo menu "Portão e relé" (só no modo local); nada nativo.
  - O MCP só lista botões, nunca aciona.
- **Conexão fraca**: mede atraso, perda de quadros e banda; após alguns segundos ruins troca para snapshot; volta só quando estabiliza; botão manual.

## Web app (instalação progressiva)
1. Navegador puro: núcleo de IA compilado para WASM + ONNX Runtime Web (WebGPU/WASM) — webcam, arquivo, demo.
2. **Agente-Cam-Elo-IA**: binário mínimo de ponte com a rede local (RTSP → WebRTC, busca ONVIF), base go2rtc ou implementação própria. Nome de serviço: `agente-cam-elo-ia`.
3. Instalação completa.
Hospedagem, login e isolamento entre técnicos: [SDD-08](SDD-08-contas-e-isolamento.md), em `https://cfia.ysafe.com.br`.
O web app mostra o botão para subir de degrau. Limitações do navegador puro documentadas na tela.

## Marca do técnico
Logo (PNG/SVG até 200 KB) e número de WhatsApp na tela Integrações; aparecem na dashboard, no quiosque, no relatório e na proposta.

## Dispositivos
Tabela de papéis no [PRD §5.8](../PRD.md). Resumo técnico:
- **APK Android** (TV Box, Android TV, celular velho): núcleo Rust compilado para `aarch64-linux-android`, serviço em primeiro plano, início no boot, exceção de bateria.
- **Armbian** em TV Box: mesmo binário Linux ARM64 do Raspberry.
- **Smart TV Tizen/webOS**: só o quiosque `/tv` pelo navegador ou app web empacotado.
