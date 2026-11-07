# Prompt de arranque — Fase 1 (Núcleo)

Você está no repositório do CamFácil IA. Leia `CLAUDE.md` e `docs/sdd/SDD-01-nucleo.md`. Não leia os outros SDDs agora.

Objetivo desta sessão: entregar o núcleo mínimo funcionando, em passos pequenos, com um commit por passo.

1. Crie o workspace Cargo (`Cargo.toml` na raiz, `crates/camfacil` binário, `crates/cf-core` biblioteca), edição 2021, `[profile.release]` com `lto = true`, `codegen-units = 1`, `strip = true`, `opt-level = "z"` (medir depois se `3` compensa).
2. Servidor axum em `localhost:7070` servindo a dashboard estática embutida (`web/`, usando `web/tokens.css`), com quatro abas vazias: Início, Câmeras, Módulos de IA, Integrações.
3. `GET /api/system`: CPU, núcleos, RAM, SO, versão (crate `sysinfo` com features mínimas).
4. Benchmark simples (`POST /api/bench`): mede desempenho de CPU por alguns segundos e grava o nível (leve/médio/pesado) em SQLite (`rusqlite` com `bundled`).
5. Gerador de RTSP (`POST /api/rtsp/build`) para Intelbras/Dahua e Hikvision, stream principal/secundário/extra e canal, com testes unitários.
6. Busca ONVIF WS-Discovery (`GET /api/discover`): IP, fabricante e modelo quando disponíveis; tempo limite de 3 s.
7. `scripts/budget.sh`: compila em release, mostra o tamanho do binário e mede a RAM ociosa por 10 s.
8. CI no GitHub Actions: fmt, clippy, test e build para Windows x86_64, Linux x86_64 e Linux ARM64.

Regras: siga as regras inegociáveis do `CLAUDE.md`. Antes de cada commit rode fmt, clippy e test. No fim, rode `scripts/budget.sh`, acione o agente `guardiao-da-leveza` e resuma o que foi entregue e o que ficou pendente.
