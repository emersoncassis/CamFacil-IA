# SDD-03 — Educação, agente guia e gamificação

Status: proposto · Fase 2

## Objetivo
Transformar cada configuração numa aula curta, com a voz do autor, sem custo de processamento e funcionando offline.

## Agente guia
- Roteiro por tela/módulo em `guide/<modulo>/aula.toml`: passos com `alvo` (seletor CSS), `audio`, `texto`, `acao_esperada`.
- O front destaca o alvo (contorno `--guide`), toca o áudio e mostra o texto (legenda) ao mesmo tempo.
- Áudios em Opus/OGG (~24 kbps), gerados previamente no Voice Studio e empacotados com o módulo.
- Botões: ouvir de novo, pular, desligar guia.

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
