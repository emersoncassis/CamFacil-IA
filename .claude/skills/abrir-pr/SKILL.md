---
name: abrir-pr
description: Prepara e abre um pull request do CamFácil IA no padrão do projeto (checagens, descrição, impacto em recursos, docs e versão). Use ao terminar uma tarefa.
---

# Abrir PR

1. Rode `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` e `cargo test --workspace`. Corrija antes de seguir. (Os testes da `cfia-api` que usam banco leem `TEST_DATABASE_URL`; o CI sobe um Postgres 16.)
2. Acione `guardiao-da-leveza` se mudou `Cargo.toml`, ingestão de vídeo ou inferência; ele roda `scripts/budget.sh`.
3. Se mexeu em tela, confira em 360×640, 390×844, 768×1024, 1366×768 e 1920×1080 sem rolar a página (regra 9). Há roteiros Playwright em `scripts/telas-*.py`.
4. Atualize a documentação da mudança: o SDD da área (`docs/sdd/`), a seção **Novidades** do `README.md` e, se aparecer para o técnico, `site-docs/pt`, `en` e `es`. Textos de tela vão em `web/i18n/pt-BR.json`.
5. Se a mudança alterar arquitetura, crie ou atualize uma ADR em `docs/adr/` (próxima: 0017).
6. Versão: subir `version` no `Cargo.toml` da raiz faz o CI publicar uma release nova (binários, APK e `SHA256SUMS.txt`). Só suba quando a mudança deve virar release.
7. Commit em Conventional Commits (pt-BR).
8. Abra o PR com `gh pr create` usando o template `.github/pull_request_template.md`, preenchendo: o que muda, item do roadmap, impacto em RAM/CPU/tamanho, como testar.
