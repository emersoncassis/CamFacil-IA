# SDD-01 — Núcleo

Status: **implementado** (fase 1 e evoluções até a 0.9.x) · revisado em 2026-11-10

## Objetivo
Um binário Rust único que sobe a dashboard local, mede a máquina, encontra câmeras e entrega quadros de vídeo para a IA, gastando o mínimo possível.

## Orçamento
| Recurso | Limite (ocioso, sem IA) |
|---|---|
| Binário | < 30 MB |
| RAM | < 50 MB |
| CPU | < 2% |
| Inicialização | < 1 s |

O orçamento é medido por `scripts/budget.sh` e roda no CI (job `budget`). A IA de nível médio custa o que custa **enquanto está ligada** (números em [SDD-02](SDD-02-modulos-ia.md)); o ocioso continua dentro do limite.

## Arquitetura (como está no código)

```
camfacil (binário, crates/camfacil)
├─ http         axum (HTTP/1, sem TLS): dashboard embutida no binário (web/) + API REST (api.rs) + rotas /tv, /botoes, /mcp
├─ store        SQLite (rusqlite, bundled) em cf-core: câmeras, regras, eventos, configurações, aceites, selos, arquivo de alertas
├─ bench        benchmark e teste de estresse (estresse.rs) → nível liberado (leve/médio/pesado)
├─ discovery    ONVIF WS-Discovery, tabela ARP (Linux e Windows/macOS), ping; inventário e foto da rede (SDD-05)
├─ rtsp         gerador de URL por fabricante + teste de conexão (cf-core/rtsp.rs); gravador DVR/NVR (gravador.rs)
├─ video        ffmpeg externo: RTSP, webcam, arquivo, link e câmera de demonstração → quadros reduzidos; visor do stream principal sob demanda (visor.rs)
├─ ia           analisador leve (cf-core/visao.rs) + detector ONNX (ia/detector.rs, ort load-dynamic) → rastreio, regras e inteligências por regra (SDD-02, SDD-09)
├─ eventos      cada thread de vídeo grava os eventos na tabela `eventos` do SQLite (`registrar_evento`); dashboard, app e MCP leem por `/api/eventos` (sem WebSocket local)
├─ notify       cf-notify: Telegram, WhatsApp (Evolution API), webhook, cofre cifrado (SDD-04)
├─ hardware     monitor de CPU/RAM/disco/temperatura com log em banco próprio (SDD-05)
├─ elo          Agente-Cam-Elo-IA: pareamento por QR e resumo ao painel (SDD-08)
└─ tv, botoes, backup, relatorio, mcp, tailscale, ngrok, autoinicio, filhos, estudio (SDD-05, 06, 07)
```

Não existe módulo `guide`: a instrução fica no site ([SDD-03](SDD-03-educacao.md), ADR 0008). Os quadros circulam por um canal `broadcast` de JPEG por câmera (MJPEG, foto e IA leem dele); os eventos vão para o SQLite e são consultados por `/api/eventos` (o app Android lê a cada 10 s). Não há `/ws/events` no programa local: o WebSocket é só do agente para o site (SDD-08).

## Decisões principais
- **Runtime**: tokio com poucas threads; a inferência roda numa thread própria (uma sessão ONNX compartilhada entre câmeras, no máximo ~3 inferências por segundo no total).
- **Vídeo**: `ffmpeg` externo (ADR-0003, aceita). Decodifica só o stream secundário/extra por padrão, em quadro reduzido; o stream principal só abre sob demanda para mostrar imagem (`?stream=principal`), sem análise. O programa não baixa o ffmpeg: ele é instalado pelo `install.sh` (apt/dnf/pacman), pelo `install.ps1` ou pelo botão da dashboard no Windows (winget), ou fica em `camfacil-dados/bin`, ao lado do executável ou no PATH; no Android vem embutido como biblioteca nativa (ADR 0011).
- **Frontend**: HTML + CSS (tokens) + JS sem framework, embutido no binário. Sem build de Node no caminho crítico. Textos em `web/i18n/pt-BR.json`.
- **Configuração**: **não existe `camfacil.toml`**. Tudo fica no SQLite (`settings` e tabelas) dentro da pasta de dados, e o backup cifrado (`.cfbkp`, SDD-05) exporta o que importa.
- **Pastas**: portátil, `camfacil-dados` **ao lado do executável** (nada em pasta escondida nem no registro). Bancos de versões até a 0.7 (Windows `%PROGRAMDATA%\CamFacil`, Linux `~/.local/share/camfacil`) são reconhecidos e "Apagar todos os dados" também os limpa. `CAMFACIL_DATA` troca a pasta.
- **Portas sem conflito** (ADR-0005): na instalação e a cada inicialização, o núcleo verifica quais portas estão livres antes de ocupar qualquer uma, para nunca conflitar com o PDV, o NVR ou outro software do cliente.
  - Ordem de tentativa: porta salva (`settings.port`) → 7070 → faixa 7071–7099 → porta livre escolhida pelo sistema (bind em `:0`).
  - A dashboard mostra o endereço real e o QR de pareamento já usa essa porta.
  - Escuta só em `127.0.0.1` por padrão; `--lan` escuta na rede local; uma vigília abre um segundo listener no IP do Tailscale sem expor a LAN (SDD-05).
  - Nunca usa portas comuns de PDV/ERP/NVR (lista negra em `cf-core/src/ports.rs`: 21, 22, 23, 25, 53, 80, 110, 135, 139, 143, 443, 445, 554, 1433, 1521, 3050, 3306, 3389, 5432, 5900, 6379, 8000, 8080, 8443, 8554, 9000, 27017, 37777…).
  - O mesmo vale para o MCP e o túnel só do MCP.
- **Acesso remoto sem abrir portas**: Tailscale já instalado na máquina (ADR-0005, SDD-05); ngrok só para testes; MCP por HTTPS via Serve/Funnel (ADR-0015).
- **Sem senha de dashboard.** A dashboard local não tem login: ela escuta só em `127.0.0.1` por padrão, e quem a expõe (`--lan`, Tailscale, ngrok) está dentro da rede do técnico ou, no ngrok, atrás de basic-auth com senha aleatória. Uma senha local própria é decisão em aberto (PRD §10).
- **Processos filhos** (ffmpeg, ngrok) morrem junto com o programa, mesmo se ele for fechado à força: Job Object no Windows, sinal do sistema no Linux/Android (`filhos.rs`).
- **Primeira execução**: aceite da isenção de responsabilidade (versão e data gravadas), depois teste da máquina.

## Instalação
- Linux/Raspberry: `curl -fsSL https://cfia.ysafe.com.br/instalar.sh | sh` (`scripts/install.sh`): baixa o binário x86_64 ou ARM64 da release, instala o ffmpeg se faltar, liga o "abrir junto com o computador" e abre a dashboard.
- Windows 10/11: `irm https://cfia.ysafe.com.br/instalar.ps1 | iex` (`scripts/install.ps1`): baixa o programa, instala o ffmpeg se faltar, cria o atalho e abre a dashboard. Também dá para baixar o `camfacil.exe` e dar dois cliques.
- Android: `camfacil-android.apk` da release (SDD-06).
- O site baixa arquivos com a versão no nome (`camfacil-0.9.7-windows-x86_64.exe`) e a release traz `SHA256SUMS.txt`.
- Opções do binário: `--lan`, `--no-browser`, `--mcp`, `--autoiniciar`, `--version`.
- Abrir junto com o computador: chave Run do usuário (Windows), `~/.config/autostart` (Linux com tela) ou serviço systemd do usuário já com `--lan` (Raspberry/servidor). Desinstalar fica em Manutenção › Dados.
- Primeiro acesso: aceite → teste da máquina → busca de câmeras.

## API local
O programa tem ~115 rotas em `crates/camfacil/src/api.rs` (e módulos). As principais:

| Grupo | Rotas |
|---|---|
| Máquina | `GET /api/system`, `/api/estado`, `POST /api/bench`, `/api/estresse`, `/api/hardware`, `/api/hardware/log` |
| Câmeras | `/api/procurar` e `/api/discover` (busca ONVIF), `/api/rtsp/{vendors,build,test}`, `POST /api/conectar` (um clique), `POST /api/gravador/adicionar`, `/api/cameras`, `/api/cameras/{id}/{foto.jpg,mjpeg,regras,nomes-canais}` |
| IA | `/api/modulos`, `/api/inteligencias`, `/api/regras/{id}`, `/api/ia` (+ `baixar`, `cancelar`), `/api/saude`, sugestões e avaliação de regra |
| Rede | `/api/rede/*` (resumo, fotos, diagnóstico, diagrama.svg), `/api/tailscale`, `/api/ngrok`, `/api/elo*` |
| Saídas | `/api/telegram*`, `/api/whatsapp*`, `/api/webhook*`, `/api/arquivo*`, `/tv` e `/api/tv*`, `/botoes` e `/api/botoes*`, `/mcp` e `/api/mcp*` |
| Manutenção | `/api/backup`, `/api/relatorio*`, `/api/termos*`, `/api/dados`, `/api/dados/apagar-tudo`, `/api/desinstalar`, `/api/autoinicio` |
| Estúdio | `/api/estudio/*` ([SDD-07](SDD-07-modo-estudio.md)) |

## Critérios de aceite (fase 1)
- `camfacil` inicia em Windows 10, Debian/Ubuntu x86_64 e Raspberry Pi OS 64 bits (o CI compila os três alvos e o Android). **Falta validar em máquina real**: Windows, Raspberry e TV Box.
- Encontra uma câmera ONVIF na rede e gera o RTSP correto para Intelbras/Dahua e Hikvision (testes sem rede; falta equipamento real).
- Mostra a imagem da câmera, da webcam, de um arquivo de vídeo e da demonstração (MJPEG e foto).
- Benchmark retorna um nível e o grava.
- Metas de orçamento medidas e publicadas no CI.

## Estado da implementação (2026-11-10)
| Item | Situação |
|---|---|
| Workspace (`camfacil`, `cf-core`, `cf-notify`, `cfia-api`), binário único, dashboard embutida | feito |
| Porta livre com lista negra (ADR-0005) | feito, com testes |
| `/api/system`, `/api/bench`, `/api/rtsp/*`, `/api/discover`, `/api/conectar` | feito |
| Aceite da isenção na primeira execução | feito |
| Pré-visualização (MJPEG, foto, stream principal sob demanda) | feito |
| ARP/MAC no Linux e no Windows/macOS (`arp -a`) | feito |
| Instaladores `install.sh` e `install.ps1`, abrir junto com o computador, desinstalar | feito |
| Teste de estresse (quantas câmeras a máquina aguenta) | feito |
| Orçamento medido no CI (`scripts/budget.sh`) | feito; última medição publicada: 4,6 MB de binário · 6,4 MB de RAM · 0% de CPU ocioso (SDD-02, 2026-11-07) |
| Benchmark | **ainda provisório**: kernel de ponto flutuante em `cf-core/src/bench.rs`. A troca pela inferência real de um modelo de referência (SDD-02, "Níveis e benchmark") segue pendente; o teste de estresse mede o que o PC aguenta de verdade |
| Dashboard com senha própria | não implementado (ver acima) |
| Validação em Windows, Raspberry e TV Box reais | pendente |
