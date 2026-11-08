# SDD-09 — Inteligências por regra

Status: **implementado** (0.9.0) · complementa o [SDD-02](SDD-02-modulos-ia.md) · decisão na [ADR 0016](../adr/0016-uma-inteligencia-por-regra.md)

## Problema

Até a 0.8.x o módulo (contagem, permanência, calor) era **global**: ligou, vale para toda regra. A linha contava qualquer coisa que o detector visse; a zona só pessoa. O técnico não conseguia dizer "esta linha é só de carro", "esta zona avisa quando passar de 3 carros" ou "esta zona só mede permanência de moto". O som era um bipe fixo e a linha tinha sempre a mesma cor.

## Regra do jogo

1. **Uma regra, uma inteligência.** Escolhida ao criar. Não dá para juntar permanência com densidade na mesma zona; para isso, duas regras.
2. **A regra diz quem observa.** Pessoa, carro, moto, bicicleta, ônibus, caminhão (vários ao mesmo tempo). O que não foi marcado é ignorado pela regra.
3. **Só deixa escolher o que está disponível.** Cada inteligência depende de um módulo ligado; sem ele a opção aparece apagada, dizendo qual módulo ligar. O servidor também recusa (409).
4. **Mostra o peso** (leve, médio, pesado) para o técnico saber o que o PC velho aguenta.
5. **Cor e som por regra**, escolhidos na criação e trocáveis depois.

## Catálogo

| Inteligência | Forma | Peso | Módulo exigido | Parâmetro | O que dispara |
|---|---|---|---|---|---|
| Movimento (padrão antigo) | linha ou zona | leve | nenhum (usa o movimento) | — | movimento na linha/zona, sem saber o que é |
| Passagem | linha | leve | Contagem | — | quem cruza, com o sentido |
| Presença | zona | leve | Pessoas e veículos | — | entrou / ficou livre |
| Densidade | zona | médio | Pessoas e veículos | quantos (2–50, padrão 3) | N ou mais dentro ao mesmo tempo (uma vez ao passar do limite) |
| Permanência | zona | médio | Permanência | segundos (5–3600, padrão 60) | parado dentro por N segundos (uma vez por objeto) |
| Mapa de calor | zona | pesado | Mapa de calor | — | acumula por onde passa, só dentro da zona e só das classes marcadas |

"Pesado" aqui é o custo no PC: a grade fica acumulando e a tela consulta; nenhuma delas roda modelo a mais, o detector é um só (YOLO nano) e todas leem o que ele já viu.

Combinações que a tela e a API recusam: densidade/permanência/presença/calor em linha; passagem em zona; classe fora do catálogo (a não ser classe de modelo de estudo, quando há um ligado); cor ou som fora da lista.

## Dados

Coluna `regras.ajustes` (JSON, migração automática; vazio = regra antiga):

```json
{ "intel": "densidade", "classes": [2], "parametro": 3, "cor": "ambar", "som": "sino" }
```

- `intel`: `legado` (padrão), `passagem`, `presenca`, `permanencia`, `densidade`, `calor`.
- `classes`: índices do COCO (0 pessoa, 1 bicicleta, 2 carro, 3 moto, 5 ônibus, 7 caminhão) ou ≥ 1000 de modelo de estudo. Vazio = comportamento de antes (linha conta qualquer classe, zona conta pessoa e objetos de estudo).
- Regras antigas viram `legado` e continuam funcionando; permanência global continua valendo nelas.
- O backup (`/api/backup`) leva os ajustes; backup antigo sem o campo restaura como legado.

## API

- `GET /api/inteligencias` → catálogo com `disponivel`, `peso`, `parametro`, classes, sons e cores.
- `POST /api/cameras/{id}/regras` aceita `intel`, `classes`, `parametro`, `cor`, `som` (todos opcionais).
- `PATCH /api/regras/{id}` aceita `classes`, `parametro`, `cor`, `som` (a inteligência não muda; para outra, crie outra regra).
- MCP `criar_regra` ganha os mesmos campos.

## Núcleo (`cf-core`)

`Rastreador::processar` percorre as regras; cada uma filtra as trilhas por `regra_casa(regra, classe)` e executa só a sua inteligência. Eventos novos: `Densidade { regra, quantas, classe }`; `Permanencia` ganhou `classe` (o aviso diz "carro parado há 65 s"). O mapa de calor, havendo regras de calor, marca só as trilhas das classes dentro dessas zonas.

## Tela

Formulário da regra em três abas, num cartão por cima do vídeo (cabe de 360×640 a 1920×1080, sem rolar a página — regra 9):

1. **O que faz** — nome e a lista de inteligências com peso e bloqueio.
2. **Quem** — classes e o parâmetro (quantos / segundos).
3. **Alerta** — textos "ocupada/livre", cor da linha (verde, âmbar, vermelho, azul, branco, magenta) e som, com botão "Ouvir".

Sons (gerados na hora, sem arquivo): bipe, sino, alarme, gota, sem som. Valem no Ao vivo, na tela de Câmeras e no Modo TV. As cores são tokens `--linha-*` em `tokens.css`.

## IA, sugira para mim

A observação passa a usar as classes do detector (quando ele está olhando) e decide **o que sugerir**:

- Classe que **se move mais do que para** (mínimo 2 e 20 % do fluxo) → linha de **passagem**, só dessas classes ("3 pessoas e 2 carros passaram").
- Classe que **chega e fica parada** → zona de **permanência** daquela classe, com o tempo sugerido = 60 % do tempo típico de quem parou (mínimo 10 s).
- Sem detector, cai no comportamento antigo (movimento, tudo conta como pessoa).
- Ao aceitar, a regra já nasce com inteligência, classes e tempo.

## Editar depois de criada

Botão **Editar** na lista de regras abre o mesmo formulário já preenchido: nome, textos, classes, tempo, cor e som mudam (`PATCH /api/regras/{id}`). A inteligência fica fixa.

## Alertas da própria câmera

Módulo `camera` ("Alertas da câmera"), desligado por padrão como todos. Com ele ligado, uma conexão por aparelho escuta a API de eventos do fabricante (Intelbras/Dahua `eventManager.cgi`, Hikvision `alertStream`), com login Digest/Basic. Cada alerta (ignorando batimento e perda de vídeo) vira evento `camera` "… (recebido da câmera)", no máximo um por tipo a cada 10 s por câmera, e segue para o Telegram como os demais. O canal do alerta é casado com o canal da câmera no gravador. Desligar o módulo fecha as conexões. Validado só contra um servidor de teste que imita a Intelbras; falta confirmar em equipamento real.

## Fora de escopo

Densidade por área (pessoas por metro quadrado), placas e reconhecimento de modelo/cor de veículo, e juntar inteligências na mesma regra. Quem precisa disso usa as regras em paralelo ou parceiros (PRD §6).

## Critérios de aceite (todos verificados)

- Linha só de carro ignora pessoa; densidade só dispara com o limite e só da classe; permanência só do carro e sem aviso de entrada; calor só da classe dentro da zona (testes em `cf-core`).
- IA sugira: carro que estaciona vira permanência de carro; pessoa que passa vira passagem de pessoa.
- API recusa módulo desligado (409), forma errada, classe/cor/som inválidos (400).
- Regra criada sem os campos novos segue como antes.
- Formulário sem rolagem em 360×640, 390×844, 768×1024, 1366×768 e 1920×1080.
