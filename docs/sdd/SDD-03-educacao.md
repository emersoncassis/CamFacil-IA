# SDD-03 — Educação, agente guia e gamificação

Status: **implementado** (Formação no site, selo, narração) · revisado em 2026-11-10 · ver ADR 0008. Não existe agente guia com voz dentro do programa local; o que o desenho antigo previa para lá (aula por módulo, `desafio.toml`, áudio por módulo, aula de rede em voz) **foi substituído** pela Formação do site.

## Objetivo
Ensinar IA de vídeo do zero, com a voz do autor, sem custo de processamento e sem instalar nada.

## Onde fica cada coisa (ADR 0008)
- **Instrução** fica no site: a **Formação de IA para CFTV** em `https://cfia.ysafe.com.br/aprender` (`crates/cfia-api/web/escola.{html,js,css}`). Roda toda no navegador: animações Errado × Certo sobre a foto da loja, exercícios interativos (desenhar a linha e a zona e ver a IA julgar), simulação do que o cliente vê, quiz de saúde da câmera e selo. Progresso salvo no navegador.
- **Operação** fica no programa local: Câmeras (buscar → senha → ao vivo), regras, “IA, sugira para mim”, módulos, integrações. Nenhuma aula embutida; os botões **Aprender** abrem a lição correspondente no site.
- Na entrada do site o técnico escolhe: **Aprender IA do zero** (formação) ou **Já trabalho com IA** (download e operação).
- Narração: `/audio/escola-01..06.ogg` no site (Opus ~24 kbps, geradas no Voice Studio). Sem o arquivo, a lição funciona só com texto.

## Lições (escola.js, `LICOES`)
1. Como a IA enxerga (centro da mancha) · 2. Linha de passagem (exercício) · 3. Zona (exercício) · 4. Balões, alertas e som (simulação) · 5. Saúde da câmera (quiz) · 6. Da formação para a loja (selo + download).
- A fala nunca é cortada: o próximo áudio espera o atual terminar. Há narração para cada lição, para a introdução e para cada acerto e erro dos exercícios (`/audio/escola-*.ogg`, textos em `narracao.json`).
- Simulação fiel à cena: as pessoas entram pela porta e vêm em direção à câmera (ficam maiores), saindo cortadas pela borda; a faixa da borda (~10%) é mostrada como overlay; o cliente para na frente do balcão.
- **Selo:** ao concluir, o técnico entra com Google (`/api/auth/google?voltar=/aprender`), informa o Instagram (opcional) e autoriza ou não o repost (Emerson e Papo Segurança). `POST /api/escola/selo` cria um selo por técnico (tabela `selos_escola`, migração 0003) com código público; `/selo/{codigo}` mostra o cartão, gera a imagem 1080×1920 para stories no navegador (canvas) e oferece WhatsApp, LinkedIn, compartilhar e copiar.
- O exercício só libera a próxima lição quando a IA simulada aprova (linha que conta; zona sem alarme falso e com ocupado).

## Formato de aula (como as lições do site seguem)
1. O que é e para que serve (áudio + exemplo animado sobre a foto da loja).
2. Exemplo certo × erro comum (ex.: linha encostada na borda).
3. Prática: o exercício roda no navegador e a IA simulada confere (linha que conta, zona sem alarme falso e com ocupado).
4. Mini desafio → selo.

O que o programa local faz de "prática na câmera do técnico": o selo de operação (abaixo), o botão **Aprender** que abre a lição certa no site e a **câmera de demonstração** com a mesma loja da Formação (`scripts/gerar-video-demo.py`, `demo-loja.mp4` embutido no binário), para treinar linha e zona no programa com a imagem que o técnico já viu na aula.

## Desafios
Os desafios são os exercícios de `escola.js` (verificados na simulação do navegador). **Não existe `desafio.toml`.** Não há verificação automática de uma regra criada pelo técnico na câmera dele; o mais próximo é o **"IA, avalie a minha regra"** do programa (SDD-09), que observa a câmera por 1 minuto e diz em frases curtas se a regra está bem posicionada.

## Gamificação
- **Selo da Formação** (site): feito, com código público e cartão para stories (acima).
- **Selo de operação local** (`/api/selos`, tabela `selos`): o selo `cerca` é concedido quando uma regra de **zona** e uma de **linha** já dispararam de verdade numa câmera (`video.rs`); aparece na dashboard e entra no backup. Selos por outros módulos e exportação: pendentes.
- Ranking e certificado "Instalador CamFácil IA": opcionais, via serviço da comunidade (fase posterior); **não implementados**; nunca obrigatórios para usar o produto.

## Aulas de rede
- **Não implementadas em voz.** O programa gera o **diagrama unifilar** em SVG (SDD-05) com IP e MAC, sem narração, e a tela Rede mostra dicas em texto para cada diagnóstico. A lição de rede (IP, máscara, gateway, conflito, MAC) e o convite "Acabou? Não tem mais câmera?" seguem como ideia para a Formação.

## Posicionamento
Toda aula final lembra: o CamFácil IA é para aprender e validar; a solução definitiva é câmera/gravador com IA de borda ou plataforma corporativa em nuvem.
