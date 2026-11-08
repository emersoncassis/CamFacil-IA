# ADR 0015 · MCP por HTTPS (Tailscale Serve/Funnel), token na URL e ngrok temporário

**Situação:** aceita · 2026-11-08 · complementa a [ADR 0012](0012-mcp-json-rpc-proprio.md) e a [ADR 0005](0005-vpn-embutida-e-portas.md)

## Contexto

Os conectores remotos do Claude (claude.ai e app) só aceitam MCP por **HTTPS com certificado válido**: nada de HTTP nem certificado autoassinado. Eles chamam **a partir da nuvem da Anthropic**, então o endereço precisa ser alcançável da internet. E o formulário de conector personalizado só pede a URL (e, opcionalmente, OAuth); **não** deixa pôr cabeçalho `Authorization`.

Ao mesmo tempo o CamFácil IA é offline-first (regra 3), leve (regra 1) e o MCP fica desligado por padrão, com token (regra 7). O dono do produto também pediu uma saída rápida para testes: o ngrok.

## Decisão

1. **HTTPS pelo Tailscale, sem certificado nosso.** O programa roda o cliente oficial já instalado:
   - **Serve** (só a rede Tailscale): `tailscale serve --bg --https=<porta> --set-path=/mcp http://127.0.0.1:<porta local>/mcp`.
   - **Funnel** (internet; o que o claude.ai precisa): `tailscale funnel --bg --https=<porta> --set-path=/mcp …`, com **confirmação forte** no diálogo da página, MCP ligado, token obrigatório e evento `mcp` registrado.
   - Desligar tira **só** o `/mcp` (`… --https=<porta> --set-path=/mcp off`). Desligar o MCP também tira o `/mcp` do Tailscale.
   - **Não mexe no que o usuário já tem.** A liberação do Funnel vale para a porta inteira; por isso o Funnel do `/mcp` nunca vai para uma porta (443, 8443, 10000) que já tenha outros caminhos, e o Serve não usa porta com Funnel. Lido de `tailscale serve status --json`.
   - Capacidades lidas de `tailscale status --json`: `Self.DNSName` (nome), `Self.CertDomains` (HTTPS ligado), `funnel` em `Self.CapMap`/`Self.Capabilities` e as portas de `…/cap/funnel-ports?ports=`. Faltando algo, a tela mostra o passo a passo com o link do painel (DNS → HTTPS Certificates; política de acesso → atributo `funnel`).
   - Sintaxe atual exige cliente **1.52+** (`tailscale version`); mais antigo → mensagem para atualizar. No Linux, "Access denied" vira a dica `sudo tailscale set --operator=$USER`.
   - O proxy entrega em `127.0.0.1` (o mount `/mcp` é tirado e somado ao caminho do alvo, que já é `/mcp`); o servidor aceita também `/mcp/`. O transporte segue o "Streamable HTTP" só com respostas JSON: `POST` JSON-RPC, notificação → 202, `GET` → 405, sem `Mcp-Session-Id` (opcional na especificação).
2. **Token na URL (`/mcp?token=…`) só por proxy HTTPS local.** Como o conector do claude.ai não manda cabeçalho, o token também vale na query, mas **só** quando a conexão vem de loopback (o Tailscale e o ngrok conectam em 127.0.0.1) **e** traz `X-Forwarded-For` (marca de proxy). Pela LAN ou pelo IP Tailscale direto, em HTTP puro, a URL com token é recusada (401 com `WWW-Authenticate`), para o token não trafegar aberto. `Authorization: Bearer` continua valendo e é o preferido.
   - **Risco aceito:** a URL leva o segredo; fica guardada no claude.ai e pode aparecer em print ou histórico. O TLS termina na própria máquina (o Funnel repassa TCP cifrado), então nem o Tailscale vê o token. A tela avisa para tratar a URL como senha e "Gerar outro token" a invalida na hora.
   - **OAuth fica pendente:** exigiria servidor de autorização (descoberta `/.well-known`, registro dinâmico de cliente, PKCE). Reavaliar quando houver contas (SDD-08) ou se o claude.ai deixar de aceitar URL sem OAuth.
3. **Claude Desktop e Claude Code no mesmo computador usam stdio** (`camfacil --mcp`, ADR 0012), que não precisa de HTTPS. A tela mostra o JSON do `claude_desktop_config.json` e `claude mcp add camfacil -- <caminho> --mcp`.
4. **Android:** o app oficial do Tailscale não tem CLI, então não há Serve nem Funnel; a tela diz isso e indica o app Tailscale só para a rede.
5. **ngrok temporário, só para testes** (Integrações → Acesso de fora, e como terceira opção no MCP):
   - **Agente oficial, sem SDK.** Usa o `ngrok` do PATH ou baixa `https://bin.equinox.io/c/bNyj1mQVY4c/ngrok-v3-stable-<so>-<arq>.(tgz|zip)` (mesma origem do instalador do site) para `camfacil-dados/bin`, só por HTTPS e só aceitando redirecionamento para `*.equinox.io`/`*.ngrok.com`, com teto de tamanho; abre o `.tgz`/`.zip` com o `miniz_oxide` que já está no binário, confere que `ngrok version` responde `ngrok version 3…` e registra o SHA-256 no evento. Um SDK em Rust traria tokio-util, protobuf e TLS próprios (MBs); o agente fica fora do binário e só gasta RAM ligado.
   - Roda `ngrok http 127.0.0.1:<porta> --log=stdout --log-format=json`; a URL vem do log (`started tunnel`) e, se não vier, da API local `http://127.0.0.1:4040/api/tunnels` (filtrando pelo `addr`). O **authtoken** vai por `NGROK_AUTHTOKEN` (nunca na linha de comando) e fica cifrado pelo cofre.
   - **Proteção obrigatória:** painel → `basic-auth` com senha aleatória mostrada na tela, por **Traffic Policy** num arquivo 0600 que some ao desligar (o `--basic-auth` está obsoleto no agente 3.x e exporia a senha no `ps`); só o MCP → um servidor à parte em `127.0.0.1:<porta livre>` com **apenas** `/mcp` (o ngrok não filtra caminho), protegido pelo token.
   - Pop-up obrigatório antes de ligar ("temporário… não é seguro… use o Tailscale"), aviso amarelo fixo com o tempo ligado, sugestão de desligar após 24 h e opção de desligar sozinho. Evento `acesso` com quem ligou (este computador ou o IP do aparelho) e quando. Fechar o programa derruba o túnel. Android: indisponível.

## Consequências

- Binário: só código (sem crate nova); o ngrok baixado ocupa ~30 MB em `camfacil-dados/bin`, fora do binário. RAM ociosa: zero (nada roda até o técnico ligar; consulta ao Tailscale só ao abrir a seção).
- O claude.ai passa a conectar pelo Funnel (ou pelo ngrok nos testes) com a URL com token.
- Pendências: validar numa máquina real com Tailscale (Serve/Funnel de verdade e o conector do claude.ai) e numa conta real do ngrok (a Traffic Policy `basic-auth` é validada na nuvem; aqui só foi conferido que o agente aceita o arquivo); OAuth.
