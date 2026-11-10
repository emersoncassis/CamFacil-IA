# Segurança do CamFácil IA

Como o CamFácil IA protege as câmeras do cliente, o acesso à dashboard, os segredos e a IA conectada. Vale para a versão da `main` em 10/11/2026. Decisões: [ADR 0017](../adr/0017-acesso-pela-rede-com-pin-e-aparelhos.md) (acesso pela rede e MCP), [ADR 0012](../adr/0012-mcp-json-rpc-proprio.md) (MCP), [ADR 0015](../adr/0015-mcp-https-tailscale-e-ngrok.md) (HTTPS e túneis). Auditoria: [parecer de 10/11/2026](PARECER-2026-11-10.md). Relatar falha: [SECURITY.md](../../SECURITY.md).

## Para o técnico, em um minuto

- **Na máquina onde o programa roda, tudo abre direto.** Pela rede (celular, TV, Tailscale, ngrok), cada aparelho é liberado uma vez: com o **PIN** da aba Segurança ou **autorizado na máquina**, conferindo o mesmo código dos dois lados.
- **Não troque o PIN por 123456.** O programa recusa sequência, repetição, data e os PINs mais usados, porque é exatamente isso que os robôs de invasão testam primeiro.
- **Não reconhece um aparelho?** Bloqueie ou remova na aba Segurança. Emergência: **Bloquear todo acesso pela rede** (a máquina continua funcionando).
- **IA conectada (MCP)** vem desligada. Ligada, ela lê; mudar algo só com a sua permissão; portão e relé, nunca.
- **Para acesso de fora, prefira o Tailscale** (cifrado de ponta a ponta). O ngrok é só para testes rápidos.

## O que protegemos e de quem

| Ameaça | Exemplo | Defesa principal |
|---|---|---|
| Aparelho estranho na rede da loja | alguém no Wi-Fi do cliente abre `http://ip-da-máquina:7070` | Acesso pela rede com PIN ou confirmação na máquina |
| Robô na internet | túnel ngrok descoberto, força bruta no PIN | PIN forte, trava progressiva, atraso por erro, registro |
| Site malicioso no navegador do técnico | página que manda pedidos para `localhost:7070` (CSRF) ou se passa por ele (DNS rebinding) | Checagem de `Host` e `Origin` em todas as rotas |
| IA maliciosa ou enganada | agente LLM tentando abusar do MCP ou texto plantado num nome de câmera ("ignore as instruções…") | Token, limite por minuto, trava, só leitura por padrão, resultado marcado como dado |
| Vazamento de segredo | token no código ou no histórico do git | Cofre cifrado, push protection, varredura de segredos |
| Dependência vulnerável | biblioteca com falha conhecida | Dependabot, `cargo audit`, CodeQL |
| Uso indevido das imagens | câmera filmando sem aviso | Termo do técnico, autorização do cliente, placa (LGPD) |

## Camadas de proteção

### 1. Acesso à dashboard pela rede (ADR 0017)

Código: `crates/camfacil/src/acesso.rs` · tela: aba **Segurança** e `web/entrar.*`.

- **Local x de fora:** local é conexão de `127.0.0.1` sem cabeçalho de encaminhamento. Tailscale Serve/Funnel e ngrok conectam em `127.0.0.1`, mas marcam o pedido com `X-Forwarded-For` e por isso contam como de fora.
- **PIN** de 6 dígitos gerado com o gerador aleatório do sistema e mostrado só na tela da máquina. PIN escolhido à mão passa pelas regras do NIST SP 800-63B para segredos memorizados: tamanho mínimo, lista de bloqueio (sequências, repetições, padrões, datas, mais usados) e explicação do motivo ao recusar. Guardado cifrado no cofre (ChaCha20-Poly1305) e comparado em tempo constante.
- **Confirmação dos dois lados:** o aparelho pede, os dois mostram o mesmo código de 6 dígitos, e a pessoa na máquina autoriza. O código do pedido só é entregue ao aparelho que o criou (segredo de 128 bits) e vale uma vez.
- **Aparelho lembrado** por 30 dias sem uso: token aleatório de 256 bits, só o SHA-256 no servidor, cookie `HttpOnly; SameSite=Strict` (`Secure` por HTTPS). Até 50 aparelhos; bloquear, liberar, remover e desconectar todos.
- **Contra força bruta:** 5 erros travam a entrada por PIN por 5 min, depois 10, 20… até 24 h; 0,8 s de atraso a cada erro; no máximo 5 pedidos pendentes, um por endereço, 3 min cada.
- **Separação de poderes:** PIN, pedidos, aparelhos e bloqueio geral só se administram na própria máquina. Um aparelho liberado não consegue liberar outro.
- **Fora do backup:** restaurar um backup em outra máquina não leva PIN nem aparelhos.

### 2. Proteção do navegador (CSRF e DNS rebinding)

Código: `crates/camfacil/src/protecao.rs`, aplicado a todas as rotas.

- `Host` aceito só se for `localhost`, IP, nome de rede local (sem ponto ou `.local`), Tailscale (`*.ts.net`) ou ngrok. Um domínio de atacante apontado para `127.0.0.1` recebe 403.
- `Origin`, quando o navegador manda, precisa ser o próprio endereço da dashboard; `Origin: null` é recusado; navegador antigo com `Sec-Fetch-Site: cross-site` não muda nada.
- Segue o OWASP *Cross-Site Request Forgery Prevention Cheat Sheet* (verificação de origem) e as recomendações contra DNS rebinding da especificação do MCP.

### 3. IA conectada (MCP) contra ataques com IA

Código: `crates/camfacil/src/mcp.rs` · referência: **OWASP Top 10 for LLM Applications (2025)**.

| Risco OWASP LLM | Como o CamFácil IA trata |
|---|---|
| LLM01 Injeção de prompt (indireta) | Todo resultado de ferramenta vem precedido de um aviso de que é **dado, não instrução**; as instruções do servidor repetem isso; textos têm controle removido e são cortados em 300 caracteres |
| LLM02 Vazamento de informação sensível | Nenhuma ferramenta devolve senha, token, URL RTSP com usuário/senha ou PIN; `ver_alertas` não entrega o registro de segurança (IPs, aparelhos) |
| LLM06 Agência excessiva | MCP desligado por padrão; ligado, só leitura; escrever exige chave "Permitir alterações"; portão e relé nunca (só listar) |
| LLM10 Consumo sem limite | 60 pedidos por minuto; acima disso, `429` com `Retry-After` e registro |
| Força bruta no token | 10 tokens errados em 10 min pela rede travam o MCP pela rede por 15 min; gerar outro token destrava; o uso local (ponte stdio) não é travado por ataque de fora |

E ainda: token de 192 bits guardado cifrado e comparado em tempo constante; pela internet só com HTTPS (token na URL só aceito via proxy HTTPS local); verificação de `Origin`; lotes JSON-RPC recusados; conexão de cliente de IA, recusas e escritas ficam no registro da aba Segurança.

### 4. Ações físicas (portão e relé)

Código: `crates/camfacil/src/botoes.rs`. PIN próprio (PBKDF2-SHA256, 60 mil iterações), confirmação com código de uso único de 20 s, 5 erros travam por 5 min, log de cada acionamento e recusa, ação cifrada no cofre. O MCP não aciona.

### 5. Segredos

- Tokens do Telegram, WhatsApp, ngrok, MCP, PIN de acesso e ações dos botões ficam no **cofre** (`crates/cf-notify/src/cofre.rs`, ChaCha20-Poly1305, chave por máquina).
- Nada de segredo no código: `.env` no `.gitignore`, `.env.example` só com nomes; **push protection** e **secret scanning** ligados no GitHub.
- Backup exportado é cifrado com a senha escolhida pelo técnico.

### 6. API e site na nuvem (`cfia.ysafe.com.br`)

Código: `crates/cfia-api`. Login só com Google (OpenID Connect com PKCE, `state` e `nonce`, ID token conferido em RS256 contra as chaves públicas do Google); cookie `HttpOnly; Secure; SameSite=Lax`; verificação de `Origin` em tudo que muda algo e no WebSocket do painel; Row-Level Security no Postgres com `SET LOCAL ROLE cfia_app` em toda transação; volta pós-login só para caminho local; imagem Docker distroless sem root; banco sem porta publicada.

### 7. Dados e LGPD

Offline-first: imagens não saem da máquina, a não ser nos alertas que o técnico configurar. Antes de qualquer integração, o técnico aceita o termo de responsabilidade; o cliente assina o termo de autorização e a placa de aviso é gerada (aba **Segurança → LGPD**). Existe "apagar todos os dados" e a desinstalação apaga tudo se pedido. Detalhes em `docs/legal/`.

### 8. Repositório e cadeia de suprimentos

- **CodeQL** (análise de código), **Dependabot** (alertas de dependência e malware), **secret scanning** e **push protection** ligados; relato privado de vulnerabilidades ligado.
- `cargo audit` sem alerta aberto; dois alertas avaliados como não exploráveis estão justificados em `.cargo/audit.toml`.
- Workflows do GitHub Actions com permissão mínima (`contents: read`); só os jobs que publicam pedem mais.
- Releases e APK são gerados pelo CI a partir do código público.

### 9. App Android

`allowBackup=false`; componentes não exportados, exceto o lançador e o receptor de boot (só eventos do sistema); a ponte JavaScript atende só a página interna do app; as chamadas em segundo plano mandam o cookie do aparelho liberado.

## Registro e resposta a incidente

Tudo fica no **registro** da aba Segurança: entradas, pedidos, recusas (em amarelo), trocas de PIN, bloqueios, conexão de clientes de IA e recusas do MCP.

Se algo parecer errado:

1. **Bloquear todo acesso pela rede** (aba Segurança → Acesso). A máquina continua gravando e alertando.
2. **Desconectar todos** os aparelhos e **gerar outro PIN**.
3. MCP: **gerar outro token** ou desligar (Conexões → MCP).
4. Trocar senhas de câmeras e gravadores e revogar tokens de Telegram/WhatsApp, se houver suspeita sobre eles.
5. Relatar pelo [canal privado](../../SECURITY.md) se a falha for do programa.

## Limitações conhecidas

- Na rede da loja a dashboard fala HTTP: o conteúdo não é cifrado dentro do Wi-Fi. Para acesso de fora, use o Tailscale (cifrado) ou HTTPS pelo Tailscale Serve.
- Quem está na frente da máquina vê o PIN: a máquina é o ponto de confiança, então proteja o login do sistema operacional.
- Forks e clones feitos antes de 10/11/2026 guardam histórico antigo; os tokens daquela época foram revogados.
- O CamFácil IA é ambiente de aprendizagem e validação, não sistema de segurança profissional (ver termos).

## Manifesto de segurança

O CamFácil IA nasceu para levar IA de vídeo a quem instala câmeras, e quem instala câmera cuida da segurança de alguém. Por isso, segurança aqui não é opcional nem acessório:

- **Seguro por padrão.** Tudo que pode expor algo vem desligado ou protegido: MCP desligado, escrita bloqueada, acesso pela rede só com aparelho autorizado, portão só com PIN e confirmação.
- **A máquina do cliente é a dona.** Nada essencial depende de nuvem; quem administra o acesso é quem está na frente da máquina.
- **Educar em vez de culpar.** O programa explica por que recusa um PIN fraco e por que pede autorização, para o técnico levar o hábito para todas as instalações.
- **IA com rédea.** Uma IA conectada lê, propõe e só muda algo com permissão. Ela nunca aciona nada físico, nunca vê segredo e nunca obedece a texto escondido nos dados.
- **Transparência.** Código aberto, decisões registradas em ADRs, auditoria publicada e canal privado para quem encontrar uma falha.

A cibersegurança do CamFácil IA foi avaliada e é assumida por **Emerson de Carvalho Assis**, responsável técnico pelo projeto, com formação em:

- **210W-07 ICS Cybersecurity Vulnerabilities**: CISA, Departamento de Segurança Interna dos EUA (U.S. Department of Homeland Security), ago. 2025;
- **Ethical Hacker**: Cisco Networking Academy, certificado de conclusão, dez. 2024;
- **Network Support and Security**: Cisco Networking Academy, certificado de conclusão, dez. 2024.

Antes de trabalhar com IA, Emerson foi instrutor técnico de segurança eletrônica para profissionais de todo o Brasil e analista de produto na mesma área.
