# SDD-04 — Integrações (Telegram, WhatsApp, webhook, LLM, MCP)

Status: proposto · Fase 3 e 6

## Telegram (padrão)
- Bot API oficial via HTTPS (long polling, sem porta aberta). Crate leve (`teloxide` ou cliente próprio com `reqwest`).
- Vínculo: a dashboard mostra QR code/link `t.me/<bot>?start=<token>`; o primeiro `/start` com o token autoriza o chat.

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
