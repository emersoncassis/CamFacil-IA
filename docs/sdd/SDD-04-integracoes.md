# SDD-04 — Integrações (Telegram, WhatsApp, webhook, LLM, MCP)

Status: Telegram **implementado** (fase 3); WhatsApp (Evolution API) **implementado** (fase 3b); webhook **implementado** (fase 3b); LLM e MCP propostos (fase 6)

## Telegram (padrão) — status: implementado
- Bot API oficial via HTTPS (long polling `getUpdates` com timeout de 30 s, só enquanto há token; sem porta aberta). Cliente próprio com `reqwest` (rustls, provedor `ring` já usado pelo binário; sem `aws-lc`), multipart do `sendPhoto` montado à mão. Crate `cf-notify`.
- **Token cifrado** (regra 7): ChaCha20-Poly1305 (`chacha20poly1305`, puro Rust, ~60 KB) com chave de 32 bytes em `data_dir/segredo.key` (modo 0600 no Unix). No SQLite fica `enc:<base64(nonce||cifra)>`.
- **Vínculo**: o contato manda `/start` ao bot; ele aparece em Integrações → Telegram → "Contatos autorizados" e o técnico liga a chave. Só autorizados recebem alertas e usam comandos. (O link `t.me/<bot>?start=<token>` fica para depois; a aprovação manual na dashboard é mais simples de explicar.)
- **Alertas**: linha cruzada, zona ocupada e mudança de saúde (escura/coberta/desfocada/sem sinal), com a foto do instante (`canal.ultimo`, JPEG do stream secundário) e o texto `HH:MM — <regra>: passagem (entrada) · <câmera>`. Antirrepique por regra: 30 s (`regra:<id>`, `saude:<camera_id>`). Foto opcional ("Mandar foto junto").
- **Offline**: fila em memória de 40 alertas; o mais velho sai; reenvio com espera crescente (10 s → 2 min) quando a internet volta. Sem token ou sem contato autorizado, a fila é descartada.
- **Comandos**: `/status`, `/foto <nome ou nº>`, `/resumo` (alertas de hoje no fuso do técnico, informado pela dashboard em `settings.fuso_min`), `/liga <módulo>`, `/desliga <módulo>` (registra evento `comando`), `/ajuda`.
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

## Provedores LLM
OpenRouter (modelos gratuitos, padrão), OpenAI, Google, Anthropic. Interface única `LlmProvider`. Aviso: gratuitos sem garantia e podem usar dados para treino — não enviar imagens do cliente por eles sem consentimento.

## Servidor MCP (opcional)
- Desligado por padrão; escuta só na interface Tailscale; token obrigatório.
- Ferramentas de leitura: `status`, `listar_cameras`, `listar_regras`, `eventos`.
- Ferramentas de escrita (exigem permissão explícita na dashboard): `criar_regra`, `ligar_modulo`, `desligar_modulo`.
- Implementar com o SDK oficial Rust de MCP (`rmcp`).
