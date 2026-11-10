# SDD-05 — Rede, diagnóstico e manutenção

Status: **implementado em grande parte** (fases 3 a 6; revisado em 2026-11-10). Pendentes: plano de IPs, senha padrão de fábrica, quedas por hora, banda por câmera, "por que travou" com o processo responsável, atualização com rollback e pacote offline.

## Inventário (status: implementado · fase 4)
ONVIF (fabricante, modelo, firmware) + tabela ARP (MAC) + varredura ping da sub-rede. Roda **só sob demanda**.

## Foto da rede saudável
Na primeira varredura com tudo funcionando, grava `{camera_id, ip, mac, fabricante, modelo}` como referência.

## Diagnóstico (status: itens 1, 2 e 3 implementados · fases 4 e 6; 4 parcial)
1. Câmera sem resposta → busca o MAC dela na ARP atual.
2. MAC em outro IP → "A câmera X trocou de IP (de .101 para .117)" + botão "corrigir RTSP".
3. Dois MACs no mesmo IP / respostas alternadas → conflito de IP. **Implementado (fase 6):** `cf_core::rede::conflitos_ip` olha as últimas 6 fotos da rede: conflito na mesma varredura (MAC do ARP ≠ MAC do ONVIF, campo `outros_macs`) ou entre varreduras (o IP alterna de dono duas vezes ou mais e o MAC antigo não está noutro IP — troca simples de dono pelo DHCP não conta). `GET /api/rede/foto` devolve `conflitos`; a tela Rede mostra o aviso e marca a linha do inventário.
4. Também: câmeras instáveis (quedas por hora), senha padrão de fábrica, banda por câmera. **Parcial:** o diagnóstico de hoje (`cf_core::rede`) devolve `tudo_ok`, `sem_resposta`, `mudou_ip`, `fora_da_faixa`, `gateway_fora`, `dns_falhou`, `lenta` e `instavel` (instável = perdeu N de 3 tentativas, não "quedas por hora"). **Senha padrão de fábrica e banda por câmera não existem.**
Limite: MAC só é visível na mesma sub-rede.

## Autocorreção de IP pelo MAC (status: implementado · fase 6)
Opção por câmera RTSP (`cameras.auto_ip`, padrão desligado; em gravador, por canal) para o programa trocar sozinho o host da URL RTSP quando a câmera (mesmo MAC, `cameras.mac`) aparece noutro IP. Vale para Windows, Linux e Android (mesmo núcleo e mesma interface web).
- **Controle**: chave "Corrigir o IP sozinho se a câmera mudar de endereço" no Ligar câmera e no Editar câmera. Escondida para demo, webcam e arquivo; travada com "Aguardando a primeira varredura da rede" enquanto o MAC não foi aprendido. O MAC é aprendido na conexão (ARP logo depois do `POST /api/conectar`, ou o MAC do ONVIF da busca), na foto da rede ou na vigília. Trocar o host da câmera à mão apaga o MAC aprendido.
- **Fluxo** (`crates/camfacil/src/autoip.rs`): a vigília de 5 min (e o diagnóstico) acha o MAC noutro IP → (a) o IP novo responde na porta RTSP salva e a ARP mostra o mesmo MAC nele → (b) troca só o host (usuário, senha, porta e caminho iguais) → (c) reinicia o vídeo e espera a imagem por até 15 s → (d) evento `rede` "IP corrigido sozinho: A → B", aviso informativo no Início e no Ao vivo e mensagem curta pelos canais ligados (Telegram, WhatsApp, webhook) → (e) se a imagem não volta, desfaz, grava o motivo e cai no aviso com o botão "Corrigir para…" (com confirmação).
- **Proteções**: IP novo em conflito (fotos da rede) ou já usado por outra câmera cadastrada → não corrige; MAC desconhecido → não corrige; antirrepique de 1 correção por câmera a cada 10 min — se a câmera mudar de novo nesse prazo, a autocorreção dela é desligada e o técnico é avisado. Câmera desativada não é corrigida sozinha.
- **Registro**: últimas 50 tentativas em `settings["rede.autoip_log"]` (desta máquina, fora do backup), visíveis na tela Rede (seção "Correções automáticas de IP") junto da coluna "Autocorreção" do diagnóstico e do indicador no inventário.
- **Dados**: colunas `auto_ip` e `mac` em `cameras`, criadas por migração ao abrir o banco; a chave antiga `rede.macs` é levada para a coluna e apagada (também ao importar backup antigo). O backup cifrado leva `auto_ip` e `mac`. API: `POST /api/conectar` e `POST /api/cameras` aceitam `auto_ip`; `PATCH /api/cameras/{id}` aceita `auto_ip`; `GET /api/cameras` devolve `auto_ip` e `mac_conhecido`; `GET /api/rede/resumo` devolve `auto_avisos`; `GET /api/rede/diagnostico` devolve `autocorrecao` e `auto_log`.
- **Gravador (DVR/NVR)**: os canais de um gravador têm o mesmo MAC e mudam juntos. O primeiro canal passa por todas as conferências; os outros (mesmo MAC e mesmo IP antigo) seguem ele (`autoip::aplicar_irmao`), e o campo `gravador` acompanha o IP novo. No fluxo com confirmação, "Corrigir para…" também corrige todos os canais do mesmo aparelho.
- **Fora do escopo**: mudar o IP no próprio aparelho via ONVIF.

## Gravador (DVR/NVR): escolher os canais (status: implementado · fase 6)
Ao ligar um aparelho, depois que a senha confere no canal 1, `cf_core::gravador::detectar` decide se é gravador, nesta ordem: (1) ONVIF com login na porta 80 (`GetCapabilities` → `GetVideoSources`/`GetProfiles`, WS-Security UsernameToken): mais de uma fonte de vídeo é gravador e os perfis dão o nome dos canais; (2) modelo visto na busca com cara de gravador (NVR, DVR, XVR, MHDX, NVD, DS-76…; sonda até parar de responder, no mínimo 4 canais); (3) `DESCRIBE` no canal 2 e seguintes com o mesmo login, 4 por vez, 2,5 s por canal, parando no primeiro lote mudo (teto 64). Só vale para os caminhos com canal do gerador de RTSP (Intelbras/Dahua `cam/realmonitor?channel=N&subtype=S`, Hikvision `Streaming/Channels/N01|N02`); caminho genérico é sempre câmera.
- **API**: `POST /api/conectar` devolve `tipo: "camera"` (câmera criada, como antes) ou `tipo: "gravador"` com `token`, `nome` sugerido, `origem` e `canais` (`canal`, `nome` do ONVIF, `responde`), sem criar nada. `POST /api/gravador/adicionar` recebe `token`, `nome`, `canais` (um ou vários), `stream` (`secundario` por padrão, ou `principal`) e `auto_ip`; cada canal vira uma câmera "<nome> · Canal N" com `gravador` (host) e `canal` gravados. O login fica só na memória, por 15 min, preso ao token. `PATCH /api/cameras/{id}` aceita `canal` (só para câmera que veio de gravador; mantém o stream). `GET /api/cameras` devolve `gravador` e `canal`. O backup leva os dois campos.
- **Tela**: no Ligar câmera aparece "Isto é um gravador (DVR/NVR) com N canais. Qual canal mostrar?", com nome do gravador, qualidade (secundário por padrão), Todos/Nenhum e a grade de canais (número, nome do ONVIF, ponto verde de quem respondeu), paginada pelo `tela.js` para caber sem rolagem. Começa só com o canal 1 marcado (cada canal é um vídeo a mais no PC). No Editar câmera, campo "Canal do gravador".
- **Testes**: montador de URL por canal por fabricante, detecção (ONVIF, modelo, RTSP com sonda simulada e com gravador falso por TCP) e `scripts/telas-gravador.py` (Playwright, respostas simuladas, 5 tamanhos).

## Plano de IPs e diagrama unifilar (status: diagrama implementado · fase 4; plano de IPs pendente)
- **Pendente:** sugerir faixas (ex.: câmeras .100–.150) e exportar documentação.
- Diagrama unifilar em SVG gerado no servidor: roteador → switch → máquina/gravador → câmeras, com IP e MAC.
  Implementação: `cf-core/src/unifilar.rs`, `GET /api/rede/diagrama.svg?foto=ID&baixar=true`. Na tela usa os tokens; baixado, cores claras de papel. Ligações abaixo do roteador são presumidas (a varredura não vê cabos) e o carimbo avisa.

## Monitor de hardware (status: implementado · fase 5; sem processos)
- `crates/camfacil/src/hardware.rs`: amostra a cada **60 s** (sysinfo): CPU, RAM (% e MB), temperatura, disco livre da pasta de dados, uptime e quadros/s por câmera.
- Log em banco próprio (`data_dir/hardware.db`, tabela `hardware_log`), retenção de **30 dias**, agregando **por hora** depois de 48 h.
- Alertas: CPU > 85% por 10 min, RAM > 90%, disco < 1 GB, temperatura > 80 °C → evento `hardware` e mensagem no Telegram, com antirrepique de 1 h. O texto sugere reduzir câmeras/módulos ou trocar por câmeras com IA de borda.
- API: `GET /api/hardware` (agora) e `GET /api/hardware/log`. O resumo de 7 dias entra no relatório de validação.
- **Pendente:** top de processos e o relatório "por que travou" apontando o processo responsável.

## Backup, atualização, offline
- **Backup (implementado · fase 5):** `crates/camfacil/src/backup.rs`. Arquivo único `.cfbkp` (cabeçalho `CFIABKP1`, PBKDF2-HMAC-SHA256 com sal e iterações gravados, ChaCha20-Poly1305 do cofre), com a senha escolhida na hora. Leva câmeras (inclusive `auto_ip`, `mac`, `gravador` e `canal`), regras (com os ajustes), módulos, modo TV (com a logo), contatos do Telegram e do WhatsApp, integrações e selos; **não leva** eventos, identidade da máquina, porta, aparelhos pareados nem chaves `mcp_*`. Não há `camfacil.toml`: o estado é o SQLite. API: `POST /api/backup`, `POST /api/backup/importar`, `POST /api/backup/telegram`; o Telegram também aceita `/backup <senha>` (apaga a mensagem com a senha, se o Telegram deixar). Agendamento do backup **pendente**.
- **Relatório de validação / proposta em PDF (implementado · fase 5):** `crates/camfacil/src/relatorio.rs`, gerador de PDF próprio (sem crate), A4, Helvetica, uma foto por câmera. Seções: capa com a marca do técnico, resumo das câmeras, módulos de IA ligados, uma página por câmera, alertas dos últimos 7 dias, saúde das câmeras e da máquina, próximo passo e proposta. Fica em `data_dir/relatorios/`; API `GET/POST /api/relatorio`, `/api/relatorio/{arquivo}` (baixar, apagar) e `/telegram`.
- **Atualização com rollback: não implementada.** Hoje atualiza-se rodando o instalador de novo (o `install.sh`/`install.ps1` trocam o binário) ou baixando a release; o `SHA256SUMS.txt` da release permite conferir o arquivo. Assinatura e rollback com um clique seguem como proposta.
- **Pacote offline: não implementado** como zip único. O modelo de IA pode ser copiado por pendrive para `data_dir/modelos/` (SDD-02) e o runtime ONNX aceita `CAMFACIL_ORT_LIB`.

## Acesso de fora pelo Tailscale (status: implementado · fase 3)
Primeira opção da ADR-0005: usar o cliente Tailscale já instalado na máquina (sem nó embutido, sem porta no roteador).

- `crates/camfacil/src/tailscale.rs`: acha o binário (PATH; Windows `C:\Program Files\Tailscale\tailscale.exe`; macOS `/Applications/Tailscale.app/...`), roda `tailscale status --json` (fallback `tailscale ip -4`) com timeout de 2,5 s, fora da thread da UI.
- **Escuta**: por padrão o servidor escuta só em `127.0.0.1`. Uma vigília leve (a cada 60 s; sem Tailscale instalado não roda processo nenhum) descobre o IP Tailscale e abre um segundo listener nesse IP, na mesma porta, **sem reiniciar** e sem expor a LAN. Com `--lan` o servidor já escuta em `0.0.0.0` e a vigília não faz nada. Se o IP mudar, o próximo início faz o bind no novo IP.
- **API**: `GET /api/tailscale` → `{instalado, logado, ip, nome, url, url_nome, escutando, aguardando, lan, erro, comando_instalar, sistema}`; `GET /api/tailscale/qr.svg` (reaproveita o gerador de QR do elo).
- **UI** (Integrações → "Acessar de fora (Tailscale)"): situação, endereço `http://<ip>:<porta>` com copiar, nome MagicDNS, QR, instruções por sistema (Windows `winget install tailscale.tailscale`; Linux/Raspberry `curl -fsSL https://tailscale.com/install.sh | sh` + `sudo tailscale up`; macOS `brew install --cask tailscale`; celular: app Tailscale com a mesma conta). Explica que o vídeo não passa pela nuvem e que o celular precisa estar na mesma rede Tailscale.
- Falta validar numa máquina real com Tailscale (aqui foi testado com um binário simulado respondendo `status --json` e `ip -4`, e o bind em um segundo IP).

## Acesso de fora pelo ngrok — temporário, só para testes (status: implementado · ADR 0015)
Fica na mesma tela (Integrações → Acesso de fora), abaixo do Tailscale, como seção que abre.

- **Como funciona:** o técnico cola o authtoken do ngrok (cofre, cifrado). "Ligar o ngrok" usa o agente oficial do PATH ou baixa de `bin.equinox.io` para `camfacil-dados/bin` (HTTPS, redirecionamento só para `*.equinox.io`/`*.ngrok.com`, teto de tamanho, `ngrok version 3…` conferido, SHA-256 no evento) e roda `ngrok http 127.0.0.1:<porta> --log=stdout --log-format=json` com `NGROK_AUTHTOKEN` no ambiente. A URL vem do log ou de `http://127.0.0.1:4040/api/tunnels`.
- **Alvos:** "Painel (com senha)" → `basic-auth` por Traffic Policy (arquivo 0600 em `camfacil-dados`, apagado ao desligar), usuário `camfacil` e senha aleatória mostrados na tela; "Só o MCP" → servidor à parte em `127.0.0.1:<porta livre>` só com `/mcp`, protegido pelo token do MCP.
- **Avisos:** pop-up obrigatório antes de ligar (diálogo da página: "O ngrok é temporário, só para testes… Para usar por mais de um dia, use o Tailscale", botões "Entendi, ligar mesmo assim" / "Usar o Tailscale"); aviso amarelo fixo com há quanto tempo está ligado e, depois de 24 h, sugestão de desligar; chave "Desligar sozinho depois de 24 h" (`ngrok_auto24`).
- **Log:** evento `acesso` com quem ligou (este computador ou o IP do aparelho), quando e a URL; queda e desligamento também. Fechar o programa derruba o túnel. Desligar o MCP derruba o túnel "Só o MCP".
- **Android:** indisponível (mensagem clara; usar o app Tailscale).
- **API:** `GET /api/ngrok` → `{disponivel, instalado, baixavel, tem_authtoken, auto24, ligado, alvo, url, desde, ha_s, usuario, senha, erro, link_authtoken}`; `POST /api/ngrok` `{authtoken?, auto24?, acao?: ligar|desligar, alvo?: painel|mcp, entendi}` (ligar exige `entendi` e o termo do técnico).
- **Custo:** nada no binário (sem SDK, sem crate nova); o agente só existe enquanto ligado.
- Testado com o agente real baixado (download, extração e conferência; authtoken falso → mensagem "o ngrok recusou o authtoken") e com um agente simulado (túnel, senha, só MCP, queda ao fechar). Falta validar numa conta real do ngrok.
