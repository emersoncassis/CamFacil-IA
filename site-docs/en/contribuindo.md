# Contributing

You don't need to be a developer to help: field testing, reporting a problem and sharing with another installer all count.

## Support the project

CamFácil IA is free and open source, and it will stay that way. Behind it there is time, study, years of classroom teaching and money out of my own pocket: over R$ 10,000 just to get it started.

If it helped you learn, sell or save time, buy me a coffee. Every bit of support keeps the project alive and open for the next installer.

[:fontawesome-solid-mug-hot: Support with Pix or card](apoiadores.md){ .md-button .md-button--primary }

## For developers

1. Read the [PRD](https://github.com/emersoncassis/CamFacil-IA/blob/main/docs/PRD.md), especially what is **out of scope**.
2. Read the SDD for the area you'll touch and open an issue before big changes.
3. Before each commit:

    ```sh
    cargo fmt --all
    cargo clippy --workspace -- -D warnings
    cargo test --workspace
    ```

4. Commits follow [Conventional Commits](https://www.conventionalcommits.org/), written in Portuguese, and the PR states the impact on RAM, CPU and binary size.

## House rules

- **Lightweight is a requirement.**
- **Zero scrolling:** no screen in the program, the app or the course scrolls the page.
- Visuals use the tokens in `web/tokens.css`: black base, green only in details.
- An architecture change becomes a new ADR.

## Roadmap

Current version: **0.9.8** ([releases](https://github.com/emersoncassis/CamFacil-IA/releases)).

- [x] Core, virtual fence, course with badge
- [x] Telegram, WhatsApp, network and diagram, TV mode, report and backup
- [x] Android app, web dashboard, Agente-Cam-Elo-IA and MCP
- [ ] Full studio mode (AI chat building the recipe, photo pre-labeling)

By contributing, you license your code under the [AGPL-3.0](https://github.com/emersoncassis/CamFacil-IA/blob/main/LICENSE).
