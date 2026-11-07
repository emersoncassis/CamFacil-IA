# Relatório — Fase 3 (operação local): saúde, Tailscale, Telegram e IA média

Branch `feat/operacao-fase3` · 2026-11-07 · quatro commits, um por item. Só o programa local (`crates/camfacil`, `crates/cf-core`, nova `crates/cf-notify`, `web/`, `modelos/`) e docs; `crates/cfia-api` intocada.

## O que foi feito

| # | Item | Onde | Docs |
|---|---|---|---|
| 1 | **Monitor de saúde da câmera** visível: tag em "Suas câmeras" e no ao vivo, balão que pisca (escura/coberta/desfocada), bloco "Saúde das câmeras" no Início, evento `saude` só na mudança confirmada (histerese 4 s, com testes), estado "sem sinal", som opcional `saude:<id>` | `cf-core/src/saude.rs`, `video.rs`, `/api/saude`, `web/` | SDD-02 |
| 2 | **Acesso de fora pelo Tailscale** (ADR-0005, opção 1): detecta cliente instalado/logado com timeout, mostra `http://<ip>:<porta>`, MagicDNS, copiar, QR; vigília abre um segundo listener no IP Tailscale sem reiniciar e sem expor a LAN (`--lan` continua opcional); instruções por sistema | `camfacil/src/tailscale.rs`, `/api/tailscale`, `web/` | SDD-05, ADR-0005 |
| 3 | **Telegram** (SDD-04): crate `cf-notify` (cliente mínimo da Bot API, long polling 30 s só com token, multipart à mão), token cifrado ChaCha20-Poly1305 com chave 0600 em `data_dir/segredo.key`, contatos aprovados na dashboard (quem manda `/start` aparece), alertas de linha/zona/saúde com foto, antirrepique 30 s por regra, fila offline de 40, comandos `/status /foto /resumo /liga /desliga /ajuda` | `crates/cf-notify/`, `camfacil/src/notificacoes.rs`, `/api/telegram*`, `web/` | SDD-04 |
| 4 | **Módulos médios por ONNX**: `ort` com `load-dynamic`; ONNX Runtime 1.20.1 oficial e YOLO11n ONNX baixados sob demanda com SHA-256, progresso e cancelar; Pessoas e veículos, Contagem, Permanência (N s), Mapa de calor (camada do técnico); ≤ 3 inferências/s no total; destrava só com teste da máquina ≥ médio; pesados seguem travados com o texto do PRD §6 | `camfacil/src/ia/`, `cf-core/src/deteccao.rs`, `modelos/`, `/api/ia*`, `/api/cameras/{id}/calor`, `web/` | SDD-02, ADR-0002, `modelos/README.md` |

Regras mantidas: sem caixas do YOLO (só balões e tags), CSS só com tokens, textos em `web/i18n/pt-BR.json`, offline-first (modelo copiável por pendrive; Telegram e download são os únicos pontos que usam internet, e são opcionais), segredos cifrados, versão do Cargo.toml inalterada.

## Impacto em RAM, CPU e binário (release, Linux x64, Xeon 2,1 GHz · 2 núcleos)

| Situação | Binário | RAM | CPU |
|---|---|---|---|
| Antes da fase 3 (ocioso) | 3,8 MB | 5,5 MB | 0 % |
| Depois do item 1 (ocioso) | 3,8 MB | 5,5 MB | 0 % |
| Depois do item 3 (ocioso) | 4,4 MB | 6,0 MB | 0 % |
| **Depois do item 4 (ocioso)** | **4,6 MB** | **6,4 MB** | **0 %** |
| Só nível leve, 2 câmeras ao vivo | — | 8,6 MB | 6 % |
| Detector ligado, 2 câmeras | — | 134 MB | ~55 % de um núcleo · 85 ms/inferência |

Orçamento do ocioso (< 30 MB · < 50 MB · < 2 %) continua folgado. A IA média custa RAM e CPU só enquanto um módulo médio está ligado; descarrega o modelo após 1 min ocioso.

Dependências novas e por quê: `chacha20poly1305` (puro Rust, ~60 KB; token cifrado), `reqwest` já estava no workspace (aqui com `rustls-no-provider` para reaproveitar o `ring` do binário e não puxar `aws-lc`), `ort`+`ort-sys`+`libloading` (sem `ndarray`, sem download no build), `miniz_oxide` (inflate do tgz/zip), `sha2` (já em `cf-core`).

## O que falta validar numa máquina real

1. **Tailscale**: aqui foi testado com um binário simulado (`status --json`, `ip -4`) e bind em um segundo IP de loopback. Falta rodar com o cliente real no Windows (`C:\Program Files\Tailscale\tailscale.exe`) e no Linux, e abrir pelo celular na mesma rede Tailscale.
2. **Telegram**: testado contra um servidor Bot API simulado (`CAMFACIL_TELEGRAM_API`): token, contatos, aprovação, alerta com foto, todos os comandos. Falta um bot real do @BotFather numa máquina com internet (TLS via `rustls-platform-verifier`: no Linux precisa de `ca-certificates`).
3. **Modelo**: o programa procura primeiro `https://github.com/emersoncassis/CamFacil-IA/releases/download/modelos-v1/yolo11n.onnx` — **a release `modelos-v1` precisa ser publicada** com o arquivo de `modelos/yolo11n.onnx` (SHA em `modelos/yolo11n.onnx.sha256`). Fallback: o arquivo no branch `main`. O download do ONNX Runtime pelas releases oficiais foi validado de verdade aqui (Linux x64); Windows/macOS/aarch64 têm URL e SHA conferidos, mas não foram executados.
4. **Detecção** validada com `bus.jpg` (4 pessoas e 1 ônibus) num vídeo em loop; falta câmera real, máquina fraca (Raspberry/TV box) e ajuste dos limiares de permanência e casamento de trilhas em cena de loja.
5. Nome MagicDNS, hora local do bot (vem do fuso do navegador do técnico via `POST /api/fuso`) e textos finais merecem uma passada de UX com um instalador.
