# SDD-01 — Núcleo

Status: proposto · Fase 1

## Objetivo
Um binário Rust único que sobe a dashboard local, mede a máquina, encontra câmeras e entrega quadros de vídeo para os módulos de IA, gastando o mínimo possível.

## Orçamento
| Recurso | Limite (ocioso, sem IA) |
|---|---|
| Binário | < 30 MB |
| RAM | < 50 MB |
| CPU | < 2% |
| Inicialização | < 1 s |

## Arquitetura

```
camfacil (binário)
├─ http        axum: dashboard estática (embutida no binário) + API REST + WebSocket de eventos
├─ store       SQLite (rusqlite, bundled): câmeras, regras, eventos, log de hardware
├─ bench       benchmark e teste de stress → nível liberado (leve/médio/pesado)
├─ discovery   ONVIF WS-Discovery (UDP multicast 3702), ARP/MAC, ping
├─ rtsp        gerador de URL por fabricante + teste de conexão
├─ ingest      fontes: RTSP, webcam, arquivo, URL, demo → quadros reduzidos (ex.: 640×360, 5 fps)
├─ modules     registro, download sob demanda e ciclo de vida dos módulos de IA (SDD-02)
├─ events      barramento interno (tokio broadcast) → dashboard, notifiers, webhook
├─ notify      Telegram, WhatsApp, webhook (SDD-04)
├─ hw          monitor de CPU/RAM/disco/temperatura + processos (sysinfo)
└─ guide       catálogo de aulas, áudios e desafios (SDD-03)
```

## Decisões principais
- **Runtime**: tokio com poucas threads (`worker_threads = min(2, núcleos)`); inferência em pool separado.
- **Vídeo**: decodificar só o stream secundário por padrão; amostrar quadros (não decodificar a 30 fps se a IA usa 5 fps). Primeira implementação via `ffmpeg` externo (baixado sob demanda) para máxima compatibilidade; avaliar decodificação nativa depois (ADR-0003).
- **Frontend**: HTML + CSS (tokens) + JS sem framework, embutido com `include_dir`/`rust-embed`. Sem build de Node no caminho crítico.
- **Configuração**: um único `camfacil.toml` + SQLite; tudo exportável para o backup.
- **Pastas**: `%PROGRAMDATA%\CamFacil` (Windows), `~/.local/share/camfacil` (Linux).
- **Portas sem conflito**: na instalação e a cada inicialização, o núcleo verifica quais portas estão livres antes de ocupar qualquer uma, para nunca conflitar com o PDV, o NVR ou outro software do cliente.
  - Ordem de tentativa: porta salva → 7070 → faixa 7071–7099 → porta livre escolhida pelo sistema (bind em `:0`).
  - Porta escolhida gravada em `camfacil.toml`; a dashboard mostra o endereço real e o QR de pareamento já usa essa porta.
  - Escuta só em `127.0.0.1` e na interface da VPN por padrão; expor na LAN é opção explícita.
  - Nunca usa portas comuns de PDV/ERP/NVR (lista negra configurável: 80, 443, 554, 1433, 3050, 3306, 5432, 8000, 8080, 37777…).
  - O mesmo vale para gateway, MCP e quiosque.
- **VPN embutida (sem abrir portas)**: acesso remoto pelo app Android e pelo técnico via Tailscale, sem port forwarding no roteador do cliente (ADR-0005).

## Instalação
- Linux: `curl -fsSL https://.../install.sh | sh` → baixa o binário da release, cria serviço systemd.
- Windows: `irm https://.../install.ps1 | iex` ou baixar `camfacil.exe` da release e dar dois cliques (abre o navegador em `http://localhost:7070`).
- Primeiro acesso: criar senha da dashboard → benchmark → busca de câmeras.

## API (rascunho)
| Método | Rota | Uso |
|---|---|---|
| GET | `/api/system` | hardware, nível liberado, versão |
| POST | `/api/bench` | roda benchmark / stress |
| GET | `/api/discover` | busca ONVIF + ARP |
| POST | `/api/rtsp/build` | gera URL por fabricante |
| POST | `/api/cameras` | adiciona câmera |
| GET | `/api/cameras/:id/snapshot` | foto atual |
| GET/POST | `/api/modules` | lista, liga, desliga |
| WS | `/ws/events` | eventos em tempo real |

## Critérios de aceite (fase 1)
- `camfacil` inicia em Windows 10 e Debian 12 x86_64 e Raspberry Pi OS 64 bits.
- Encontra uma câmera ONVIF na rede e gera o RTSP correto para Intelbras/Dahua e Hikvision.
- Mostra a imagem da câmera, da webcam e de um arquivo de vídeo.
- Benchmark retorna um nível e o grava.
- Metas de orçamento medidas e publicadas no CI.
