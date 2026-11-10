# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users
- **Técnico instalador de CFTV** (principal): configura câmeras e regras de IA na visita ao cliente, em notebook ou no PC de balcão; usa a ferramenta para aprender IA e rede IP na prática e para demonstrar ao cliente.
- **Dono do comércio** (loja, bar, portaria): deixa a dashboard aberta no PC do balcão o dia todo e olha de relance.
- **Cliente em decisão de compra**: vê a demonstração montada pelo técnico.
- **Técnico em estudo**: aprende sozinho em casa ou em curso.

## Product Purpose
Ferramenta open source que ensina instaladores de CFTV a validar inteligência artificial de vídeo num PC velho, aprendendo na prática, para depois venderem câmera ou gravador com IA de borda ou uma plataforma parceira de IA em nuvem. Sucesso: o técnico entende o que configurou, prova o valor ao cliente e fecha a venda da solução definitiva.

## Positioning
Nasceu da sala de aula de um ex-instrutor técnico da Intelbras. Cada configuração vira aula com a voz do autor; roda num computador velho, com um comando, sem nuvem. É a ponte para os parceiros.

## Operating Context
- Dashboard local no navegador, servida por um binário Rust de ~2 MB.
- Máquinas fracas e antigas, ligadas o dia todo, às vezes ao lado do PDV.
- Quiosque em TV (Smart TV, TV Box) e app Android via Tailscale.
- Alertas por Telegram e WhatsApp.

## Capabilities and Constraints
- Ver `docs/PRD.md` e `docs/sdd/`.
- Leveza é requisito número 1: sem framework de UI, sem fontes ou imagens pesadas, CSS e JS puros embutidos no binário.
- O cliente nunca vê caixas ou máscaras de detecção; só o balão com o nome da regra.
- Fora do escopo: painel multicliente, busca em gravações, ronda virtual, gravação contínua.

## Brand Commitments
- Nome: **CamFácil IA**. Agente de rede: **Agente-Cam-Elo-IA**.
- Paleta de origem: inspirada na Intelbras. Decisão do autor (2026-11-07): **base preta, verde só em detalhes**.
- Nada de visual "cara de IA" genérico (o autor rejeitou a primeira versão por isso).
- Projeto independente, sem vínculo com a Intelbras; não usar a marca nem o logo.
- Voz: instrutor amigo, frases curtas, pt-BR.

## Evidence on Hand
- Manifesto em `MANIFESTO.md`. Não há depoimentos, clientes ou números de uso: não inventar.

## Product Principles
1. Ensinar fazendo: toda tela explica o que está acontecendo.
2. Leve antes de bonito, e bonito dentro do leve.
3. Honestidade: aprendizagem e validação, nunca solução final.
4. O técnico é o dono da relação com o cliente; a marca dele aparece.

## Accessibility & Inclusion
Contraste AA, `prefers-reduced-motion`, uso em telas pequenas e em TV a distância.
