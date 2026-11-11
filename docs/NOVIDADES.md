# Novidades e próximos passos

Histórico de cada versão do CamFácil IA. A versão atual e os arquivos para baixar estão nas [releases](https://github.com/emersoncassis/CamFacil-IA/releases). Voltar ao [README](../README.md).

## Novidades da 0.9.9

- **CamFácil IA Builder:** skill para assistentes de código que ajuda quem não programa a montar a própria versão (telas, funções, inteligências, integrações). Usa o [Reversa](https://github.com/sandeco/reversa) como método e recusa, com o motivo e uma alternativa, o pedido que fere a LGPD, a licença AGPL-3.0 ou a segurança. Skill em `.claude/skills/camfacil-ia-builder/`, entrada para outros assistentes em `AGENTS.md` e [página no site](https://emersoncassis.github.io/CamFacil-IA/builder/) em português, inglês e espanhol.
- **Tela travada ao desinstalar ou apagar os dados:** depois de **Desinstalar** ou **Apagar todos os dados**, a dashboard escurece e bloqueia tudo, dizendo para fechar a página (desinstalou) ou abrir o programa de novo (apagou os dados).
- **Regra só liga com a inteligência ligada:** ligar uma regra cujo módulo está desligado agora avisa "nenhuma inteligência ligada" e oferece ligar o módulo, levando direto para a tela Módulos. Desligar um módulo desliga as regras que dependem dele, e elas voltam sozinhas quando o módulo for ligado de novo.
- **"Como funciona" nos módulos:** o botão **?** ao lado de **Aprender**, em Movimento inteligente e Módulo de IA, explica a diferença entre os dois e que podem ficar ligados juntos (não se atrapalham: com a IA ligada, zonas e linhas passam para ela e o movimento não conta em dobro).
- **Ícone no atalho:** o instalador do Windows e o do Linux agora criam o atalho com o ícone do CamFácil IA (câmera com ponto verde). Os arquivos estão em `scripts/icones/`.

## Novidades da 0.9.8

- **Aba Segurança:** acesso pela rede, aparelhos liberados, registro de segurança, IA conectada (MCP) e LGPD num lugar só.
- **Acesso pela rede com PIN:** na própria máquina abre direto; pela rede da loja, Tailscale ou ngrok, cada aparelho é liberado uma vez com o PIN gerado pelo programa (mostrado só na máquina) ou autorizado na máquina conferindo o mesmo código dos dois lados. Fica liberado por 30 dias sem uso. PIN fraco é recusado com explicação; tentativas erradas travam aos poucos. Ver [ADR 0017](adr/0017-acesso-pela-rede-com-pin-e-aparelhos.md).
- **MCP blindado contra abuso por IA:** limite de pedidos por minuto, trava por token errado, resultado marcado como dado (contra injeção de prompt) e registro de cada cliente de IA.
- **Proteção do navegador:** um site aberto no navegador do técnico não consegue mais mandar pedidos para a dashboard (CSRF) nem se passar por ela (DNS rebinding).
- **Documento de segurança** em [docs/seguranca/SEGURANCA.md](seguranca/SEGURANCA.md) e política de relato em [SECURITY.md](../SECURITY.md).
- **Site (cfia.ysafe.com.br):** título com a frase que se digita (como no README), botão **Baixar grátis** sempre no topo, menu que vira botão quando as abas não cabem (nada de nome cortado no celular) e a seção **Por dentro**, para quem programa: mapa interativo, GitHub, documentação, diagrama de como funciona e de segurança, como contribuir e como relatar falha. SEO: título e descrição novos, URL canônica, dados estruturados, `robots.txt` e `sitemap.xml`.
- **Ao atualizar:** quem abria a dashboard pela rede precisa liberar o aparelho uma vez (PIN da aba Segurança ou "Pedir à máquina").

## Novidades da 0.9.6

- **Regra sempre é criada:** com o módulo da inteligência desligado, a regra (sugerida pela IA ou desenhada) fica salva **desligada** e com um aviso dizendo qual módulo ligar. Ao ligar o módulo, ela liga sozinha.
- **Tela de regra não trava mais** a escolha da inteligência quando o módulo está desligado.

## Novidades da 0.9.5

- **Demonstração com a loja da Formação:** "Adicionar demonstração" agora mostra a mesma loja e as mesmas pessoas do curso no site (duas entram, uma sai, um cliente para no balcão), em vez dos blocos cinzas. Dá para treinar linha e zona no programa com a imagem que o técnico já viu na aula.

## Novidades da 0.9.4

- **Ligar e desligar regra** sem excluir (botão na lista de regras); desligada some do ao vivo e da TV e não alerta.
- **Permanência fica na regra** (editar a regra, campo de segundos); saiu da tela de Módulos.
- **Linha conta carro rápido:** o rastreio aceita mais deslocamento quando o PC demora entre leituras.

## Novidades da 0.9.3

- **Um módulo só liga a IA:** "Pessoas e veículos". Contagem, permanência e mapa de calor deixaram de ser interruptores separados na tela; viraram tipos de regra. Filtro de classe (pessoa, carro, moto, ônibus, caminhão, bicicleta) funciona sem ligar mais nada.

## Novidades da 0.9.2
- **Alertas dizem o que passou**: o balão e a mensagem do Telegram trazem "carro passou — entrada", "pessoa parada há 65 s", "3 carros ao mesmo tempo".
- **IA, avalie a minha regra**: ao lado de "Observar e sugerir" há "Avaliar uma regra". Você escolhe a regra criada, ela observa 1 minuto e diz em frases curtas se está bem posicionada (quantos passaram e quantos cruzaram a linha, quem entrou na área), se o limite de densidade ou o tempo de permanência estão certos (e sugere o número) e quem a regra está ignorando.

## Novidades da 0.9.1
- **Nada vem ligado**: movimento, cerca, saúde e os demais módulos começam desligados. Você escolhe quais alertas quer, inclusive ficar só com IA de vídeo e sem o movimento por pixel (que enche o histórico).
- **Alertas da própria câmera** (módulo novo, desligado por padrão): o programa escuta os eventos que a câmera ou o gravador manda (Intelbras/Dahua e Hikvision: movimento, cruzou a linha, invadiu a área…) e registra cada um como "recebido da câmera". Ligue e desligue em Módulos de IA.
- **Editar a regra depois de criada**: botão Editar na lista de regras (nome, textos, quem observa, tempo, cor e som). O que a regra faz não muda; para outra coisa, crie outra regra.

## Novidades da 0.9.0
- **Uma inteligência por regra** ([SDD-09](sdd/SDD-09-inteligencias-por-regra.md)): passagem (linha), presença, densidade, permanência e mapa de calor (zona). Cada uma mostra o peso (leve, médio, pesado) e só pode ser escolhida se o módulo dela estiver ligado.
- **Quem a regra observa**: pessoa, carro, moto, bicicleta, ônibus e caminhão, em qualquer combinação. "Linha só de carro e moto", "zona que avisa com 3 ou mais carros", "permanência só de moto".
- **Densidade** (N ou mais ao mesmo tempo) e **permanência** com tempo próprio por regra; o aviso diz o que é ("carro parado há 65 s").
- **Cor da linha** (6 opções) e **som do alerta** (bipe, sino, alarme, gota, sem som) por regra, no Ao vivo, em Câmeras e no Modo TV.
- **IA, sugira para mim** agora vê o que passa: se mais carros e pessoas passam, sugere **passagem** das classes certas; se alguém ou um carro chega e fica parado, sugere **permanência** daquela classe, com o tempo sugerido.
- Formulário da regra em 3 abas (O que faz, Quem, Alerta), sem rolagem em nenhum tamanho de tela. Regras antigas continuam como eram.

## Novidades da 0.8.2
- **Nada fica rodando escondido**: ngrok e ffmpeg morrem junto com o programa, mesmo se ele for fechado à força (janela fechada, Gerenciador de Tarefas). No Windows, por Job Object; no Linux/Android, por sinal do sistema. Ao abrir, derruba um ngrok que uma versão anterior tenha deixado.
- **Desinstalar** (Manutenção > Dados): para ngrok, endereço HTTPS do Tailscale e o abrir com o computador, tira o atalho e o programa e, se marcado, apaga os dados.
- **Versão no nome do arquivo baixado**: o site baixa `camfacil-0.8.2-windows-x86_64.exe` (e o mesmo para Linux, ARM e Android).

## Novidades da 0.8.1
- **MCP por HTTPS** (o Claude exige): pelo **Tailscale Serve** (só sua rede Tailscale) ou **Tailscale Funnel** (internet, para os conectores do claude.ai, com confirmação forte e token). Para Claude Desktop e Claude Code na mesma máquina, configuração por stdio pronta para colar (ADR 0015).
- **ngrok temporário** no Acesso de fora e no MCP: o programa baixa o agente oficial e sobe o túnel; aviso obrigatório de que é só para testes, pode cair e não é seguro (para mais de um dia, Tailscale); painel protegido por senha aleatória; opção de desligar sozinho após 24 h.

## Novidades da 0.8.0
- **Ao vivo completo**: balões das regras sobre cada câmera (alinhados à imagem, piscam no alerta), som do alerta (com "Ligar som" e mudo) e escolha de stream **extra** (padrão) ou **principal** por câmera, com aviso de que o principal é da gravação e pesa mais.
- **Nome real do canal** do DVR/NVR (Intelbras/Dahua e Hikvision, via Digest) vira o nome da câmera; nomes genéricos são descartados.
- **Nome padrão sem IP**: "Câmera 1", "Gravador 1"; os nomes antigos com IP são trocados sozinhos.
- **IA, sugira para mim** com textos curtos e confiança só em "% · boa/baixa".
- Botões de portão e relé também com HTTP Digest.

## Novidades da 0.7.1
- **IA, sugira para mim** consertado: botão **Cancelar**; **uma regra por pesquisa** (aceitar pede confirmação, cria a regra e para a pesquisa); o traçado tracejado sai da imagem; confiança **70% ou mais = boa**, abaixo disso aviso para observar de novo.
- **Dados numa pasta só, ao lado do programa** (`camfacil-dados`): nada em pasta escondida nem no registro. Em Manutenção > Dados, **Apagar todos os dados** zera o programa, inclusive a pasta antiga das versões até 0.7.
- **Mostrar linhas e zonas**: interruptor no ao vivo da câmera para mostrar o desenho das regras além do balão (desligado por padrão).

## Novidades da 0.7.0
- **Autocorreção de IP pelo MAC**: chave por câmera (Ligar e Editar), desligada por padrão. Confere porta RTSP e MAC no IP novo, troca só o host, confere a imagem em até 15 s e desfaz se não voltar. Não corrige com conflito de IP; no máximo uma correção a cada 10 min. Avisa no Início, no Ao vivo e no Telegram/WhatsApp/webhook. Histórico na tela Rede e no backup.
- **Gravador (DVR/NVR)**: reconhece o gravador (ONVIF, modelo ou teste dos canais) e deixa escolher um ou vários canais; cada canal vira uma câmera "Gravador · Canal N", no substream por padrão. Troca de canal no Editar; se o gravador mudar de IP, todos os canais corrigem juntos.
- **Estúdio**: abre com dois blocos fechados, *Treine sua IA* (os seis passos, funcionando) e *VMS sob medida* (só desenho).
- **Celular estreito**: ações do cabeçalho das telas viram só ícone, para não passar da borda.
- Ainda falta validar autocorreção e DVR/NVR com equipamento real (testados com rede simulada e gravador falso de 4 canais).

## Novidades da 0.5 e 0.6
- **Ao vivo**: mosaico de 1 a 4 câmeras e, embaixo, aparelho, IA, alertas, para quem vai e serviços ativos.
- **Login salvo das câmeras**, editar e remover câmera, zero rolagem em todas as telas.
- **Android**: IA de pessoas (YOLO) no aparelho, tela do servidor com o endereço da rede da loja e QR, botão do Tailscale.
- **Estúdio básico** de ponta a ponta e **manifesto narrado** no site.

## Regra de tela
Nenhuma tela rola: programa, app, site e curso cabem numa tela só, do celular ao monitor. O que não cabe vira aba, seção que abre ou página (regra 9 do [`CLAUDE.md`](../CLAUDE.md), detalhes no [`DESIGN.md`](../DESIGN.md)).

## Em desenvolvimento / próximos passos
- Desenhado, ainda **não** implementado: dashboard com senha própria; perfis técnico e cliente no app; vídeo à distância por WebRTC (hoje é pelo Tailscale); IA no navegador (WASM); clipes de 10 s nos alertas; plano de IPs; atualização com rollback e pacote offline; benchmark pela inferência real; contrato de módulo da comunidade (hoje a via é a receita do estúdio)
- IA avançada: diagnóstico da imagem, veredito por inteligência e aviso técnico para o cliente ([SDD-10](sdd/SDD-10-ia-avancada.md), proposta)
- Modo estúdio: conversa com a IA montando a receita, pré-marcação das fotos (vocabulário aberto) e posição da linha/zona vinda da receita (fase 7)
- Mosaico ao vivo com mais de 4 câmeras (6, 9, 16)
- Rede: senha de fábrica, quedas por hora e banda por câmera (conflito de IP já avisa na tela Rede)
- Telegram: backup agendado; foto do alerta com a linha/zona desenhada (`/backup <senha>` já existe)
- Android: câmera do próprio celular como fonte, chave de assinatura fixa do APK, APK por arquitetura (hoje ~20 MB com a IA)
- Testes em campo: rede real, Windows, TV Box com controle remoto, autocorreção de IP e canais de DVR/NVR com equipamento de verdade
