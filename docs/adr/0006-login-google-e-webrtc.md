# ADR-0006 — Login só com Google e vídeo por WebRTC direto

Status: aceito · 2026-11-07

## Contexto
O web app fica em `https://cfia.ysafe.com.br`, usado por muitos técnicos. Um técnico não pode acessar o agente de outro, e o servidor do projeto não deve tocar em vídeo nem em senhas de câmera (LGPD e custo).

## Decisão
1. **Login só com Google** (OpenID Connect). Nenhuma senha guardada; o técnico já usa a conta Google no Colab do modo estúdio.
2. **WebRTC direto** entre o navegador e o Agente-Cam-Elo-IA. O servidor só faz a sinalização.
3. **Chave por dispositivo** no navegador (WebCrypto, não exportável), autorizada no agente durante o pareamento por QR com confirmação local. O agente verifica uma assinatura amarrada às impressões DTLS antes de aceitar qualquer comando.
4. **Sem TURN** na v1. Acesso remoto só pela rede Tailscale do técnico.

## Consequências
- Um servidor comprometido não consegue comandar câmeras nem ver vídeo.
- Técnicos sem conta Google precisam criar uma.
- Acesso remoto exige Tailscale. Um TURN próprio pode ser avaliado depois, mas faria o vídeo passar pelo servidor do projeto.
- Detalhes em [SDD-08](../sdd/SDD-08-contas-e-isolamento.md).
