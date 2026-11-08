# DESIGN.md — CamFácil IA

Sistema visual da dashboard local (`web/`). Fonte da verdade dos valores: [`web/tokens.css`](web/tokens.css). Contrato da superfície: `.impeccable/surfaces/web-index-html.md`.

## Direção

Painel escuro padrão do mercado de CFTV, executado no acabamento de Linear, Vercel e Frigate. Escolha do autor (2026-11-07): **base preta, verde só em detalhes**, sem "cara de IA". Sem mundo temático: o caráter vem da precisão.

Cena de uso: PC de balcão ligado o dia todo, notebook do técnico na visita, TV na loja. Por isso escuro, denso e legível de relance.

## Cor

Estratégia contida: neutros pretos e cinzas, um acento.

| Papel | Token | Valor | Uso |
|---|---|---|---|
| Chão | `--bg` | `#000000` | Fundo da página e do trilho lateral |
| Superfície | `--surface` | `#0a0a0a` | Cabeçalho de tabela, diálogo, resultado |
| Superfície 2 | `--surface-2` | `#111111` | Hover, item ativo do menu |
| Campo | `--field` | `#171717` | Inputs, select, controle segmentado |
| Linha | `--line` | `#232323` | Divisões de 1 px |
| Borda de controle | `--line-strong` | `#2e2e2e` | Botões, campos |
| Texto | `--text` | `#ededed` | Principal |
| Texto 2 | `--text-2` | `#b4b4b4` | Rótulos de formulário |
| Texto 3 | `--text-3` | `#8a8a8a` | Descrições, dados secundários |
| Ação primária | `--primary-bg` | `#ededed` | Botão principal (branco sobre preto) |
| **Acento verde** | `--accent` | `#06cb3f` | Só: ponto ao vivo, ícone do item ativo, foco, chave ligada, valor de IA ativa |
| Alerta | `--c-alert` | `#ff9c9c` | Balão de inteligência em alerta |
| Erro | `--c-danger` | `#f06a55` | Mensagens de erro, offline |
| Atenção | `--c-warn` | `#e0b341` | Estados instáveis |

Regras:
- Verde nunca preenche áreas grandes, botões ou cartões.
- Nada de faixa colorida na lateral de cartões ou avisos.
- Estados de cor só onde carregam significado.

## Tipografia

Fonte do sistema (zero download): `system-ui, Segoe UI, Roboto, Ubuntu`. Mono (`ui-monospace, Cascadia Mono, Consolas`) só para dados: IP, MAC, URL RTSP, métricas no rodapé do trilho.

| Uso | Tamanho | Peso |
|---|---|---|
| Título da página | 22 px | 600, tracking -0,02em |
| Valor de medida | 16 px | 600 |
| Corpo, menu, tabela | 14 px | 400/500 |
| Rótulo, nota | 13 px | 400 |
| Cabeçalho de tabela, legenda | 12 px | 500 |

## Estrutura

- **Trilho lateral** de 224 px: marca, quatro destinos com ícone desenhado, status ao vivo e métricas no pé. Abaixo de 760 px vira barra superior com menu rolável.
- **Cabeçalho de página**: título, descrição (é a fala do agente guia) e ação primária à direita.
- **Blocos** separados por linha de 1 px; sem cartões aninhados.
- Conteúdo em **tabelas** e **linhas chave/valor** com bordas finas, raio de 6 px.

## Zero rolagem (regra geral)
- **Nenhuma página rola.** O corpo da página ocupa exatamente a tela (`height: 100dvh; overflow: hidden`) no celular, no tablet, na TV e no monitor. Se o técnico tiver que rolar, perdemos o cliente.
- O conteúdo se **adapta ao tamanho da tela**: grade e tipografia fluidas (`clamp()`, `minmax()`, container queries), vídeo e imagem com `object-fit: contain` dentro do espaço que sobrar.
- O que não cabe vira **seção que abre** (acordeão, `<details>`), **aba**, **menu/submenu** ou **passo seguinte**. Nunca uma página comprida.
- Listas longas (câmeras, alertas, eventos, inventário) **paginam ou filtram** dentro de um espaço fixo. Rolagem interna só como último recurso, num bloco pequeno e claramente delimitado, nunca na página.
- No celular, navegação embaixo (barra de abas) e um assunto por vez na tela.
- Teste obrigatório: abrir cada tela em 360×640, 390×844, 768×1024, 1366×768 e 1920×1080 e conferir `document.documentElement.scrollHeight <= innerHeight`.

## Componentes

| Componente | Regra |
|---|---|
| Botão primário | Branco `#ededed`, texto preto, 32 px de altura |
| Botão secundário | Superfície preta, borda `--line-strong` |
| Botão fantasma | Sem fundo nem borda até o hover |
| Campo | 34 px, fundo `--field`, foco com borda verde e halo verde translúcido de 3 px |
| Controle segmentado | Trilho `--field`; opção marcada em `--surface-2` com borda interna |
| Chave liga/desliga | Cinza desligada; verde ligada; travada com cadeado na linha |
| Tabela | Cabeçalho 12 px em `--surface`; hover de linha em `--surface`; estado vazio centralizado |
| Medida | Três células em linha com divisórias de 1 px; o nível liberado em verde |
| Selo "Em breve" | Pílula com borda, texto 12 px |
| Sub-abas | Abaixo do cabeçalho da página, texto 13 px, item ativo com traço verde de 2 px embaixo; quando não cabem na largura viram um `select` ("Assunto desta tela"). Montadas por `web/tela.js` a partir de `data-sub` nos blocos |
| Seção que abre | `details.secao` com seta desenhada em CSS; um `h3.sub` dentro de um bloco vira seção automaticamente; seções irmãs abrem uma por vez |
| Paginação | Setas fantasma e "1 de 3" à direita, embaixo da lista; o tamanho da página diminui até a lista caber no painel |
| Confirmação | Diálogo da própria página (`confirmar()`), nunca `confirm()` do navegador, que o WebView do Android não mostra |
| Balão de inteligência | Pílula preta translúcida com ponto verde e o nome da regra; em alerta fica rosa, texto preto, e pisca a 1 Hz por até 10 s. Nunca caixas ou máscaras do YOLO |

## Ícones

SVG próprios em traço de 1,6 px, cantos arredondados, `currentColor`, definidos como `<symbol>` em `web/index.html`. Nada de emoji ou caracteres Unicode no lugar de ícone.

## Movimento

Só transições de 120–150 ms em cor de fundo, borda e posição da chave. O único elemento que pisca é o balão em alerta; com `prefers-reduced-motion` ele só troca de cor.

## Superfícies do navegador

Seleção de texto, cursor de digitação, barras de rolagem, anel de foco e números tabulares seguem a paleta (ver `tokens.css`).
