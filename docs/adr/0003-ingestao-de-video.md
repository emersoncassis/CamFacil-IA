# ADR-0003 — Ingestão de vídeo

Status: proposto · 2026-11-07

## Contexto
Câmeras de fabricantes variados usam RTSP com H.264/H.265. Decodificar é a parte mais cara antes da IA.

## Decisão (inicial)
- Usar o stream secundário por padrão e amostrar poucos quadros por segundo.
- Fase 1: `ffmpeg` como processo externo, baixado sob demanda, entregando quadros brutos reduzidos por pipe. Máxima compatibilidade, pouco código.
- Depois: avaliar `retina` (RTSP em Rust) + decodificação por hardware para eliminar o ffmpeg.

## Consequências
- Download adicional de ~20–80 MB na primeira câmera.
- Medir no CI o custo por câmera para manter o orçamento do SDD-01.
