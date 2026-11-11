# Reversa: a base do CamFácil IA Builder

O [Reversa](https://github.com/sandeco/reversa), criado pelo professor **Sandeco Macedo**, é o framework que o Builder usa para desenvolver. Ele transforma um sistema existente em especificações que um agente de código consegue seguir e, no **ciclo forward**, faz o sistema evoluir uma feature por vez, da especificação ao código. Licença MIT. Documentação: <https://sandeco.github.io/reversa/pt/>.

Por que o Builder depende dele: quem não programa não consegue revisar código, mas consegue revisar **o que foi combinado**. O Reversa obriga a escrever o combinado antes do código e a manter os dois iguais depois.

As informações abaixo são da versão 1.3.6. O Reversa muda rápido: em caso de diferença, vale a documentação dele.

## Instalar (uma vez por clone)

Precisa do Node.js 18 ou mais novo, só para o instalador. O programa do CamFácil IA continua sem Node.

```sh
npx reversa install
```

Rode na raiz do repositório. O instalador faz perguntas (motores de IA detectados, quais agentes, nome e idioma do projeto): aceite os padrões e escolha **português**. Se o agente não conseguir responder às perguntas, peça para a pessoa rodar o comando no terminal dela.

O que ele cria: as skills em `.agents/skills/` (e em `.claude/skills/` para o Claude Code) e a pasta `.reversa/` (estado, configuração e plano). O instalador **só cria arquivos novos**: como este repositório já tem `CLAUDE.md` e `AGENTS.md`, eles ficam como estão, e as skills do Reversa funcionam do mesmo jeito.

Outros comandos: `npx reversa status` (onde parou), `npx reversa update`, `npx reversa add-engine` (outro assistente de código), `npx reversa uninstall` (remove só o que o Reversa criou).

## Primeira vez: descobrir o sistema

Se ainda não existe `_reversa_sdd/`, rode a descoberta uma vez:

- `/reversa`: passo a passo (reconhecimento, escavação, interpretação, geração, revisão), parando entre os agentes até a pessoa digitar `CONTINUAR`.
- `/reversa-autonomous`: a mesma descoberta, de ponta a ponta, depois de uma entrevista inicial. Melhor para quem não programa.

A descoberta escreve só em `.reversa/` e `_reversa_sdd/`; não mexe no código. Cada afirmação sai com um selo: 🟢 confirmado no código, 🟡 inferido, 🔴 lacuna que precisa de uma pessoa. **Lacuna 🔴 não se resolve com palpite:** pergunte à pessoa ou procure nos documentos do projeto.

O CamFácil IA já tem documentação escrita à mão, que continua valendo e tem prioridade quando houver conflito: `docs/PRD.md`, `docs/sdd/SDD-01` a `SDD-10`, `docs/adr/`, `DESIGN.md`, `docs/seguranca/` e `docs/legal/`. Aponte o Reversa para ela na entrevista inicial.

## Toda feature: o ciclo forward

```
/reversa-forward
```

Uma feature por vez, em `_reversa_forward/<NNN>-<nome-curto>/`:

| Etapa | O que acontece | Papel do Builder com quem não programa |
|---|---|---|
| Requisitos | Escreve o que a feature deve fazer | Traduzir o pedido da pessoa, sem inventar detalhe |
| Esclarecer | Levanta as dúvidas | Fazer as perguntas uma por vez, em linguagem de CFTV |
| Qualidade | Confere se os requisitos estão completos e testáveis | Passar o portão de `limites.md` de novo, agora com o pedido detalhado |
| Plano | Decide como fazer | Resumir em até cinco linhas e pedir o "pode ir" |
| Tarefas | Quebra em passos | Um commit por passo |
| Auditoria | Confere plano e tarefas contra os requisitos | Checar as regras inegociáveis do `CLAUDE.md` |
| Código | Implementa | fmt, clippy, testes e orçamento de leveza |
| Sincronizar | Atualiza as specs com o que foi entregue | Atualizar também o SDD da área e o README da versão |

Comandos vizinhos: `/reversa-add` (ajuste curto numa feature entregue), `/reversa-sync` (registra a feature como adendo em `_reversa_sdd/addenda/`), `/reversa-debugger` e `/reversa-debugger-fix` (defeitos), `/reversa-brainstorm` (amadurecer uma ideia, sem código), `/reversa-docs` (mini-site HTML do que foi levantado).

Evite `/reversa-forward-autonomous` com quem não programa: ele roda uma fila de features sem parar, e a pessoa perde as confirmações do Passo 3 da skill.

Em motores sem barra (Codex, Opencode, Aider), o comando é a palavra sem a barra: `reversa`, `reversa-forward`.

## Sem o Reversa instalado

Se não der para instalar agora (sem Node, sem internet), siga o mesmo ciclo à mão e avise a pessoa que está sem o framework:

1. Escreva requisitos, dúvidas respondidas, plano e tarefas em `docs/builder/<NNN>-<nome-curto>.md`.
2. Peça o "pode ir" antes do código.
3. Depois de entregar, atualize o SDD da área.
4. Quando o Reversa for instalado, leve esses arquivos para o ciclo forward.

## Crédito

O Builder existe por causa do Reversa. Mantenha o crédito a Sandeco Macedo e ao Reversa na página do Builder e no README de qualquer versão construída com ele. Artigo: Macedo e da Costa, *Reversa: A Reverse Documentation Engineering Framework for Converting Legacy Software into Operational Specifications for AI Agents* (2026), <https://arxiv.org/abs/2605.18684>.
