# PRD — CamFácil IA

| Campo | Valor |
|---|---|
| Versão do documento | 0.2 (revisado contra o código da versão 0.9.7) |
| Autor | Emerson de Carvalho Assis |
| Data | 2026-11-07 · revisão 2026-11-10 |

> **Como ler os status.** Onde aparece **[feito]**, está no código e testado; **[parcial]** tem uma parte feita; **[pendente]** ainda não existe. O texto original do PRD é a intenção do produto; os SDDs dizem como cada parte ficou.
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
- Binário único em Rust; instalação por comando ou release do GitHub (Windows, Linux x86_64, Linux ARM64) **[feito]**; Android ARM64 e ARMv7 pelo APK **[feito]**.
- Servidor web local com a dashboard **[feito]**, numa porta livre que não conflita com PDV/NVR.
- Benchmark na instalação → libera níveis de IA (leve, médio, pesado) **[parcial]**: o teste é uma estimativa por ponto flutuante; a inferência real de referência está pendente (SDD-01).
- Teste de stress opcional → limite real (ex.: 4 câmeras com cerca) **[feito]**.
- Câmeras: busca ONVIF na rede, gerador de RTSP por fabricante (Intelbras/Dahua e Hikvision/HiLook **[feito]**; outras marcas pendentes), stream principal/secundário/extra, botão de testar **[feito]**. Gravador DVR/NVR com escolha de canais e nome real do canal **[feito]**.
- Fontes alternativas: webcam, arquivo de vídeo e modo demonstração (a loja da Formação) **[feito]**; link de vídeo público/YouTube **[pendente]** como fonte dedicada.

### 5.2 Módulos de IA (liga/desliga, download sob demanda)
| Nível | Módulos |
|---|---|
| Leve | Movimento inteligente; cerca/linha virtual |
| Médio | Pessoas e veículos (YOLO nano); contagem; permanência |
| Pesado | Leitura de placas; reconhecimento facial |
| Transversal | Saúde das câmeras (tapada, escura, desfocada, sem sinal **[feito]**; girada/movida **[pendente]**); mapa de calor **[feito]**; regras por horário **[pendente]**; modo LGPD (borrar rostos no que sai da máquina) **[pendente]**; alertas da própria câmera (Intelbras/Dahua e Hikvision) **[feito]** |

Na prática, a IA de nível médio é **um módulo só** ("Pessoas e veículos", YOLO11n ONNX) e o técnico escolhe, **por regra**, uma inteligência (passagem, presença, densidade, permanência, mapa de calor) e quais classes observar (pessoa, carro, moto, bicicleta, ônibus, caminhão), com cor e som próprios ([SDD-09](sdd/SDD-09-inteligencias-por-regra.md)). Nada vem ligado por padrão. "IA, sugira para mim" e "IA, avalie a minha regra" ajudam o técnico a desenhar e conferir as regras.

- **Cerca virtual genérica**: áreas com nome e estados editáveis (ex.: módulo Bar com mesas vazias/cheias).
- **Visual limpo**: sem caixas/máscaras do YOLO; só o **balão** com o nome da regra, que pisca e muda de cor no alerta.

### 5.3 Educação (o diferencial)
- **Decisão de 2026-11-07 (ADR 0008):** a instrução vive no site, na **Formação de IA para CFTV** (`/aprender`, com a voz do autor, exercícios no navegador e selo **[feito]**). O programa local é só operação, sem agente guia com voz embutido; os botões **Aprender** abrem a lição certa no site. O agente guia com voz dentro do programa, a aula por módulo e o `desafio.toml` ficaram de lado.
- Selo da Formação **[feito]**; selo de operação (`cerca`) **[feito]**; ranking e certificado "Instalador CamFácil IA" **[pendente]**.
- Ao fechar a instalação: **diagrama unifilar** da rede (roteador, switch, máquina, gravador, câmeras, IP/MAC) em SVG **[feito]**, sem narração em voz **[pendente]**.

### 5.4 Rede e manutenção
- Inventário (IP, MAC, fabricante, modelo, firmware).
- Foto da rede saudável (IP+MAC); diagnóstico detecta câmera que trocou de IP e oferece corrigir o RTSP **[feito]**; **autocorreção de IP pelo MAC** (opção por câmera, com conferência da imagem) **[feito]**.
- Conflito de IP e câmera instável **[feito]**; senha padrão de fábrica e banda por câmera **[pendente]**; sugestão de plano de IPs **[pendente]**.
- Monitor de hardware com log (CPU, RAM, temperatura, disco e quadros por câmera, 30 dias) e alertas **[feito]**; "por que travou" com o processo responsável **[pendente]**.
- Backup criptografado da configuração (manual e pelo Telegram `/backup`) **[feito]**; agendado e por WhatsApp **[pendente]**; instalação offline em pacote e atualização com rollback **[pendente]**.

### 5.5 Comunicação e integrações
- **Telegram** (padrão, oficial) **[feito]** e **WhatsApp** pela **Evolution API** (ADR 0010, que substituiu a biblioteca não oficial; QR code, aviso de banimento e chip dedicado) **[feito]**.
- Comandos v1: `foto <câmera>`, `status`, `liga/desliga <módulo>`, `resumo` nos dois canais e `backup` no Telegram — só contatos autorizados **[feito]**.
- Alertas como texto simples (padrão) e foto **[feito]**; clipe de ~10 s e descrição por IA (nuvem ou modelo local) **[pendente]**. Arquivo de alertas num canal privado do Telegram **[parcial]**.
- Webhook por alerta **[feito]**; MQTT como módulo futuro da comunidade.
- Provedores LLM (OpenRouter, OpenAI, Google, Anthropic) **[pendente]**: não há integração direta; a conversa com IA acontece pelo MCP.
- **Servidor MCP** opcional **[feito]**: desligado por padrão, com token, leitura por padrão e escrita só se o técnico permitir (ADR 0012); endereço HTTPS para o Claude por Tailscale Serve/Funnel, ou ngrok só para testes (ADR 0015).

### 5.6 Saídas para o cliente
- **Módulo TV (quiosque)** **[feito]**: página em tela cheia com o mosaico das câmeras e os balões das regras, relógio, logo e QR do WhatsApp do técnico.
- **App Android** **[feito, sem perfis]**: servidor no próprio aparelho ou visualizador à distância, pareamento por QR, ao vivo com modo foto automático em conexão fraca, alertas com foto, botões de portão e relé com PIN e registro. **Perfis técnico e cliente com permissões distintas [pendente]**; biometria [pendente]; monitor de hardware no app [pendente].
- **Marca do técnico** **[parcial]**: logo e QR do WhatsApp na TV; o PDF leva o nome do técnico e do cliente, sem logo.
- Relatório de validação em PDF com proposta na última seção **[feito]**.

### 5.7 Web app e instalação progressiva
1. Só navegador (WASM + ONNX Runtime Web): webcam, vídeo, demonstração **[pendente]**. No navegador existem a Formação (simulação) e o modo estúdio.
2. **Agente-Cam-Elo-IA** **[parcial]**: existe **dentro do programa local** (pareia por QR com confirmação local e manda ao painel só um resumo sem imagem, ADR 0007). A ponte RTSP → WebRTC **[pendente]**; o "ver ao vivo" do painel abre a página do programa do cliente pelo Tailscale.
3. Instalação completa 24/7 **[feito]**.

Web app em **https://cfia.ysafe.com.br** (Dokploy), com login Google e isolamento por técnico: [SDD-08](sdd/SDD-08-contas-e-isolamento.md).

### 5.8 Dispositivos e papéis
| Dispositivo | Papel | IA |
|---|---|---|
| PC ou mini PC | Cérebro completo | Todos os níveis que o benchmark liberar |
| Raspberry Pi | Cérebro compacto | Leve e médio |
| TV Box (APK ou Armbian) | Quiosque + Agente-Cam-Elo-IA + IA leve | Leve; pessoas só em 1 câmera, poucos fps |
| Smart TV (Tizen, webOS) | Só tela (quiosque) | Nenhuma |
| Android TV / Google TV | Tela + IA leve (mesmo APK) | Leve |
| Celular velho | Agente-Cam-Elo-IA, IA em 1–2 câmeras, câmera de demonstração, mini arquivo de clipes | Leve a médio |

- Android: serviço em primeiro plano, início automático, exceção na economia de bateria; preferir aparelhos homologados pela Anatel.
- Celular 24h na tomada: limitar carga (~80%) ou tomada inteligente; lugar ventilado.
- Mini arquivo de clipes no celular: só clipes de alertas, limpeza automática, cifrados, cartão "endurance". Não é gravação contínua.

### 5.9 Modo estúdio (estudo)
Criar inteligências novas conversando com a IA (Anthropic por padrão), com treino opcional no Google Colab e web app no domínio do projeto (Dokploy). Somente estudo. Detalhes no [SDD-07](sdd/SDD-07-modo-estudio.md).

### 5.10 LGPD e responsabilidades
Modelos de termo, placa e isenção em [`docs/legal/`](legal/README.md). Aceites registrados com data, hora e versão.

## 6. Fora do escopo (não competir com parceiros)
Painel multicliente, busca inteligente em gravações, ronda virtual, gravação contínua (NVR), monitoramento profissional 24h. Propostas nessa linha são recusadas em revisão.

## 7. Requisitos não funcionais
- **Leveza** é requisito número 1: toda dependência nova precisa justificar o custo em MB e CPU.
- Offline-first: nada depende de nuvem para funcionar.
- Segurança: segredos criptografados em repouso (cofre ChaCha20-Poly1305) **[feito]**; a dashboard escuta só em `127.0.0.1` por padrão e **não tem senha própria [pendente]**; acesso remoto por Tailscale, com ngrok temporário (basic-auth) só para testes.
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
| 6 — App e web | App Android, web app + Agente-Cam-Elo-IA, MCP |
| 7 — Modo estúdio | Conversa com IA, vocabulário aberto, Colab |

**Situação em 2026-11-10 (versão 0.9.7):** fases 1 a 6 entregues, com as lacunas marcadas em cada seção acima (clipes, perfis no app, WebRTC, WASM, senha padrão, plano de IPs, atualização com rollback). Fase 7: versão básica (descrever, coletar, rotular, treinar no Colab, importar, rodar e compartilhar receitas); faltam a conversa com a IA, o vocabulário aberto e a pré-marcação. Depois das fases: inteligências por regra (SDD-09, 0.9.x, feito) e IA avançada (SDD-10, proposta).

## 9. Riscos
| Risco | Mitigação |
|---|---|
| Banimento do WhatsApp | Telegram como padrão; aviso e chip dedicado |
| Máquinas fracas demais | Benchmark e níveis; teste de stress |
| Licença AGPL do YOLO | Projeto inteiro em AGPL-3.0 |
| Termos do YouTube | Priorizar vídeos próprios ou com permissão |
| LGPD | Modo LGPD, placa, termos de autorização e responsabilidade, dados ficam locais, projeto não hospeda imagens |
| Modelos de estudo usados em produção | Aviso em todo modelo exportado e no modo estúdio |
| Concorrer com parceiros | Seção "Fora do escopo" como regra de revisão |

## 10. Questões em aberto
- Gravação dos áudios: as lições atuais da Formação já têm narração; lições novas (rede, regras por classe) precisam de falas novas.
- Fabricantes adicionais no gerador RTSP além de Intelbras/Dahua e Hikvision.
- Formato do contrato de módulo para a comunidade (ver SDD-02): o contrato por pasta não foi implementado; a via atual é a receita do modo estúdio.
- Senha própria para a dashboard local e perfis técnico/cliente no app.
- Validação em campo: autocorreção de IP, DVR/NVR, alertas da câmera, Tailscale, Telegram real, WhatsApp com Evolution real, Windows e TV Box.
