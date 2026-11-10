# ADR-0003 — Ingestão de vídeo

Status: aceita e **implementada** (fase 1 em diante) · 2026-11-07 · revisada em 2026-11-10

> **Como ficou.** O `ffmpeg` externo por pipe é o caminho de vídeo de hoje (`crates/camfacil/src/video.rs`). Ajuste frente ao texto original: o programa **não baixa** o ffmpeg. Ele procura, nesta ordem, `CAMFACIL_FFMPEG`, `camfacil-dados/bin`, a pasta do executável e o PATH (no Windows, também o atalho do winget). Quem instala é o `install.sh` (apt, dnf ou pacman), o `install.ps1`/botão da dashboard no Windows (winget) ou o próprio técnico. No Android, um ffmpeg mínimo vai dentro do APK ([ADR 0011](0011-servidor-no-android.md)). A avaliação do `retina` com decodificação por hardware **não foi feita**; o custo por câmera é medido pelo `scripts/budget.sh` e pelo teste de estresse.

## Contexto
Câmeras de fabricantes variados usam RTSP com H.264/H.265. Decodificar é a parte mais cara antes da IA.

## Decisão (inicial)
- Usar o stream secundário por padrão e amostrar poucos quadros por segundo.
- Fase 1: `ffmpeg` como processo externo, baixado sob demanda, entregando quadros brutos reduzidos por pipe. Máxima compatibilidade, pouco código.
- Depois: avaliar `retina` (RTSP em Rust) + decodificação por hardware para eliminar o ffmpeg.

## Consequências
- Download adicional de ~20–80 MB na primeira câmera.
- Medir no CI o custo por câmera para manter o orçamento do SDD-01.
