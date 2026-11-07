---
name: abrir-pr
description: Prepara e abre um pull request do CamFácil IA no padrão do projeto (checagens, descrição, impacto em recursos). Use ao terminar uma tarefa.
---

# Abrir PR

1. Rode `cargo fmt --all`, `cargo clippy --workspace -- -D warnings` e `cargo test --workspace`. Corrija antes de seguir.
2. Acione `guardiao-da-leveza` se mudou Cargo.toml, ingestão de vídeo ou inferência.
3. Commit em Conventional Commits (pt-BR).
4. Abra o PR com `gh pr create` usando o template `.github/pull_request_template.md`, preenchendo: o que muda, item do roadmap, impacto em RAM/CPU/tamanho, como testar.
5. Se a mudança alterar arquitetura, inclua ou atualize uma ADR.
