# ADR-0002 — Inferência com ONNX Runtime (ort) e licença AGPL-3.0

Status: aceito · 2026-11-07

## Contexto
Os modelos mais práticos de detecção (YOLO da Ultralytics) são AGPL-3.0. O projeto é open source e educativo. A referência conceitual de visão (Supervision, Roboflow) é Python, o que não serve ao requisito de leveza.

## Decisão
- Rodar modelos exportados para ONNX com a crate `ort`.
- Licenciar todo o projeto em **AGPL-3.0**, compatível com YOLO.
- Reimplementar em Rust só as utilidades necessárias (zonas, linhas, rastreamento simples), inspiradas no Supervision.

## Consequências
- Quem redistribuir com a própria marca precisa manter o código aberto.
- Modelos baixados sob demanda, com hash verificado.
