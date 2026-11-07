# ADR 0007 · Resumo do agente no site

**Situação:** aceita · 2026-11-07

## Contexto

Depois de parear, o técnico não sabia o que o computador do cliente estava vendo: o site mostrava só "on-line" ou "desconectado".

## Decisão

O Agente-Cam-Elo-IA manda ao site, a cada 30 s e só quando pareado, um **resumo sem imagem**: nome e situação de cada câmera (no ar, quadros por segundo, motivo quando sem sinal, quantas regras), uso de CPU e memória, nome da máquina, sistema, versão, IA ligada e os 10 últimos alertas (câmera, regra, tipo, horário).

- O servidor aceita só esses campos, com tamanho limitado (`hub::limpar_estado`), e guarda apenas o último resumo (`agentes.estado`, migração 0002).
- A Row-Level Security já existente garante que só o técnico dono vê o resumo.
- Nunca vão para o site: vídeo, foto, endereço RTSP, usuário ou senha de câmera.

## Consequências

- O painel passa a ser útil de longe sem violar o offline-first: sem internet, o agente segue funcionando e o site mostra o último resumo com o horário.
- Nomes de câmera e de regra saem da máquina do cliente: o técnico deve evitar nomes com dados pessoais (documentado na LGPD).
- Custo no agente: uma leitura de sysinfo e de SQLite a cada 30 s; no servidor, um UPDATE por agente a cada 30 s.
