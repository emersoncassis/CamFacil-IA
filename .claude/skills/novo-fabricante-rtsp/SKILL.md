---
name: novo-fabricante-rtsp
description: Adiciona um fabricante de câmera ou gravador ao gerador de RTSP do CamFácil IA. Use quando pedirem suporte a uma marca nova.
---

# Novo fabricante no gerador RTSP

1. Confirme o padrão de URL na documentação oficial do fabricante (stream principal, secundário e extra; canal; porta padrão 554). Cite a fonte no PR.
2. Adicione a entrada na tabela de fabricantes (`crates/cf-core/src/rtsp/vendors.rs` ou equivalente), com:
   - nome, aliases (marcas OEM que usam o mesmo padrão), template, numeração de canal e de stream.
3. Escreva testes de geração de URL (sem rede) para principal, secundário e canal > 1.
4. Se o fabricante tiver senha padrão conhecida e documentada publicamente, registre-a só para o alerta de "senha padrão" (nunca para tentar acesso automático sem o técnico pedir).
5. Commit: `feat(rtsp): suporte a <Fabricante>`.
