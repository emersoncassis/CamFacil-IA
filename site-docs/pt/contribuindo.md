# Contribuir

Você não precisa ser programador para ajudar: testar em campo, relatar um problema e compartilhar com outro técnico já conta.

## Apoie o projeto

O CamFácil IA é livre e gratuito, e vai continuar assim. Por trás dele tem tempo, estudo, anos de sala de aula e dinheiro do próprio bolso: só para começar, foram mais de R$ 10 mil.

Se ele te ajudou a aprender, a vender ou a ganhar tempo, pague um café. Cada apoio mantém o projeto vivo e aberto para o próximo técnico.

[:fontawesome-solid-mug-hot: Apoiar com Pix ou cartão](apoiadores.md){ .md-button .md-button--primary }

## Para quem programa

1. Leia o [PRD](https://github.com/emersoncassis/CamFacil-IA/blob/main/docs/PRD.md), principalmente o que fica **fora do escopo**.
2. Leia o SDD da área que você vai mexer e abra uma issue antes de mudanças grandes.
3. Antes de cada commit:

    ```sh
    cargo fmt --all
    cargo clippy --workspace -- -D warnings
    cargo test --workspace
    ```

4. Commits em [Conventional Commits](https://www.conventionalcommits.org/pt-br/) em português e, no PR, o impacto em RAM, CPU e tamanho do binário.

## Regras da casa

- **Leveza é requisito.**
- **Zero rolagem:** nenhuma tela do programa, do app ou do curso rola a página.
- Visual com os tokens de `web/tokens.css`: base preta, verde só em detalhes.
- Mudança de arquitetura vira uma nova ADR.

## Roteiro

Versão atual: **0.6.0** ([releases](https://github.com/emersoncassis/CamFacil-IA/releases)).

- [x] Núcleo, cerca virtual, formação com selo
- [x] Telegram, WhatsApp, rede e diagrama, modo TV, relatório e backup
- [x] App Android, painel web, Agente-Cam-Elo-IA e MCP
- [ ] Modo estúdio completo (conversa com a IA montando a receita, pré-marcação das fotos)

Ao contribuir, você licencia o seu código sob a [AGPL-3.0](https://github.com/emersoncassis/CamFacil-IA/blob/main/LICENSE).
