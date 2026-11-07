# PRD — CamFácil IA

| Campo | Valor |
|---|---|
| Versão | 0.1 (rascunho) |
| Autor | Emerson de Carvalho Assis |
| Data | 2026-11-07 |
| Licença | AGPL-3.0 |

## 1. Resumo

O CamFácil IA permite que instaladores de CFTV, mesmo leigos, rodem inteligência artificial de vídeo num computador velho com um único comando, aprendendo na prática enquanto configuram. É um **ambiente de aprendizagem e validação**: o técnico prova o valor da IA no cliente e depois vende a solução final — câmera ou gravador com IA de borda, ou uma plataforma corporativa de IA em nuvem (ex.: Zion, 3dEYE).

## 2. Problema

- Técnicos de segurança eletrônica têm dificuldade com IA de vídeo e com o básico de rede IP (o que é IP, conflito, sub-rede).
- Soluções de IA exigem hardware caro ou assinatura antes de o cliente ver valor.
- Não há ferramenta gratuita, leve e didática para demonstrar IA de vídeo no cliente.

## 3. Público

| Persona | Necessidade |
|---|---|
| **Técnico instalador (principal)** | Aprender IA e rede fazendo; demonstrar ao cliente; vender câmeras com IA |
| **Cliente final** (loja, bar, portaria) | Ver valor da IA no próprio negócio; receber alertas; tela de TV |
| **Comunidade dev** | Criar módulos e melhorar o núcleo |
| **Parceiros** (fabricantes, plataformas de IA) | Receber clientes já educados e validados |

## 4. Objetivos e métricas

| Objetivo | Métrica |
|---|---|
| Instalação trivial | Do download à primeira câmera com IA em < 10 min |
| Rodar em máquina velha | Núcleo ocioso < 50 MB RAM e < 2% CPU; binário < 30 MB |
| Ensinar | ≥ 70% dos técnicos concluem o 1º mini desafio |
| Gerar venda para parceiros | Relatório de validação gerado em ≥ 50% das instalações |
| Comunidade | ≥ 5 módulos criados por terceiros em 12 meses |

## 5. Escopo

### 5.1 Núcleo (v0.1)
- Binário único em Rust; instalação por comando ou release do GitHub (Windows, Linux x86_64, Linux ARM64; Android ARM64 depois).
- Servidor web local com a dashboard.
- Benchmark na instalação → libera níveis de IA (leve, médio, pesado).
- Teste de stress opcional → limite real (ex.: 4 câmeras com cerca).
- Câmeras: busca ONVIF na rede, gerador de RTSP por fabricante (Intelbras/Dahua, Hikvision e outros), stream principal/secundário/extra, botão de testar.
- Fontes alternativas: webcam, arquivo de vídeo, link de vídeo público, modo demonstração.

### 5.2 Módulos de IA (liga/desliga, download sob demanda)
| Nível | Módulos |
|---|---|
| Leve | Movimento inteligente; cerca/linha virtual |
| Médio | Pessoas e veículos (YOLO nano); contagem; permanência |
| Pesado | Leitura de placas; reconhecimento facial |
| Transversal | Saúde das câmeras (tapada, desfocada, girada, offline); mapa de calor; regras por horário; modo LGPD (borrar rostos no que sai da máquina) |

- **Cerca virtual genérica**: áreas com nome e estados editáveis (ex.: módulo Bar com mesas vazias/cheias).
- **Visual limpo**: sem caixas/máscaras do YOLO; só o **balão** com o nome da regra, que pisca e muda de cor no alerta.

### 5.3 Educação (o diferencial)
- Agente guia que destaca botões e explica em voz (áudios pré-gerados com a voz do autor, empacotados por módulo, funcionam offline).
- Toda configuração é uma aula: exemplo certo, erro comum, prática na câmera do técnico.
- Mini desafio + selo por módulo; ranking opcional; certificado "Instalador CamFácil IA".
- Ao fechar a instalação: **diagrama unifilar** da rede (roteador, switch, máquina, gravador, câmeras, IP/MAC) explicado em voz.

### 5.4 Rede e manutenção
- Inventário (IP, MAC, fabricante, modelo, firmware).
- Foto da rede saudável (IP+MAC); diagnóstico detecta câmera que trocou de IP e oferece corrigir o RTSP.
- Conflito de IP, câmera instável, senha padrão, banda por câmera; sugestão de plano de IPs.
- Monitor de hardware com log (o que pesou e quando), para justificar máquina nova ou câmeras de borda.
- Backup criptografado da configuração (manual, agendado, ou enviado ao Telegram/WhatsApp); instalação offline; atualização com rollback.

### 5.5 Comunicação e integrações
- **Telegram** (padrão, oficial) e **WhatsApp** (QR code, biblioteca não oficial, com aviso de banimento e recomendação de chip dedicado).
- Comandos v1: `foto <câmera>`, `status`, `liga/desliga <módulo>`, `resumo`, `backup` — só contatos autorizados.
- Alertas como texto simples (padrão), foto, clipe de ~10 s, ou descrição por IA (nuvem ou modelo local).
- Webhook por alerta (v1); MQTT como módulo futuro da comunidade.
- Provedores LLM: OpenRouter (gratuito, padrão, sem garantia), OpenAI, Google, Anthropic.
- **Servidor MCP** opcional: desligado por padrão, só via Tailscale, com token, leitura por padrão.

### 5.6 Saídas para o cliente
- **Módulo TV (quiosque)**: página em tela cheia com o que os módulos ativos produzem (ex.: status das mesas).
- **App Android** (técnico e cliente, permissões distintas): pareamento por QR, ao vivo, alertas com foto, liga/desliga, monitor de hardware, botões personalizáveis que disparam ações na rede local (portão, relé, webhook) com PIN/biometria e registro; modo snapshot automático em conexão fraca.
- **Marca do técnico**: logo no canto da dashboard e do quiosque + QR code para o WhatsApp dele.
- Relatório de validação em PDF e gerador de proposta.

### 5.7 Web app e instalação progressiva
1. Só navegador (WASM + ONNX Runtime Web): webcam, vídeo, demonstração.
2. Gateway mínimo: RTSP → WebRTC + busca ONVIF.
3. Instalação completa 24/7.

## 6. Fora do escopo (não competir com parceiros)
Painel multicliente, busca inteligente em gravações, ronda virtual, gravação contínua (NVR), monitoramento profissional 24h. Propostas nessa linha são recusadas em revisão.

## 7. Requisitos não funcionais
- **Leveza** é requisito número 1: toda dependência nova precisa justificar o custo em MB e CPU.
- Offline-first: nada depende de nuvem para funcionar.
- Segurança: segredos criptografados em repouso; dashboard exige senha; acesso remoto só via Tailscale.
- i18n: pt-BR primeiro; textos em arquivos de tradução.
- Acessibilidade: contraste AA, `prefers-reduced-motion`.

## 8. Roadmap
| Fase | Entrega |
|---|---|
| 1 — Núcleo | Binário, dashboard, benchmark, ONVIF, gerador RTSP, webcam/vídeo |
| 2 — Primeiro módulo | Cerca virtual + balão + aula guiada + mini desafio |
| 3 — Alertas | Telegram, depois WhatsApp; comandos; texto/foto/clipe |
| 4 — Rede | Inventário, foto da rede, diagnóstico, diagrama unifilar |
| 5 — Saídas | Módulo TV, monitor de hardware, relatório, backup |
| 6 — App e web | App Android, web app + gateway, MCP |

## 9. Riscos
| Risco | Mitigação |
|---|---|
| Banimento do WhatsApp | Telegram como padrão; aviso e chip dedicado |
| Máquinas fracas demais | Benchmark e níveis; teste de stress |
| Licença AGPL do YOLO | Projeto inteiro em AGPL-3.0 |
| Termos do YouTube | Priorizar vídeos próprios ou com permissão |
| LGPD | Modo LGPD, placa "ambiente monitorado", dados ficam locais |
| Concorrer com parceiros | Seção "Fora do escopo" como regra de revisão |

## 10. Questões em aberto
- Gravação dos áudios do agente: lista final de falas por módulo.
- Fabricantes adicionais no gerador RTSP além de Intelbras/Dahua e Hikvision.
- Formato do contrato de módulo para a comunidade (ver SDD-02).
