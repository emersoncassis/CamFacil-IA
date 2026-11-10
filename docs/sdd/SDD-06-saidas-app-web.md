# SDD-06 — Módulo TV, app Android e web app

Status: **implementado em grande parte** (fases 5 e 6; revisado em 2026-11-10). Não implementados: perfis técnico/cliente no app, IA no navegador (WASM), vídeo por WebRTC e o "web app em três degraus" como desenhado. Ver cada seção.

## Módulo TV (quiosque) — implementado (fase 5)
- Rota `/tv` em tela cheia, sem menus (`crates/camfacil/src/tv.rs`, `web/tv.*`). Mostra o **mosaico das câmeras escolhidas, com os balões das regras por cima** (sem caixas do YOLO), relógio, a logo do técnico e o QR do WhatsApp dele. O mosaico mostra até 9 câmeras por página; havendo mais, as páginas trocam sozinhas a cada 30 s.
- Configuração (quais câmeras, ordem, som, WhatsApp) em `settings.tv_config` e entra no backup; logo PNG até 200 KB em `data_dir/logo.png`. API: `GET/POST /api/tv`, `GET /api/tv/estado`, `/api/tv/logo.png`, `/api/tv/whatsapp.svg`.
- Atualização por **consulta a cada 1 s** (`/api/tv/estado`), não por WebSocket. Funciona em navegador de TV Box e Smart TV com navegador moderno. Som de alerta por regra (bipe, sino, alarme, gota) vale na TV.
- Não existem cartões de estado por módulo (ex.: "Mesa 1 — vazia") como desenhado antes; o que a TV mostra é o que as regras produzem sobre a imagem.

## App Android — implementado (fase 6; ADRs 0009, 0011, 0014)
- **Tecnologia decidida: Kotlin sem AndroidX com WebView** (ADR 0009); Tauri foi descartado. O APK sai no mesmo job de release do GitHub (`camfacil-android.apk`, mesma versão do `Cargo.toml`) e é baixado também pelo site.
- **Dois modos**: *Este aparelho é o servidor* (o núcleo Rust e um ffmpeg mínimo vão dentro do APK, `ServidorService` em primeiro plano, início no boot, endereço da rede da loja com QR e botão do Tailscale, ADR 0011) ou *Ver um servidor à distância* (WebView do programa de outro computador ou de `cfia.ysafe.com.br`).
- **IA no aparelho**: o nível médio (pessoas e veículos, YOLO) roda no Android com o ONNX Runtime próprio (ADR 0014). Webcam não existe no Android; fontes válidas: RTSP, arquivo e demonstração. Câmera do próprio celular como fonte: pendente.
- **Pareamento**: ler o QR da dashboard (Integrações → Tailscale), digitar o endereço ou usar o site. Testa `/api/saude` antes de salvar.
- **Alertas**: serviço em primeiro plano opcional (`AlertasService`) lendo `/api/eventos` a cada 10 s (sem push); notificação com o nome da regra e a foto crua da câmera, nunca caixas da IA.
- **Câmeras ao vivo** (`assets/cameras.html`): MJPEG do programa; **conexão fraca**: mede atraso e banda a cada 5 s, troca para foto a cada 3 s após 2 medições ruins e só volta após 3 boas; botões Automático / Ao vivo / Só foto.
- **Portão e relé** pelo menu (só no modo local): abre a rota `/botoes` no WebView; nada nativo.
- **Pendentes**: perfis **técnico** e **cliente** com permissões distintas (o app mostra o que o painel oferece, sem API de perfis); chave de assinatura fixa do APK (sem os secrets `CFIA_KEYSTORE_*` o CI assina com chave de debug e o Android não atualiza por cima); APK por arquitetura (hoje ~20 MB com a IA); notificações push; câmera do celular como fonte.
- **Botões personalizáveis — implementado (fase 6):**
  - Cadastro em Integrações → Portão e relé: nome, ícone (portão, relé, luz), URL `http(s)://`, método (GET/POST/PUT), usuário/senha opcionais (HTTP Basic e Digest) e corpo opcional. A ação inteira fica **cifrada** pelo cofre (`botao_acao_<id>`); a lista só mostra o endereço sem usuário, senha nem parâmetros. Máximo de 12 botões.
  - **PIN** de 4 a 8 números (PBKDF2-SHA256, 60 mil iterações, sal próprio; nunca em claro). 5 erros travam os botões por 5 min.
  - **Confirmação**: `POST /api/botoes/{id}/preparar` (PIN + nome de quem aciona) devolve um código de uso único válido por 20 s; só `POST /api/botoes/{id}/acionar` com esse código executa a chamada (timeout 8 s), feita pelo servidor local.
  - **Log**: eventos `acionamento` (botão · quem · de onde [painel/app, IP] · resultado) e `acionamento_negado` (PIN errado); `GET /api/botoes/log`. Cadastro, remoção e troca de PIN viram `botao_config`.
  - Tela para o celular: `GET /botoes` (HTML/CSS/JS puro embutido).
  - O MCP só lista botões, nunca aciona.

## Web app e Agente-Cam-Elo-IA — situação real
O desenho original previa três degraus (navegador puro com IA em WebAssembly, agente mínimo RTSP→WebRTC, instalação completa). O que existe:
1. **Navegador puro (WASM + ONNX Runtime Web): não implementado.** No navegador roda a Formação (simulação) e o Modo estúdio (rotular fotos); a IA de vídeo real roda no programa local.
2. **Agente-Cam-Elo-IA: implementado, mas como parte do programa** (`crates/camfacil/src/elo.rs`, `cf-core/src/elo.rs`), não como binário separado nem com go2rtc. Ele pareia por QR com confirmação local e manda ao site, a cada 30 s, só um **resumo sem imagem** (ADR 0007). **Não há ponte RTSP→WebRTC**: o "Ver ao vivo" do painel abre a página leve do próprio programa do cliente pelo endereço Tailscale (precisa de Tailscale no computador do cliente e no aparelho do técnico).
3. **Instalação completa 24/7**: é o programa local (Windows, Linux x86_64/ARM64, Android).
Hospedagem, login e isolamento entre técnicos: [SDD-08](SDD-08-contas-e-isolamento.md), em `https://cfia.ysafe.com.br`. A sinalização de WebRTC existe no servidor (relé de mensagens), mas **nenhum cliente a usa ainda**.

## Marca do técnico — parcial
Logo (PNG até 200 KB) e número de WhatsApp na configuração do Modo TV, exibidos no quiosque. O relatório em PDF leva o nome do técnico e do cliente; **a logo ainda não entra no PDF** nem no cabeçalho da dashboard. A proposta existe como a última seção do relatório de validação (SDD-05).

## Dispositivos
Tabela de papéis no [PRD §5.8](../PRD.md). Resumo técnico:
- **APK Android** (TV Box, Android TV, celular velho): implementado (acima). Falta validar com TV Box real e controle remoto.
- **Armbian** em TV Box: o binário Linux ARM64 do Raspberry e o `install.sh` já atendem; sem validação em aparelho real.
- **Smart TV Tizen/webOS**: só o quiosque `/tv` pelo navegador; não há app web empacotado.
