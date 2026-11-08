# SDD-05 — Rede, diagnóstico e manutenção

Status: proposto · Fase 4 e 5

## Inventário (status: implementado · fase 4)
ONVIF (fabricante, modelo, firmware) + tabela ARP (MAC) + varredura ping da sub-rede. Roda **só sob demanda**.

## Foto da rede saudável
Na primeira varredura com tudo funcionando, grava `{camera_id, ip, mac, fabricante, modelo}` como referência.

## Diagnóstico (status: itens 1, 2 e 3 implementados · fases 4 e 6; 4 pendente)
1. Câmera sem resposta → busca o MAC dela na ARP atual.
2. MAC em outro IP → "A câmera X trocou de IP (de .101 para .117)" + botão "corrigir RTSP".
3. Dois MACs no mesmo IP / respostas alternadas → conflito de IP. **Implementado (fase 6):** `cf_core::rede::conflitos_ip` olha as últimas 6 fotos da rede: conflito na mesma varredura (MAC do ARP ≠ MAC do ONVIF, campo `outros_macs`) ou entre varreduras (o IP alterna de dono duas vezes ou mais e o MAC antigo não está noutro IP — troca simples de dono pelo DHCP não conta). `GET /api/rede/foto` devolve `conflitos`; a tela Rede mostra o aviso e marca a linha do inventário.
4. Também: câmeras instáveis (quedas por hora), senha padrão de fábrica, banda por câmera.
Limite: MAC só é visível na mesma sub-rede.

## Plano de IPs e diagrama unifilar (status: diagrama implementado · fase 4; plano de IPs pendente)
- Sugere faixas (ex.: câmeras .100–.150) e exporta documentação.
- Diagrama unifilar em SVG gerado no servidor: roteador → switch → máquina/gravador → câmeras, com IP e MAC.
  Implementação: `cf-core/src/unifilar.rs`, `GET /api/rede/diagrama.svg?foto=ID&baixar=true`. Na tela usa os tokens; baixado, cores claras de papel. Ligações abaixo do roteador são presumidas (a varredura não vê cabos) e o carimbo avisa.

## Monitor de hardware
- Amostra a cada 10 s (sysinfo): CPU, RAM, disco, temperatura e top 5 processos.
- Log compacto em SQLite com retenção (ex.: 30 dias, agregando por minuto após 24 h).
- Relatório "por que travou": picos e o processo responsável.

## Backup, atualização, offline
- Backup: `camfacil.toml` + SQLite (sem vídeos) → arquivo único cifrado (age/ChaCha20-Poly1305, senha do técnico).
- Atualização: baixa a release, valida assinatura, troca o binário e guarda o anterior para rollback com um clique.
- Pacote offline: binário + modelos + áudios num zip para pendrive.

## Acesso de fora pelo Tailscale (status: implementado · fase 3)
Primeira opção da ADR-0005: usar o cliente Tailscale já instalado na máquina (sem nó embutido, sem porta no roteador).

- `crates/camfacil/src/tailscale.rs`: acha o binário (PATH; Windows `C:\Program Files\Tailscale\tailscale.exe`; macOS `/Applications/Tailscale.app/...`), roda `tailscale status --json` (fallback `tailscale ip -4`) com timeout de 2,5 s, fora da thread da UI.
- **Escuta**: por padrão o servidor escuta só em `127.0.0.1`. Uma vigília leve (a cada 60 s; sem Tailscale instalado não roda processo nenhum) descobre o IP Tailscale e abre um segundo listener nesse IP, na mesma porta, **sem reiniciar** e sem expor a LAN. Com `--lan` o servidor já escuta em `0.0.0.0` e a vigília não faz nada. Se o IP mudar, o próximo início faz o bind no novo IP.
- **API**: `GET /api/tailscale` → `{instalado, logado, ip, nome, url, url_nome, escutando, aguardando, lan, erro, comando_instalar, sistema}`; `GET /api/tailscale/qr.svg` (reaproveita o gerador de QR do elo).
- **UI** (Integrações → "Acessar de fora (Tailscale)"): situação, endereço `http://<ip>:<porta>` com copiar, nome MagicDNS, QR, instruções por sistema (Windows `winget install tailscale.tailscale`; Linux/Raspberry `curl -fsSL https://tailscale.com/install.sh | sh` + `sudo tailscale up`; macOS `brew install --cask tailscale`; celular: app Tailscale com a mesma conta). Explica que o vídeo não passa pela nuvem e que o celular precisa estar na mesma rede Tailscale.
- Falta validar numa máquina real com Tailscale (aqui foi testado com um binário simulado respondendo `status --json` e `ip -4`, e o bind em um segundo IP).
