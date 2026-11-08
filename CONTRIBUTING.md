# Como contribuir

Obrigado por querer ajudar técnicos de todo o Brasil a aprender IA de vídeo.

Não programa? Testar em campo, relatar problemas e compartilhar com outro técnico já ajuda muito. E, se quiser apoiar com dinheiro, [apoie pelo Pix ou cartão](https://emersoncassis.github.io/CamFacil-IA/apoiadores/).

## Antes de começar
1. Leia o [PRD](docs/PRD.md), principalmente a seção **6 — Fora do escopo**. O CamFácil IA ensina e valida; não concorre com câmeras de borda nem com plataformas de IA em nuvem.
2. Leia o SDD da área que você vai mexer em [`docs/sdd/`](docs/sdd).
3. Abra uma issue antes de mudanças grandes.

## Regras
- **Leveza é requisito.** Diga no PR o impacto em RAM, CPU e tamanho do binário.
- Sem Node, Python ou Docker no caminho de execução.
- CSS só com os tokens de [`web/tokens.css`](web/tokens.css).
- Textos de interface em pt-BR.
- Mudança de arquitetura = nova ADR em [`docs/adr/`](docs/adr).

## Fluxo
```sh
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
```
Commits em [Conventional Commits](https://www.conventionalcommits.org/pt-br/) em português: `feat(cerca): estados editáveis`.

## Criando um módulo de IA
Siga o contrato em [SDD-02](docs/sdd/SDD-02-modulos-ia.md). Todo módulo traz uma aula (SDD-03). Se você usa Claude Code, a skill `novo-modulo-ia` faz o esqueleto.

## Licença
Ao contribuir, você concorda em licenciar sua contribuição sob a [AGPL-3.0](LICENSE).
