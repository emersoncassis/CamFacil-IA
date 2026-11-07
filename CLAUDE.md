# CLAUDE.md — CamFácil IA

Ferramenta open source (AGPL-3.0) em Rust para instaladores de CFTV aprenderem e validarem IA de vídeo num PC velho, com um comando. Ambiente de **aprendizagem e validação**, não produto final.

## Leia antes de codar (só o necessário para a tarefa)
- Produto: `docs/PRD.md`
- Núcleo: `docs/sdd/SDD-01-nucleo.md` · Módulos: `SDD-02` · Educação: `SDD-03` · Integrações: `SDD-04` · Rede: `SDD-05` · App/TV/Web: `SDD-06` · Modo estúdio: `SDD-07` · LGPD: `docs/legal/`
- Decisões: `docs/adr/` · Visual: `DESIGN.md` e `web/tokens.css` (redesign com a skill `impeccable`)

## Regras inegociáveis
1. **Leveza primeiro.** Núcleo ocioso < 50 MB RAM, < 2% CPU, binário < 30 MB. Toda dependência nova: justificar no PR (tamanho e custo). Prefira `default-features = false`.
2. **Sem Node/Python/Docker no caminho de execução.** Front = HTML + CSS + JS puro embutido no binário.
3. **Offline-first.** Nada essencial depende de nuvem.
4. **Visual limpo.** Nunca desenhar caixas/máscaras do YOLO para o cliente; só o balão com o nome da regra.
5. **CSS só com tokens** de `web/tokens.css`. Base preta, verde só em detalhes (ver `DESIGN.md`).
6. **Escopo**: não implementar painel multicliente, busca em gravações, ronda virtual nem gravação contínua (PRD §6).
7. **Segurança**: segredos cifrados; MCP desligado por padrão; ações físicas (portão/relé) exigem confirmação e log.
8. Textos de interface em **pt-BR**, em arquivos de tradução.

## Convenções
- Workspace Cargo em `crates/` (`camfacil` binário, `cf-core`, `cf-modules`, `cf-notify`…), criado conforme as fases.
- `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test` antes de cada commit.
- Commits: Conventional Commits em português (`feat(rtsp): gerador para Hikvision`).
- Um PR por item do roadmap; descreva impacto em RAM/CPU.
- Mudou uma decisão de arquitetura → nova ADR em `docs/adr/`.

## Economia de tokens
- Não leia arquivos inteiros sem necessidade; use busca.
- Não reescreva docs que não fazem parte da tarefa.
- Prefira scripts em `scripts/` para tarefas repetitivas (build, medir orçamento, gerar release).
- Use os agentes em `.claude/agents/` e as skills em `.claude/skills/` quando a tarefa casar com eles.

## Comandos
```sh
cargo run -p camfacil            # sobe a dashboard em http://localhost:7070
cargo test --workspace
scripts/budget.sh                # mede RAM/CPU/tamanho do binário (a criar na fase 1)
```
