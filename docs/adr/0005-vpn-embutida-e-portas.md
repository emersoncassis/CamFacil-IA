# ADR-0005 — VPN embutida e escolha automática de portas

Status: proposto · 2026-11-07

## Contexto
A máquina do cliente costuma rodar PDV, ERP, banco de dados e NVR. Abrir portas no roteador é inseguro e muitas vezes impossível (CGNAT). Ocupar uma porta usada por outro software derruba o sistema do cliente.

## Decisão
1. **Portas**: o núcleo nunca assume uma porta fixa. Verifica disponibilidade antes de cada bind, evita uma lista negra de portas comuns de PDV/ERP/NVR e grava a porta escolhida (detalhes no SDD-01).
2. **VPN embutida**, em ordem de preferência:
   - Se o Tailscale já estiver instalado na máquina, usar a interface dele.
   - Caso contrário, nó Tailscale embutido no próprio processo, sem instalar cliente no sistema e sem abrir portas:
     - curto prazo: `tailscale-rs` (biblioteca Rust oficial, hoje em preview experimental, ainda sem conexão peer-to-peer, todo tráfego via DERP) atrás de uma feature flag;
     - alternativa estável: `tsnet` (Go) como processo auxiliar pequeno, se o preview não atender.
   - Pareamento do app por QR code com a chave de autenticação.
3. Revisar esta ADR quando o `tailscale-rs` sair do preview.

## Consequências
- Zero configuração de roteador; funciona atrás de CGNAT.
- No preview do `tailscale-rs`, o vídeo ao vivo remoto pode ficar limitado (DERP); o modo snapshot cobre esse caso.
- Dependência de conta Tailscale (gratuita para uso pessoal) ou de um servidor Headscale próprio.

Referência: [Tailscale — tailscale-rs preview](https://tailscale.com/blog/tailscale-rs-rust-tsnet-library-preview).
