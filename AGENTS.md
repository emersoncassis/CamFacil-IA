# AGENTS.md — CamFácil IA

Para assistentes de código que leem este arquivo (Codex, Opencode, Antigravity e outros). No Claude Code, o equivalente é o [`CLAUDE.md`](CLAUDE.md).

1. **Leia o [`CLAUDE.md`](CLAUDE.md) inteiro.** Ele traz o mapa do código, as regras inegociáveis, as convenções e os comandos. Vale para qualquer assistente.
2. **A pessoa quer mudar, criar ou montar a própria versão?** Siga o **CamFácil IA Builder**: [`.claude/skills/camfacil-ia-builder/SKILL.md`](.claude/skills/camfacil-ia-builder/SKILL.md). Ele é o ponto de partida de todo desenvolvimento neste repositório, fala com quem não programa e usa o [Reversa](https://github.com/sandeco/reversa) como método.
3. **Antes de escrever código**, confira o pedido contra [`.claude/skills/camfacil-ia-builder/referencias/limites.md`](.claude/skills/camfacil-ia-builder/referencias/limites.md): lei (LGPD), licença (AGPL-3.0), segurança, regras da casa e escopo. Pedido que fere as três primeiras é recusado com o motivo e uma alternativa.
4. As outras skills ficam em `.claude/skills/` (`novo-modulo-ia`, `novo-fabricante-rtsp`, `abrir-pr`, `impeccable`) e os agentes em `.claude/agents/`. Se o seu assistente não carrega skills sozinho, abra o `SKILL.md` e siga os passos.
