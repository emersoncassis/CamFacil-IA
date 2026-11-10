# SDD-04 — Integrações (Telegram, WhatsApp, webhook, LLM, MCP)

Status: Telegram **implementado** (fase 3); WhatsApp (Evolution API) **implementado** (fase 3b); webhook **implementado** (fase 3b); MCP **implementado** (fase 6) e por HTTPS (ADR 0015); arquivo de alertas **em parte**; clipes, descrição por IA e provedores LLM **não implementados** · revisado em 2026-11-10

## Telegram (padrão) — status: implementado
- Bot API oficial via HTTPS (long polling `getUpdates` com timeout de 30 s, só enquanto há token; sem porta aberta). Cliente próprio com `reqwest` (rustls, provedor `ring` já usado pelo binário; sem `aws-lc`), multipart do `sendPhoto` montado à mão. Crate `cf-notify`.
- **Token cifrado** (regra 7): ChaCha20-Poly1305 (`chacha20poly1305`, puro Rust, ~60 KB) com chave de 32 bytes em `data_dir/segredo.key` (modo 0600 no Unix). No SQLite fica `enc:<base64(nonce||cifra)>`.
- **Vínculo**: o contato manda `/start` ao bot; ele aparece em Integrações → Telegram → "Contatos autorizados" e o técnico liga a chave. Só autorizados recebem alertas e usam comandos. (O link `t.me/<bot>?start=<token>` fica para depois; a aprovação manual na dashboard é mais simples de explicar.)
- **Alertas**: linha cruzada, zona ocupada e mudança de saúde (escura/coberta/desfocada/sem sinal), com a foto do instante (`canal.ultimo`, JPEG do stream secundário) e o texto `HH:MM — <regra>: passagem (entrada) · <câmera>`. Antirrepique por regra: 30 s (`regra:<id>`, `saude:<camera_id>`). Foto opcional ("Mandar foto junto").
- **Offline**: fila em memória de 40 alertas; o mais velho sai; reenvio com espera crescente (10 s → 2 min) quando a internet volta. Sem token ou sem contato autorizado, a fila é descartada.
- **Comandos**: `/status`, `/foto <nome ou nº>`, `/resumo` (alertas de hoje no fuso do técnico, informado pela dashboard em `settings.fuso_min`), `/liga <módulo>`, `/desliga <módulo>` (registra evento `comando`), `/backup <senha>` (só no Telegram: manda o backup cifrado e tenta apagar a mensagem com a senha), `/ajuda`. O relatório de validação em PDF também pode ser enviado pelo Telegram (botão na tela de relatório).
- **API local**: `GET/POST/DELETE /api/telegram`, `POST /api/telegram/testar`, `POST /api/telegram/fotos`, `POST/DELETE /api/telegram/chats/{id}`, `POST /api/fuso`.
- Testado contra um servidor Bot API simulado (`CAMFACIL_TELEGRAM_API`); falta validar com um bot real (@BotFather) numa máquina com internet.

## WhatsApp (opcional) — status: implementado via Evolution API (fase 3b)
- **Gateway HTTP, não biblioteca** (ADR-0010, substitui a ADR-0004): a [Evolution API](https://github.com/EvolutionAPI/evolution-api) roda como contêiner Docker fora do binário (no mesmo servidor do painel ou em qualquer PC com Docker); o CamFácil IA só fala HTTP com ela (`crates/cf-notify/src/whatsapp.rs`, cliente mínimo com o `reqwest` já existente, nenhuma dependência nova). Mantida a regra 2: nada de Node no caminho de execução **do programa**.
- **Configuração**: URL da Evolution, nome da instância e chave (`AUTHENTICATION_API_KEY`), esta cifrada pelo mesmo cofre ChaCha20-Poly1305 do token do Telegram (`settings.wa_apikey = enc:…`). "Salvar e testar" chama `GET /instance/fetchInstances`; "Gerar QR" chama `GET /instance/connect/{instância}` (cria a instância Baileys com `POST /instance/create` se faltar) e mostra o PNG em base64 na tela.
- **Aviso permanente na tela** (PRD §5): *"WhatsApp não oficial pode ter o número bloqueado; use um chip dedicado, nunca o seu número principal."*
- **Contatos**: quem manda "oi" para o número aparece em Integrações → WhatsApp → Números autorizados e o técnico liga a chave (tabela `whatsapp_contatos`, igual ao Telegram). Só autorizados recebem alertas e usam comandos.
- **Alertas**: mesmos textos e fotos do Telegram (`POST /message/sendText` e `sendMedia` com JPEG em base64), antirrepique de 30 s por regra, fila offline de 40 com espera crescente.
- **Comandos** (com ou sem barra): `status`, `foto <nome ou nº>`, `resumo`, `liga/desliga <módulo>`, `ajuda` (`crates/cf-notify/src/comandos.rs`). Chegam pelo **webhook** da Evolution: `POST /api/whatsapp/webhook/{token}` (token de 128 bits gerado uma vez, guardado cifrado; sem ele a rota responde 404). O botão "Registrar na Evolution" chama `POST /webhook/set/{instância}` com `MESSAGES_UPSERT`. A Evolution precisa alcançar o PC do CamFácil IA (mesma rede com `--lan`, ou Tailscale); o técnico pode informar o endereço que ela enxerga. Polling não foi implementado: a Evolution não oferece um `getUpdates` leve (o `findMessages` consulta o banco inteiro), e o webhook na mesma rede é o caminho normal.
- **API local**: `GET/POST/DELETE /api/whatsapp`, `POST /api/whatsapp/testar`, `POST /api/whatsapp/qr`, `POST /api/whatsapp/fotos`, `POST /api/whatsapp/webhook/registrar`, `POST /api/whatsapp/webhook/{token}`, `POST/DELETE /api/whatsapp/contatos/{jid}`.
- **Subir a Evolution API no mesmo servidor Docker do painel** (uma linha, PostgreSQL embutido não é necessário para começar):
  ```sh
  docker run -d --name evolution -p 8080:8080 -e AUTHENTICATION_API_KEY=troque-esta-chave -v evolution_instances:/evolution/instances atendai/evolution-api:v2.2.3
  ```
  Depois aponte a dashboard para `http://<ip-do-servidor>:8080` com essa chave. Para produção, use o `docker-compose` oficial da Evolution (com Postgres e Redis) e HTTPS pelo mesmo proxy do painel.
- Testado contra um servidor HTTP simulado (testes em `whatsapp.rs`); falta validar contra uma Evolution real e um chip dedicado.

## Comandos v1 (iguais nos dois canais, exceto `backup`, só no Telegram)
| Comando | Ação |
|---|---|
| `foto <câmera>` | snapshot atual |
| `status` | módulos ativos e saúde da máquina |
| `liga <módulo>` / `desliga <módulo>` | ativa/pausa |
| `resumo` | alertas do dia |
| `backup` | envia backup criptografado |

Só contatos autorizados na dashboard. Ações registradas no log.

## Formato do alerta (por regra)
- **Texto simples (padrão), implementado**: `22:14 — Cerca Portão: passou carro — entrada · Câmera 3`. Desde a 0.9.2 o texto diz a classe e o que aconteceu (`… : carro parado há 65 s · …`, `… : 3 carros ao mesmo tempo · …`).
- **Foto do instante, implementada** (opcional, "Mandar foto junto"; JPEG cru do stream secundário, sem caixas).
- **Clipe (~10 s) e descrição por IA (provedor LLM ou modelo local): não implementados.**
- **Alertas da própria câmera** (módulo `camera`, SDD-09) também seguem para o Telegram como "recebido da câmera".

## Telegram como arquivo de alertas — status: implementado em parte (fase 3b)
- **Implementado** (`crates/camfacil/src/arquivo.rs`): desligado por padrão, só liga com o termo do técnico aceito. Guarda a foto de cada alerta em `fotos/cam_<id>/` com índice na tabela `arquivo_alertas` e retenção por câmera (padrão 200 fotos, as mais velhas saem do disco). Com o ID de um canal privado configurado, o mesmo bot manda a foto para lá e guarda o `message_id` (link `t.me/c/…` na tela). O webhook recebe `foto_url` da foto guardada. API: `GET/POST /api/arquivo`, `GET /api/arquivo/<id>.jpg`.
- **Pendente**: imagem com marcação (contorno da zona/linha), clipes, envio como arquivo sem compressão e apagar mensagens antigas do canal no prazo.
- O bot envia fotos e clipes de cada alerta para um canal ou grupo privado, que funciona como arquivo gratuito e pesquisável por data.
- **Imagens com marcação só aqui**: contorno da zona/linha, destaque do objeto, nome da regra, câmera e horário, nas cores do projeto. Dashboard e quiosque continuam só com balões.
- Limite da Bot API: 50 MB por envio. Opção de enviar como arquivo para não comprimir.
- LGPD: chats de bot não têm criptografia ponta a ponta. Canal privado, modo LGPD quando possível, prazo para apagar mensagens antigas.
- Aceite obrigatório do [termo de responsabilidade do técnico](../legal/TERMO-RESPONSABILIDADE-TECNICO.md) antes de ativar (vale também para WhatsApp e provedores de IA).

## Termos e LGPD no programa — status: implementado (fase 3b)
- Integrações → Termos e LGPD (`crates/camfacil/src/termos.rs`). Sem o aceite do termo do técnico (versão gravada em `acceptances`), `POST /api/telegram`, `/api/whatsapp`, `/api/webhook` e ligar o arquivo respondem 412.
- Autorização do cliente: dados gravados localmente; `GET /api/termos/cliente.md` devolve o termo preenchido (canais ligados marcados) para imprimir e assinar; `GET /api/termos/placa.md`, a placa de ambiente monitorado.
- "Apagar dados deste cliente" (`POST /api/termos/apagar`, exige digitar `APAGAR`): eventos, índice do arquivo e fotos; câmeras, regras e configurações ficam. Cada aceite e limpeza vira evento `lgpd` no log.

## Webhook — status: implementado (fase 3b)
- `POST` com JSON por alerta para a URL configurada em Integrações → Webhook (`crates/cf-notify/src/webhook.rs`). Campos: `programa`, `versao`, `tipo` (`regra`, `permanencia`, `saude`, `teste`), `camera_id`, `camera`, `regra_id`, `regra`, `quando` (unix), `hora` (local), `texto` (o mesmo do Telegram) e `foto_url` (foto do instante guardada neste PC, só alcançável na rede local).
- Cabeçalho opcional `Nome: valor` (ex.: `Authorization: Bearer …`) guardado **cifrado** pelo cofre; é a autenticação do lado de lá. A assinatura HMAC prevista antes ficou de fora: Home Assistant, n8n e Node-RED já validam por cabeçalho ou token na URL, e menos código é menos risco.
- Retentativa 3× (1 s, 3 s) por evento, antirrepique de 30 s por regra, fila em memória de 64; falhas e último erro aparecem na tela. Botão "Enviar teste" manda um evento `tipo: teste` na hora e mostra o status HTTP devolvido.
- API local: `GET/POST/DELETE /api/webhook`, `POST /api/webhook/testar`.

## Provedores LLM (proposto, nada implementado)
Não há interface `LlmProvider` nem chamada a OpenRouter/OpenAI/Google/Anthropic no programa local. A conversa com IA existe hoje só pelo **MCP** (o assistente do técnico, como Claude, fala com o programa) e está prevista no modo estúdio (SDD-07, pendente).

OpenRouter (modelos gratuitos, padrão), OpenAI, Google, Anthropic. Interface única `LlmProvider`. Aviso: gratuitos sem garantia e podem usar dados para treino — não enviar imagens do cliente por eles sem consentimento.

## Servidor MCP (opcional) — implementado (ADR 0012)
- **Desligado por padrão.** Integrações → MCP: chave "Ligar o MCP" (aviso + confirmação, exige o termo do técnico) e chave "Permitir alterações" (escrita), também desligada por padrão.
- Transportes: `POST /mcp` (JSON-RPC 2.0, JSON simples, sem SSE) no servidor da dashboard, com `Authorization: Bearer <token>` obrigatório; e `camfacil --mcp`, ponte stdio para o programa aberto na mesma máquina (lê o token do banco local).
- Token aleatório guardado cifrado pelo cofre; "Gerar outro token" invalida o anterior. Chaves `mcp_*` não viajam no backup.
- Ferramentas de leitura: `listar_cameras` (sem senha/RTSP), `ver_estado`, `listar_regras`, `ver_alertas`, `listar_botoes`.
- Ferramentas de escrita (só com "Permitir alterações"): `criar_regra`, `remover_regra`, `ligar_modulo`. Cada uma gera evento `mcp`.
- Portão e relé **não** são acionáveis pelo MCP (ação física exige PIN + confirmação de uma pessoa; SDD-06).
- JSON-RPC implementado à mão (sem `rmcp`) para não trazer dependências; ver ADR 0012.
- API da dashboard: `GET/POST /api/mcp` (`{ligado, escrita}`), `POST /api/mcp/token`.

### MCP por HTTPS (ADR 0015) — implementado
- **Por quê:** os conectores remotos do Claude (claude.ai e app) só aceitam HTTPS com certificado válido e chamam da nuvem.
- **Integrações → MCP → "Endereço HTTPS para o Claude"** (seção aberta por padrão, sem rolar a página):
  - **Tailscale Serve** (só a rede Tailscale): `tailscale serve --bg --https=<porta> --set-path=/mcp http://127.0.0.1:<porta local>/mcp` → `https://<máquina>.<tailnet>.ts.net/mcp`.
  - **Tailscale Funnel** (internet, o que o claude.ai precisa): mesma coisa com `tailscale funnel`, após o diálogo "Isto publica o MCP na internet. Só quem tiver o token consegue usar. Continuar?"; exige MCP ligado e token; evento `mcp`.
  - **ngrok (só testes)**: túnel só do `/mcp` (SDD-05).
  - Mostra, prontos para copiar: a **URL para o claude.ai** (`…/mcp?token=…`), a URL HTTPS e o cabeçalho `Authorization: Bearer …`; o comando equivalente; e o passo a passo quando falta HTTPS (painel → DNS → HTTPS Certificates) ou Funnel (política → atributo `funnel`). "Desligar endereço HTTPS" tira só o `/mcp`; desligar o MCP também.
  - Porta escolhida entre 443, 8443 e 10000 sem mexer no que o usuário já publica (Funnel vale para a porta inteira). Cliente Tailscale 1.52+.
  - Android: sem Serve/Funnel (o app Tailscale não tem CLI); mensagem clara.
- **Token na URL** só vale por proxy HTTPS local (conexão de loopback com `X-Forwarded-For`); por HTTP puro → 401. OAuth fica pendente.
- **Claude Desktop / Claude Code** no mesmo computador: stdio (`camfacil --mcp`), sem HTTPS; a tela mostra o JSON do `claude_desktop_config.json` e `claude mcp add camfacil -- <caminho> --mcp`.
- `/mcp/` (com barra) também atende. API: `GET /api/mcp/https` → `{instalado, versao, versao_ok, logado, nome, https_ok, funnel_ok, funnel_portas, publicado{modo,porta,url}, comandos, links, android}`; `POST /api/mcp/https` `{modo: serve|funnel|off, confirmado}`; `GET /api/mcp` ganhou `claude_code`.
- Testes: parser de `status --json`/`serve status --json`/`version`, escolha de porta, comandos montados; servidor real respondendo `initialize`/`tools/list` por HTTP como o proxy entrega (cabeçalho, `?token=`, `/mcp/`, 202, 405, 401 sem proxy); `scripts/telas-mcp-https.py` (Playwright, 5 tamanhos).
