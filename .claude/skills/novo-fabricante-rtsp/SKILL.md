---
name: novo-fabricante-rtsp
description: Adiciona um fabricante de câmera ou gravador ao gerador de RTSP (e, se houver, aos eventos e nomes de canal) do CamFácil IA. Use quando pedirem suporte a uma marca nova.
---

# Novo fabricante

Hoje o gerador cobre dois padrões: **Realmonitor** (Intelbras e Dahua: `/cam/realmonitor?channel=N&subtype=S`) e **StreamingChannels** (Hikvision e HiLook: `/Streaming/Channels/N0S`). Muitas marcas OEM já entram por um deles.

1. Confirme o padrão de URL na documentação oficial do fabricante (stream principal, secundário e extra; canal; porta padrão 554). Cite a fonte no PR.
2. Marca que usa um padrão existente: acrescente o nome em `tambem` da entrada em `VENDORS` (`crates/cf-core/src/rtsp.rs`). Padrão novo: crie uma variante em `Pattern`, trate-a em `build`, `caminho_canal`, `fabricante_do_caminho` e `canal_da_url`, e acrescente a entrada em `VENDORS`.
3. Escreva testes de geração de URL (sem rede) para principal, secundário e canal > 1, no estilo dos testes de `rtsp.rs` (`intelbras_dahua`, `hikvision`).
4. Se a marca tem gravador com vários canais, confira `crates/cf-core/src/gravador.rs` (detecção por ONVIF, modelo e `DESCRIBE`) e `crates/cf-core/src/nomes_canais.rs` (nome real do canal pela API do fabricante, com Digest).
5. Se a marca tem API de eventos, veja `crates/cf-core/src/eventos_camera.rs` (`caminho`) e o módulo "Alertas da câmera" (`crates/camfacil/src/alertas_camera.rs`). Valide em equipamento real ou diga no PR que só foi testado com servidor simulado.
6. Não existe hoje o alerta de "senha padrão de fábrica" (SDD-05, item 4 pendente). Se for implementá-lo, use só senhas padrão documentadas publicamente pelo fabricante, e nunca para tentar acesso automático sem o técnico pedir.
7. Commit: `feat(rtsp): suporte a <Fabricante>`.
