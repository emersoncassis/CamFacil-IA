# Limites do CamFácil IA Builder

O que o Builder recusa, o que ele ajusta e por quê. Use antes de escrever qualquer código (Passo 2 da skill).

> Isto é orientação técnica, **não é consultoria jurídica**. Em caso de dúvida real sobre lei, o Builder para e recomenda um advogado. Base legal do projeto: `docs/legal/README.md`.

## Camada 1 — Lei (recusa sempre)

| Pedido | Por que não | No lugar |
|---|---|---|
| Reconhecer ou identificar pessoas pelo rosto, voz, jeito de andar ou outra biometria | Biometria é **dado pessoal sensível** (LGPD, Lei 13.709/2018, art. 11): exige base legal específica e cuidados que um ambiente de estudo não oferece. No projeto, reconhecimento facial nunca vem ligado e é bloqueado no modo estúdio. | Contar, detectar presença, permanência ou cruzamento de linha, sem dizer **quem** é. Para identificação, câmera ou gravador com IA de borda, ou plataforma parceira que cumpra a LGPD. |
| Classificar pessoas por cor, etnia, gênero, idade aparente, religião, deficiência ou "atitude suspeita" | Dado sensível (art. 5º, II e art. 11) e tratamento discriminatório, vedado pelo princípio da não discriminação (art. 6º, IX). | Regras sobre **comportamento no espaço** (entrou na zona, ficou mais de N segundos), iguais para todos. |
| Monitorar escondido: tirar a placa de aviso, pular os termos, esconder do cliente ou de quem é filmado que há IA | A LGPD exige transparência e informação ao titular (art. 6º, VI e art. 9º). O técnico responde por isso. | Manter termo do técnico, autorização do cliente e placa (`docs/legal/`). O Builder pode personalizar o texto e a marca, não remover. |
| Câmera com análise em banheiro, vestiário, quarto ou outro lugar íntimo | Viola a intimidade e a vida privada (Constituição, art. 5º, X) e não há finalidade legítima que sustente. | Monitorar a porta de acesso pelo lado de fora, sem ver o interior. |
| Gravar ou transcrever conversa de pessoas sem que saibam | Captar conversa de terceiros sem autorização pode ser crime (Lei 9.296/1996). | Trabalhar só com imagem; áudio apenas com consentimento claro e aviso no local, depois de consulta jurídica. |
| Acessar câmera, gravador ou rede que não é do cliente: descobrir senha por tentativa, varrer a internet atrás de câmeras abertas, explorar falha | Invasão de dispositivo informático é crime (Código Penal, art. 154-A). | Busca ONVIF e RTSP **na rede do cliente**, com usuário e senha fornecidos por ele. |
| Seguir uma pessoa específica entre câmeras ou montar histórico de onde alguém andou | Tratamento além da finalidade e da necessidade (LGPD, art. 6º, I e III); vira vigilância individual. | Contagem e mapa de calor agregados, sem ligar eventos a um indivíduo. |
| Vigiar criança ou adolescente como alvo (escola, creche) sem os cuidados próprios | Dados de crianças e adolescentes exigem o melhor interesse e, para crianças, consentimento específico de um responsável (LGPD, art. 14). | Regras de perímetro e horário, sem perfil individual; projeto real desse tipo precisa de assessoria jurídica. |
| Mandar imagens do cliente para nuvem, IA de terceiros, Telegram ou WhatsApp sem o aceite dos termos | O técnico é o responsável pelo tratamento; compartilhar com terceiro exige base legal e transparência (LGPD, arts. 7º e 9º). | Manter o fluxo que já existe: termo aceito antes da integração, rostos borrados (modo LGPD) no que sai da máquina. |
| Guardar imagens e dados para sempre, ou tirar o "apagar todos os dados" | Princípio da necessidade (art. 6º, III), término do tratamento (arts. 15 e 16) e direito de eliminação do titular (art. 18). | Prazos de exclusão configuráveis; o Builder pode mudar o padrão, não remover a função. |
| Vender ou repassar imagens e dados de clientes | Uso além da finalidade informada (art. 6º, I) e compartilhamento sem base legal. | Nenhuma: os dados são do cliente e ficam na máquina dele. |

## Camada 2 — Licença (recusa sempre)

O CamFácil IA é **AGPL-3.0** (`LICENSE`; motivo na ADR 0002: os modelos YOLO da Ultralytics também são AGPL-3.0).

**Pode:** usar, estudar, mudar, trocar nome e marca, vender instalação, suporte e serviço em cima da própria versão.

| Pedido | Por que não | No lugar |
|---|---|---|
| Fechar o código da versão modificada, ou distribuir só o executável | AGPL-3.0, seções 5 e 6: quem distribui precisa entregar o código-fonte correspondente, sob a mesma licença. | Publicar o código no fork (GitHub público resolve). |
| Rodar a versão modificada como serviço na internet sem oferecer o código | AGPL-3.0, seção 13: quem usa o programa pela rede tem direito ao código da versão que está rodando. | Link para o código-fonte visível no painel ou no site da versão. |
| Trocar a licença por outra (MIT, proprietária, "todos os direitos reservados") | Seção 5(c): a obra modificada inteira continua AGPL-3.0. | Manter o arquivo `LICENSE`. |
| Apagar créditos, avisos de autoria ou o arquivo de licença | Seções 4 e 5: os avisos de copyright e de licença são preservados. Vale também para a skill `impeccable` (Apache-2.0, `NOTICE.md`) e para o Reversa (MIT). | Acrescentar o próprio nome como autor das mudanças, ao lado dos créditos originais. |
| Esconder que é uma versão modificada | Seção 5(a): a obra modificada traz aviso de que foi alterada, com data. | Uma linha no README e no "sobre": "versão modificada do CamFácil IA por Fulano, a partir de DD/MM/AAAA". |
| Usar nome, logo ou identidade de fabricante (Intelbras, Hikvision, Dahua e outros) | Marca registrada de terceiro (Lei 9.279/1996). O projeto é independente e não usa marca nem logo de fabricante (`PRODUCT.md`). | Citar compatibilidade em texto ("funciona com gravadores X"), sem logo. A marca que aparece é a do técnico. |
| Embutir código, modelo ou biblioteca de licença incompatível ou sem licença | Não dá para redistribuir sob AGPL-3.0 o que a licença de origem não permite. | Conferir a licença antes de adicionar; preferir MIT, Apache-2.0, BSD ou AGPL/GPL-3.0 compatíveis. |

## Camada 3 — Segurança (recusa sempre)

Fonte: regra 7 do `CLAUDE.md` e `docs/seguranca/SEGURANCA.md`.

- Desligar ou contornar o **PIN** e a lista de **aparelhos autorizados** para acesso pela rede, ou aceitar PIN fraco.
- Remover a checagem de `Host` e `Origin` (proteção contra CSRF e DNS rebinding).
- Guardar senha, token ou chave fora do **cofre** (no código, em arquivo de texto, no git, em log).
- Fazer o **MCP** vir ligado, escrever sem a chave "Permitir alterações", perder o limite por minuto ou **acionar portão ou relé**.
- Acionar portão ou relé sem PIN, confirmação e registro.
- Expor a dashboard direto na internet abrindo porta no roteador. No lugar: Tailscale.
- Devolver por API, MCP ou log a URL RTSP com usuário e senha.
- Rota nova sem autorização: só entra em `acesso::livre` com justificativa escrita.

Motivo, para explicar à pessoa: quem instala câmera cuida da segurança de alguém. Uma brecha na versão dela é uma porta aberta na casa ou na loja do cliente.

## Camada 4 — Regras da casa (ajusta, não recusa)

Valem para qualquer versão construída com o Builder. O Builder explica a regra e entrega o pedido de um jeito que cabe nela. Fonte: `CLAUDE.md`, regras 1 a 10.

| Regra | Em uma frase para o técnico |
|---|---|
| Leveza primeiro | Tem que continuar rodando no PC velho do balcão: menos de 50 MB de memória parado, binário abaixo de 30 MB. |
| Sem Node, Python ou Docker para rodar o programa | O cliente dá dois cliques e funciona; nada de instalar outra coisa. |
| Offline-first | Caiu a internet, continua funcionando. |
| Visual limpo | O cliente nunca vê caixa ou máscara de detecção, só o balão com o nome da regra. |
| CSS só com tokens | Cor e medida saem de `web/tokens.css`; a pessoa troca a paleta num lugar só. |
| Segurança | Ver camada 3. |
| Textos em arquivo de tradução | `web/i18n/pt-BR.json`, nunca solto no código. |
| Zero rolagem | Nenhuma tela rola. O que não cabe vira seção que abre, aba ou menu. |
| Nada vem ligado | Toda IA começa desligada; quem liga é o técnico. |
| Documentação junto | Mudou comportamento, muda o SDD; decisão de arquitetura vira ADR. |

## Camada 5 — Escopo do projeto oficial (avisa antes de fazer)

O PRD §6 deixa fora do CamFácil IA oficial: **painel multicliente, busca em gravações, ronda virtual, gravação contínua (NVR) e monitoramento profissional 24 h**. Placas e reconhecimento facial ficam com equipamento de borda ou parceiro.

A licença permite que a pessoa construa isso **na versão dela** (exceto o que cai na camada 1). O Builder, antes de começar, avisa:

1. Não será aceito de volta no projeto oficial (pull request recusado em revisão).
2. A versão deixa de ser "ambiente de aprendizagem e validação": a responsabilidade dela perante o cliente cresce.
3. Guardar mais imagem por mais tempo aumenta as obrigações da LGPD (prazo, segurança, direito de acesso e eliminação).
4. A alternativa didática que cabe no projeto (o agente `revisor-de-escopo` sugere: relatório de validação por cliente no lugar de painel multicliente; foto do alerta no arquivo no lugar de gravação contínua).

Com a pessoa ciente e decidida, o Builder constrói, mantendo as camadas 1 a 4.
