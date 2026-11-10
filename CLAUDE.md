# CLAUDE.md — CamFácil IA

Ferramenta open source (AGPL-3.0) em Rust para instaladores de CFTV aprenderem e validarem IA de vídeo num PC velho, com um comando. Ambiente de **aprendizagem e validação**, não produto final.

Versão atual: ver `version` no `Cargo.toml` da raiz (a release sai sozinha quando ela muda; ver `.github/workflows/rust.yml`).

## Leia antes de codar (só o necessário para a tarefa)
- Produto: `docs/PRD.md`
- Núcleo: `docs/sdd/SDD-01-nucleo.md` · Módulos: `SDD-02` · Educação: `SDD-03` · Integrações: `SDD-04` · Rede: `SDD-05` · App/TV/Web: `SDD-06` · Modo estúdio: `SDD-07` · Contas e isolamento: `SDD-08` · Inteligências por regra: `SDD-09` · IA avançada (proposta): `SDD-10` · LGPD: `docs/legal/`
- Web app e API: **https://cfia.ysafe.com.br** (Dokploy, ver `docs/DEPLOY.md`)
- Escolinha (instrução) fica no site em `crates/cfia-api/web/escola.*`; programa local é só operação (ADR 0008)
- Decisões: `docs/adr/` (0001 a 0017) · Visual: `DESIGN.md` e `web/tokens.css` (redesign com a skill `impeccable`)
- Relatórios de fase (histórico): `docs/RELATORIO-FASE3.md`, `docs/RELATORIO-FASE6-ANDROID.md`

## Mapa do código
| Pasta | O que é |
|---|---|
| `crates/camfacil` | Binário do programa local: servidor axum, API (`api.rs`), vídeo via ffmpeg (`video.rs`, `visor.rs`), IA ONNX (`ia/`), rede, hardware, backup, TV, botões de portão/relé, MCP, Tailscale, ngrok, elo (Agente-Cam-Elo-IA), modo estúdio |
| `crates/cf-core` | Núcleo compartilhado (programa local e Android): SQLite (`store.rs`), RTSP/ONVIF, visão leve e cerca virtual (`visao.rs`), detecção e rastreio (`deteccao.rs`), catálogo de inteligências (`inteligencias.rs`), sugestão e avaliação de regras, saúde da câmera, diagnóstico de rede e diagrama unifilar, gravador DVR/NVR, receita e notebook do estúdio |
| `crates/cf-notify` | Telegram, WhatsApp (Evolution API), webhook, cofre cifrado (ChaCha20-Poly1305) |
| `crates/cfia-api` | API e site da nuvem (login Google, Postgres com RLS, pareamento, sinalização, Formação, selo, receitas do estúdio) |
| `web/` | Dashboard do programa local: HTML + CSS + JS puro, textos em `web/i18n/pt-BR.json`, tokens em `web/tokens.css` |
| `android/` | App Kotlin (sem AndroidX) que abre o painel num WebView e, opcionalmente, roda o núcleo Rust no aparelho (ADRs 0009, 0011, 0014) |
| `site-docs/` + `mkdocs.yml` | Site de documentação (MkDocs Material) em pt, en e es |
| `scripts/` | Instaladores, `budget.sh`, deploy, build Android, telas de teste (Playwright) |
| `deploy/` | Dockerfile e composes da API (Dokploy, VPS, servidor) |

Não existem `cf-modules`, `modules/<id>/module.toml` nem `guide/`: o contrato de módulo por pasta (SDD-02) ficou como proposta para a comunidade. Hoje os módulos são os de `crates/camfacil/src/video.rs` (`MODULOS`) e as inteligências por regra de `crates/cf-core/src/inteligencias.rs`.

## Regras inegociáveis
1. **Leveza primeiro.** Núcleo ocioso < 50 MB RAM, < 2% CPU, binário < 30 MB. Toda dependência nova: justificar no PR (tamanho e custo). Prefira `default-features = false`.
2. **Sem Node/Python/Docker no caminho de execução do programa.** Front = HTML + CSS + JS puro embutido no binário. (Docker só no servidor do painel e na Evolution API opcional do WhatsApp.)
3. **Offline-first.** Nada essencial depende de nuvem.
4. **Visual limpo.** Nunca desenhar caixas/máscaras do YOLO para o cliente; só o balão com o nome da regra. A ferramenta de rotular do estúdio (do técnico) é a exceção.
5. **CSS só com tokens** de `web/tokens.css`. Base preta, verde só em detalhes (ver `DESIGN.md`).
6. **Escopo**: não implementar painel multicliente, busca em gravações, ronda virtual nem gravação contínua (PRD §6).
7. **Segurança**: segredos cifrados no cofre; MCP desligado por padrão, com limite por minuto e resultado marcado como dado; ações físicas (portão/relé) exigem PIN, confirmação e log; o MCP nunca aciona portão. Fora da própria máquina, só aparelho autorizado (PIN ou confirmação na máquina; ADR 0017); rota nova que não deve exigir isso entra em `acesso::livre` com justificativa. Ver `docs/seguranca/SEGURANCA.md`.
8. Textos de interface em **pt-BR**, em arquivos de tradução (`web/i18n/pt-BR.json`; no site, `crates/cfia-api/web`).
9. **Zero rolagem.** Nenhuma tela (programa local, app Android, site, curso) rola a página. Tudo cabe numa tela só, do celular ao monitor (layout em `100dvh`, adaptado ao tamanho da tela). Informação a mais vira seção que abre (acordeão), aba, menu ou submenu, nunca página comprida. "Rolou a página, perdeu o cliente." Listas longas paginam ou filtram dentro do espaço fixo. Ver `DESIGN.md`.
10. **Nada vem ligado.** Todo módulo de IA começa desligado; o técnico escolhe. Regra criada com o módulo desligado fica salva desligada e liga junto com o módulo.

## Convenções
- Workspace Cargo em `crates/`: `camfacil`, `cf-core`, `cf-notify`, `cfia-api`.
- O CI (`.github/workflows/rust.yml`) roda `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (com Postgres para a `cfia-api`), `scripts/budget.sh`, build Windows/Linux x86_64/Linux ARM64, APK Android e a publicação da release.
- Rode `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace` antes de cada commit.
- Commits: Conventional Commits em português (`feat(rtsp): gerador para Hikvision`).
- Um PR por item do roadmap; descreva impacto em RAM/CPU.
- Mudou uma decisão de arquitetura → nova ADR em `docs/adr/` (a próxima é a 0018).
- Mudou comportamento visível → atualize o SDD da área, o `README.md` (Novidades) e, se for o caso, `site-docs/pt|en|es`.
- **A cada 5 commits** (no máximo), revise e atualize toda a documentação: SDDs das áreas tocadas, `docs/NOVIDADES.md`, `README.md`, `site-docs/pt|en|es` e o mapa do código. Mudança de comportamento já entra com a doc no mesmo PR; esta revisão pega o que escapou.
- Subir a versão no `Cargo.toml` da raiz publica uma release nova (inclui o APK, que usa a mesma versão).

## Economia de tokens
- Não leia arquivos inteiros sem necessidade; use busca.
- Não reescreva docs que não fazem parte da tarefa.
- Prefira scripts em `scripts/` para tarefas repetitivas (build, medir orçamento, gerar release).
- Use os agentes em `.claude/agents/` (`guardiao-da-leveza`, `revisor-de-escopo`, `autor-de-aulas`) e as skills em `.claude/skills/` (`abrir-pr`, `novo-modulo-ia`, `novo-fabricante-rtsp`, `impeccable`) quando a tarefa casar com eles.

## Comandos
```sh
cargo run -p camfacil            # sobe a dashboard na primeira porta livre a partir de 7070
cargo run -p camfacil -- --lan   # escuta na rede local (celular na mesma rede)
cargo test --workspace
scripts/budget.sh                # mede RAM/CPU/tamanho do binário (Linux); roda no CI
scripts/gerar-video-demo.py      # refaz a câmera de demonstração (loja da Formação)
```
Outras opções do binário: `--no-browser`, `--mcp` (ponte stdio do MCP), `--autoiniciar`, `--version`.
