# ADR-0004 — WhatsApp por biblioteca não oficial

Status: substituída pela ADR-0010 (Evolution API como gateway HTTP) · 2026-11-07

## Contexto
A Evolution API depende de Node/Docker, pesada para máquina velha. A API oficial do WhatsApp exige empresa verificada e custo por mensagem.

## Decisão
- Telegram é o canal padrão.
- WhatsApp como módulo opcional por QR code: tentar `whatsapp-rust`; se instável, usar `whatsmeow` (Go) como processo auxiliar pequeno.
- Aviso obrigatório: não oficial, risco de banimento, usar chip dedicado.

## Consequências
- Possíveis quebras quando o WhatsApp muda o protocolo; o módulo pode ser desligado sem afetar o resto.
