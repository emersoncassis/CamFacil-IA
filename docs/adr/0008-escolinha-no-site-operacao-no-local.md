# ADR 0008 · Escolinha no site, operação no programa local

**Situação:** aceita · 2026-11-07

## Contexto

A aula da cerca virtual rodava dentro do programa local, misturada com a operação: cartão de aula no meio da tela de câmeras, botões que trocavam de aba, fala cortada ao clicar. Quem já trabalha com IA tropeçava na aula; quem nunca mexeu precisava baixar o programa só para aprender.

## Decisão

- **Instrução** vive no site, em `/aprender` (Escolinha de IA): roda inteira no navegador, sem login e sem instalar nada, com animações, exercícios interativos sobre a foto da loja e narração. Progresso e selo ficam no navegador.
- **Operação** vive no programa local, sem aula embutida. Os botões **Aprender** dos módulos abrem a lição correspondente no site, em outra aba.
- Na entrada do site há a escolha explícita: **Aprender IA do zero** ou **Já trabalho com IA** (download e operação).

## Consequências

- Programa local mais leve (saem JSON, áudios e imagens da aula: ~230 KB) e mais direto.
- A escolinha pode evoluir sem lançar versão do programa, e serve de porta de entrada para quem ainda não tem câmera.
- Offline: a escolinha precisa de internet uma vez; a operação continua offline-first. Aceitável, porque aprender é um momento, operar é o dia a dia.
- O selo da cerca no programa passa a ser de operação real (linha e zona disparando numa câmera sua), não de aula.
