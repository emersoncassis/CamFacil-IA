# Design System — CamFácil IA

A identidade visual do CamFácil IA é **inspirada** na paleta da Intelbras, onde o projeto nasceu (o autor foi instrutor técnico lá). É uma homenagem de origem: o CamFácil IA **não usa** a marca, o logo nem o nome da Intelbras, e não é afiliado a ela.

Referência das cores originais: [Brandbook Intelbras — Paleta Cromática](https://brandbook.intelbras.com/personalidade/cores/index.html).

## Princípios

1. **Leve antes de bonito.** Nada de framework de UI, fontes externas ou imagens pesadas. CSS puro, um arquivo, variáveis.
2. **Escuro por padrão.** A dashboard fica aberta em balcão e em TV o dia todo; o tema escuro cansa menos e destaca os alertas.
3. **Verde = inteligência ativa.** O verde só aparece onde há IA funcionando, ações principais ou sucesso. Não é decoração.
4. **Alerta tem de chamar atenção.** O balão de alerta pisca e troca de cor; nada mais na tela pisca.
5. **Ensinar é parte da interface.** O agente guia tem cor própria (azul céu), para o técnico reconhecer sempre que é uma explicação.

## Paleta

| Token | HEX | Origem na paleta Intelbras | Uso |
|---|---|---|---|
| `--cf-green` | `#06CB3F` | Verde Institucional | Ação principal, IA ativa, sucesso |
| `--cf-green-soft` | `#85FFC5` | Verde Auxiliar | Hover, realces, selos |
| `--cf-green-deep` | `#035144` | Verde Revendas | Superfícies elevadas no escuro |
| `--cf-ink` | `#163134` | Verde Grandes Projetos | Fundo principal (escuro) |
| `--cf-black` | `#0B1A1C` | derivado do Verde Grandes Projetos | Fundo de vídeo e quiosque |
| `--cf-gray-100` | `#EFEFED` | Cinza-Claro | Texto no escuro / fundo no claro |
| `--cf-gray-400` | `#B9B5B4` | Cinza-Médio | Texto secundário, bordas |
| `--cf-sky` | `#31BFEB` | Azul Céu | Agente guia, dicas, aulas |
| `--cf-lime` | `#D9FF82` | Lima | Gamificação (selos, ranking) |
| `--cf-alert` | `#FF9C9C` | Pêssego | Alerta (fundo do balão) |
| `--cf-danger` | `#BC431B` | Laranja | Erro, conflito de IP, câmera offline |
| `--cf-warn` | `#C7AC3E` | Ocre | Atenção (máquina no limite, senha padrão) |

Contraste: texto `--cf-gray-100` sobre `--cf-ink` passa AA. Verde `--cf-green` sobre `--cf-ink` é usado em botões com texto `--cf-black`.

## Tipografia

Fonte do sistema (sem download): `system-ui, -apple-system, "Segoe UI", Roboto, Ubuntu, sans-serif`. Números (IP, FPS, CPU) em `ui-monospace, "Cascadia Mono", Consolas, monospace`.

| Nível | Tamanho | Peso |
|---|---|---|
| Título de tela | 24 px | 700 |
| Seção | 18 px | 600 |
| Corpo | 15 px | 400 |
| Legenda / dados | 13 px | 400 |
| Quiosque (TV) | 32–64 px | 700 |

## Espaçamento e forma

- Escala de 4 px: 4, 8, 12, 16, 24, 32.
- Raio: 8 px em cartões, 999 px em balões e selos.
- Sombras evitadas (custo de renderização em máquina fraca); usar bordas de 1 px.

## Componentes-chave

| Componente | Regra |
|---|---|
| **Balão de inteligência** | Pílula translúcida (`--cf-ink` a 80%) com o nome da regra e um ponto verde. Em alerta: fundo `--cf-alert`, texto `--cf-black`, pisca a 1 Hz por até 10 s. Nunca desenha caixas ou máscaras do YOLO. |
| **Botão liga/desliga de módulo** | Desligado: cinza. Ligado: verde. Bloqueado pela máquina: cadeado + texto "sua máquina não aguenta este nível". |
| **Cartão do agente guia** | Borda esquerda `--cf-sky`, ícone de alto-falante, botão "ouvir de novo". |
| **Selo** | Círculo `--cf-lime` com ícone do módulo. |
| **Status de câmera** | Ponto: verde online, ocre instável, laranja offline. |
| **Logo do técnico** | Canto inferior direito, até 48 px de altura, com QR code opcional ao lado. |

## Animação

- Só `opacity` e `transform` (baratos para a GPU).
- `prefers-reduced-motion`: o balão troca de cor sem piscar.

## Tokens

Os tokens prontos estão em [`web/tokens.css`](../web/tokens.css). Todo CSS do projeto usa apenas essas variáveis.
