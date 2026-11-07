# ADR-0001 — Rust e binário único

Status: aceito · 2026-11-07

## Contexto
O produto precisa rodar em PC velho de balcão, mini PC, Raspberry e talvez TV Box, com instalação de um comando.

## Decisão
Escrever o núcleo em Rust e distribuir um binário único por plataforma (Windows x86_64, Linux x86_64, Linux ARM64; Android ARM64 depois), com a dashboard embutida.

## Consequências
- Baixo consumo, sem runtime (Node/Python/Docker).
- Compilação cruzada no CI.
- Curva de aprendizado maior para a comunidade, mitigada pelo contrato de módulos simples (SDD-02) e pelos agentes/skills do Claude Code no repositório.
