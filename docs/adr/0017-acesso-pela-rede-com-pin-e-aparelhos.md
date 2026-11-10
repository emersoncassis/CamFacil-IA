# ADR 0017 · Acesso pela rede com PIN gerado e aparelhos autorizados

**Situação:** aceita · 2026-11-10 · complementa o [SDD-05](../sdd/SDD-05-rede-e-manutencao.md), o [SDD-06](../sdd/SDD-06-saidas-app-web.md) e a [ADR 0012](0012-mcp-json-rpc-proprio.md) · detalhes em [docs/seguranca/SEGURANCA.md](../seguranca/SEGURANCA.md)

## Contexto

A dashboard local não tinha login: na própria máquina isso é o certo (offline-first, zero atrito), mas com `--lan`, pelo IP do Tailscale ou por túnel (Tailscale Serve/Funnel, ngrok) qualquer aparelho que alcançasse a porta abria tudo, inclusive câmeras e configuração. O parecer de segurança de 2026-11-10 (item 6) recomendou proteger esse acesso. O público é o técnico de CFTV, que na prática põe `123456` quando pode escolher; e hoje os ataques a câmeras são majoritariamente automatizados, cada vez mais com IA.

## Decisão

1. **Na própria máquina abre direto.** "Local" é conexão de `127.0.0.1` **sem** cabeçalho de encaminhamento (`X-Forwarded-For`, `Forwarded`, `X-Forwarded-Host`, `X-Real-IP`, `Tailscale-User-Login`): Serve, Funnel e ngrok conectam em `127.0.0.1`, mas marcam o pedido e por isso contam como "de fora".
2. **De fora, só aparelho autorizado**, por um de dois caminhos:
   - **PIN de 6 dígitos gerado pelo programa** e mostrado só na tela da máquina (aba Segurança). O técnico que instala sabe; quem está longe liga para o cliente. PIN escolhido à mão só se passar nas regras (6 a 8 dígitos, sem sequência, repetição, padrão, data ou lista dos mais usados). O PIN não é enviado por Telegram, WhatsApp nem e-mail.
   - **Confirmação dos dois lados:** o aparelho pede acesso e mostra um código de 6 dígitos; a máquina mostra o mesmo código e alguém clica em "Autorizar". O QR da aba Segurança só leva o celular até essa tela. É o caminho da TV, que não lê QR.
3. **Aparelho lembrado por 30 dias sem uso** (cada uso renova; a TV não precisa de PIN todo mês). Cookie `HttpOnly; SameSite=Strict` (`Secure` quando vem por HTTPS); no servidor fica só o SHA-256 do token.
4. **Só a máquina administra:** ver e trocar o PIN, autorizar ou recusar pedidos, bloquear, liberar, remover e desconectar todos, e o bloqueio geral do acesso pela rede. Quem entrou de fora não consegue liberar outros aparelhos nem ler o PIN.
5. **Defesa contra força bruta:** 5 PINs errados travam a entrada por PIN por 5 min, depois 10, 20… até 24 h; atraso de 0,8 s a cada erro; no máximo 5 pedidos pendentes (um por endereço), cada um vale 3 min. A confirmação dos dois lados continua disponível durante a trava (precisa de uma pessoa na máquina).
6. **Fica de fora da trava** o que já tem token próprio: `/mcp` e o webhook do WhatsApp (Evolution API).
7. **Registro:** `acesso`, `acesso_negado` e `acesso_config` no histórico de eventos, visíveis na aba Segurança.
8. **Fora do backup:** chaves `acesso_*` são locais (como `mcp_*`); restaurar um backup não libera aparelhos em outra máquina.
9. **MCP endurecido junto** (mesma aba, seção "IA conectada"): no máximo 60 pedidos por minuto; 10 tokens errados em 10 min pela rede travam o MCP pela rede por 15 min (o uso local, como a ponte stdio, não é afetado; gerar outro token destrava); todo resultado vem marcado como dado, com controle removido e textos cortados em 300 caracteres (contra injeção indireta de prompt); `ver_alertas` não entrega o registro de segurança; conexão de cliente de IA, recusas e escritas ficam no registro.

## Consequências

- Nenhuma dependência nova; custo por pedido: conferir dois cabeçalhos e, de fora, um SHA-256 e uma busca numa lista de até 50 aparelhos.
- O app Android manda o cookie da WebView nas chamadas em segundo plano (alertas, câmeras) e trata `401` como "programa no ar pedindo PIN" ao parear.
- Quem acessava pela rede sem senha precisa liberar o aparelho uma vez depois de atualizar.
- O PIN fica cifrado no cofre (ChaCha20-Poly1305) para a tela poder mostrá-lo de novo; quem tem acesso à própria máquina vê o PIN, o que é a premissa (a máquina é o ponto de confiança).
- Alternativas descartadas: usuário e senha (o técnico reusa senha e esquece), login pelo Google na dashboard local (quebra o offline-first) e mandar o PIN por mensageiro (um canal comprometido passaria a abrir a dashboard).
