# SDD-05 — Rede, diagnóstico e manutenção

Status: proposto · Fase 4 e 5

## Inventário
ONVIF (fabricante, modelo, firmware) + tabela ARP (MAC) + varredura ping da sub-rede. Roda **só sob demanda**.

## Foto da rede saudável
Na primeira varredura com tudo funcionando, grava `{camera_id, ip, mac, fabricante, modelo}` como referência.

## Diagnóstico
1. Câmera sem resposta → busca o MAC dela na ARP atual.
2. MAC em outro IP → "A câmera X trocou de IP (de .101 para .117)" + botão "corrigir RTSP".
3. Dois MACs no mesmo IP / respostas alternadas → conflito de IP.
4. Também: câmeras instáveis (quedas por hora), senha padrão de fábrica, banda por câmera.
Limite: MAC só é visível na mesma sub-rede.

## Plano de IPs e diagrama unifilar
- Sugere faixas (ex.: câmeras .100–.150) e exporta documentação.
- Diagrama unifilar em SVG gerado no servidor: roteador → switch → máquina/gravador → câmeras, com IP e MAC.

## Monitor de hardware
- Amostra a cada 10 s (sysinfo): CPU, RAM, disco, temperatura e top 5 processos.
- Log compacto em SQLite com retenção (ex.: 30 dias, agregando por minuto após 24 h).
- Relatório "por que travou": picos e o processo responsável.

## Backup, atualização, offline
- Backup: `camfacil.toml` + SQLite (sem vídeos) → arquivo único cifrado (age/ChaCha20-Poly1305, senha do técnico).
- Atualização: baixa a release, valida assinatura, troca o binário e guarda o anterior para rollback com um clique.
- Pacote offline: binário + modelos + áudios num zip para pendrive.
