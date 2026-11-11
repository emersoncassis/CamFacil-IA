# Como contribuir

Obrigado por querer ajudar técnicos de todo o Brasil a aprender IA de vídeo.

Não programa? Testar em campo, relatar problemas e compartilhar com outro técnico já ajuda muito. E, se quiser apoiar com dinheiro, [apoie pelo Pix ou cartão](https://emersoncassis.github.io/CamFacil-IA/apoiadores/).

## Não programa e quer mudar algo?
Use o **[CamFácil IA Builder](https://emersoncassis.github.io/CamFacil-IA/builder/)**: faça o fork, abra a pasta num assistente de código (Claude Code, Codex ou outro) e chame `/camfacil-ia-builder`. Ele guia do pedido ao código, com o método do [Reversa](https://github.com/sandeco/reversa), e avisa quando um pedido fere a LGPD, a licença ou a segurança.

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
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Commits em [Conventional Commits](https://www.conventionalcommits.org/pt-br/) em português: `feat(cerca): estados editáveis`.

## Criando uma inteligência ou um módulo de IA
O contrato de módulo por pasta (`module.toml`) do [SDD-02](docs/sdd/SDD-02-modulos-ia.md) é proposta e ainda não existe. Hoje há dois caminhos:
- **Dentro do programa:** nova inteligência por regra ([SDD-09](docs/sdd/SDD-09-inteligencias-por-regra.md), `crates/cf-core/src/inteligencias.rs` e `deteccao.rs`) ou novo módulo (lista `MODULOS` em `crates/camfacil/src/video.rs`). Se a capacidade merece lição, ela vai na Formação do site ([SDD-03](docs/sdd/SDD-03-educacao.md)). Se você usa Claude Code, a skill `novo-modulo-ia` guia os passos.
- **Sem compilar nada:** uma receita do modo estúdio ([SDD-07](docs/sdd/SDD-07-modo-estudio.md)), publicada na galeria do site.

Suporte a uma marca nova de câmera ou gravador: skill `novo-fabricante-rtsp`.

## Licença
Ao contribuir, você concorda em licenciar sua contribuição sob a [AGPL-3.0](LICENSE).
