# SDD-04 — Integrações (Telegram, WhatsApp, webhook, LLM, MCP)

Status: Telegram **implementado** (fase 3); WhatsApp, webhook, LLM e MCP propostos (fase 3 e 6)

## Telegram (padrão) — status: implementado
- Bot API oficial via HTTPS (long polling `getUpdates` com timeout de 30 s, só enquanto há token; sem porta aberta). Cliente próprio com `reqwest` (rustls, provedor `ring` já usado pelo binário; sem `aws-lc`), multipart do `sendPhoto` montado à mão. Crate `cf-notify`.
- **Token cifrado** (regra 7): ChaCha20-Poly1305 (`chacha20poly1305`, puro Rust, ~60 KB) com chave de 32 bytes em `data_dir/segredo.key` (modo 0600 no Unix). No SQLite fica `enc:<base64(nonce||cifra)>`.
- **Vínculo**: o contato manda `/start` ao bot; ele aparece em Integrações → Telegram → "Contatos autorizados" e o técnico liga a chave. Só autorizados recebem alertas e usam comandos. (O link `t.me/<bot>?start=<token>` fica para depois; a aprovação manual na dashboard é mais simples de explicar.)
- **Alertas**: linha cruzada, zona ocupada e mudança de saúde (escura/coberta/desfocada/sem sinal), com a foto do instante (`canal.ultimo`, JPEG do stream secundário) e o texto `HH:MM — <regra>: passagem (entrada) · <câmera>`. Antirrepique por regra: 30 s (`regra:<id>`, `saude:<camera_id>`). Foto opcional ("Mandar foto junto").
- **Offline**: fila em memória de 40 alertas; o mais velho sai; reenvio com espera crescente (10 s → 2 min) quando a internet volta. Sem token ou sem contato autorizado, a fila é descartada.
- **Comandos**: `/status`, `/foto <nome ou nº>`, `/resumo` (alertas de hoje no fuso do técnico, informado pela dashboard em `settings.fuso_min`), `/liga <módulo>`, `/desliga <módulo>` (registra evento `comando`), `/ajuda`.
- **API local**: `GET/POST/DELETE /api/telegram`, `POST /api/telegram/testar`, `POST /api/telegram/fotos`, `POST/DELETE /api/telegram/chats/{id}`, `POST /api/fuso`.
- Testado contra um servidor Bot API simulado (`CAMFACIL_TELEGRAM_API`); falta validar com um bot real (@BotFather) numa máquina com internet.

## WhatsApp (opcional)
- Biblioteca não oficial por QR code: `whatsapp-rust` (nativo) ou `whatsmeow` (Go) como processo auxiliar se o port for instável (ADR-0004).
- **Aviso obrigatório na tela de ativação**: biblioteca não oficial; o número pode ser banido; use um chip dedicado.

## Comandos v1 (iguais nos dois canais)
| Comando | Ação |
|---|---|
| `foto <câmera>` | snapshot atual |
| `status` | módulos ativos e saúde da máquina |
| `liga <módulo>` / `desliga <módulo>` | ativa/pausa |
| `resumo` | alertas do dia |
| `backup` | envia backup criptografado |

Só contatos autorizados na dashboard. Ações registradas no log.

## Formato do alerta (por regra)
- Texto simples (padrão): `22:14 — 2 pessoas na Cerca Portão (Câmera 3)`.
- Foto, clipe (~10 s), ou descrição por IA (provedor LLM ou modelo local pequeno).

## Telegram como arquivo de alertas
- O bot envia fotos e clipes de cada alerta para um canal ou grupo privado, que funciona como arquivo gratuito e pesquisável por data.
- **Imagens com marcação só aqui**: contorno da zona/linha, destaque do objeto, nome da regra, câmera e horário, nas cores do projeto. Dashboard e quiosque continuam só com balões.
- Limite da Bot API: 50 MB por envio. Opção de enviar como arquivo para não comprimir.
- LGPD: chats de bot não têm criptografia ponta a ponta. Canal privado, modo LGPD quando possível, prazo para apagar mensagens antigas.
- Aceite obrigatório do [termo de responsabilidade do técnico](../legal/TERMO-RESPONSABILIDADE-TECNICO.md) antes de ativar (vale também para WhatsApp e provedores de IA).

## Webhook
POST JSON por evento para URL configurada, com assinatura HMAC.

## Provedores LLM
OpenRouter (modelos gratuitos, padrão), OpenAI, Google, Anthropic. Interface única `LlmProvider`. Aviso: gratuitos sem garantia e podem usar dados para treino — não enviar imagens do cliente por eles sem consentimento.

## Servidor MCP (opcional)
- Desligado por padrão; escuta só na interface Tailscale; token obrigatório.
- Ferramentas de leitura: `status`, `listar_cameras`, `listar_regras`, `eventos`.
- Ferramentas de escrita (exigem permissão explícita na dashboard): `criar_regra`, `ligar_modulo`, `desligar_modulo`.
- Implementar com o SDK oficial Rust de MCP (`rmcp`).
