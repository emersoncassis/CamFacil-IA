# ADR 0012 · MCP com JSON-RPC próprio, desligado por padrão

**Situação:** aceita · 2026-11-07 · substitui a nota "implementar com `rmcp`" do [SDD-04](../sdd/SDD-04-integracoes.md)

## Contexto

A fase 6 pede um servidor MCP para o técnico configurar o CamFácil IA conversando com uma IA (Claude Desktop, Cursor…). O PRD exige: desligado por padrão, token, leitura por padrão. A regra 7 do CLAUDE.md exige que ação física (portão/relé) tenha confirmação e log. A regra 1 limita binário e RAM.

O SDK oficial em Rust (`rmcp`) traz `schemars`, macros procedurais, `tokio-util`/SSE e transportes que não usamos. O que o programa precisa é pequeno: `initialize`, `ping`, `tools/list` e `tools/call`, com respostas JSON.

## Decisão

1. **JSON-RPC 2.0 à mão** em `crates/camfacil/src/mcp.rs` (~400 linhas com testes), só com `serde_json` e `axum`, que já estão no binário. Nenhuma dependência nova.
2. **Dois transportes, um só núcleo:**
   - HTTP: `POST /mcp` no mesmo servidor da dashboard (resposta JSON simples, sem SSE; `GET /mcp` → 405). Notificações → 202. Lotes recusados (como na revisão 2025-06-18). `Origin` de fora de localhost → 403 (DNS rebinding).
   - stdio: `camfacil --mcp` é uma **ponte** que lê uma mensagem por linha e repassa ao programa já aberto (`127.0.0.1:<porta>/mcp`), lendo o token do banco local. Assim o Claude Desktop usa `{"command": "<caminho>/camfacil", "args": ["--mcp"]}` sem copiar token, e só existe um dono do vídeo e do banco.
3. **Desligado por padrão.** Integrações → MCP liga com aviso e confirmação; ligar exige o termo do técnico aceito. Ligado sem token, o token (24 bytes aleatórios) é criado e guardado **cifrado** pelo cofre. "Gerar outro token" invalida o anterior.
4. **Leitura por padrão.** `criar_regra`, `remover_regra` e `ligar_modulo` só funcionam com a segunda chave "Permitir alterações"; sem ela a ferramenta devolve `isError` explicando como liberar. Desligar o MCP desliga também a escrita. Toda alteração e toda mudança de configuração vira evento `mcp`.
5. **Ação física fora do MCP.** Há só `listar_botoes`; acionar portão/relé exige PIN + confirmação de uma pessoa na dashboard ou no app (`/botoes`).
6. As chaves `mcp_*` são **desta máquina** (`chave_local`): restaurar um backup não liga o MCP noutra máquina.

## Consequências

- Binário: + ~40 KB (estimativa pelo tamanho do código; sem crate nova). RAM ociosa: zero com o MCP desligado (só uma leitura de configuração por requisição em `/mcp`).
- Protocolo acompanhado à mão: versões aceitas `2025-06-18`, `2025-03-26`, `2024-11-05`; recursos (`resources`, `prompts`, `sampling`) não implementados. Se um dia precisarmos de streaming/SSE ou OAuth, reavaliar o `rmcp`.
- O MCP escuta onde a dashboard escuta (localhost; rede da loja com `--lan`; Tailscale). O token é obrigatório em todos os casos.
- HTTPS para os conectores do claude.ai (Tailscale Serve/Funnel, token na URL, ngrok): ver [ADR 0015](0015-mcp-https-tailscale-e-ngrok.md).
