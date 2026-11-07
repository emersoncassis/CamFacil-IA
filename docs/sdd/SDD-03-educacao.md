# SDD-03 — Educação, agente guia e gamificação

Status: implementado (formação no site) · revisado em 2026-11-07 · ver ADR 0008

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
- **Selo:** ao concluir, o técnico entra com Google (`/api/auth/google?voltar=/aprender`), informa o Instagram (opcional) e autoriza ou não o repost (Emerson, Papo Segurança, Zion). `POST /api/escola/selo` cria um selo por técnico (tabela `selos_escola`, migração 0003) com código público; `/selo/{codigo}` mostra o cartão, gera a imagem 1080×1920 para stories no navegador (canvas) e oferece WhatsApp, LinkedIn, compartilhar e copiar.
- O exercício só libera a próxima lição quando a IA simulada aprova (linha que conta; zona sem alarme falso e com ocupado).

## Formato de aula
1. O que é e para que serve (áudio + exemplo em vídeo).
2. Exemplo certo × erro comum (ex.: linha encostada na borda).
3. Prática: o técnico configura na câmera dele; o sistema confere (ex.: linha dentro da área útil, polígono com área mínima).
4. Mini desafio → selo.

## Desafios
`desafio.toml` define verificações objetivas (ex.: "crie uma linha que dispare quando alguém atravessar a porta"; verificado com o vídeo de demonstração).

## Gamificação
- Selos locais por módulo; exportáveis.
- Ranking e certificado: opcionais, via serviço da comunidade (fase posterior); nunca obrigatórios para usar o produto.

## Aulas de rede
- Ao terminar de adicionar câmeras: "Acabou? Não tem mais câmera?" → diagrama unifilar (SDD-05) explicado em voz: IP, máscara, gateway, conflito, MAC.

## Posicionamento
Toda aula final lembra: o CamFácil IA é para aprender e validar; a solução definitiva é câmera/gravador com IA de borda ou plataforma corporativa em nuvem.
