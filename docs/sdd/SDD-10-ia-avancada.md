# SDD-10 — Módulo IA avançada

Status: **proposta** (nada implementado) · complementa o [SDD-02](SDD-02-modulos-ia.md) e o [SDD-09](SDD-09-inteligencias-por-regra.md)

## Problema

A IA básica (módulo "Pessoas e veículos" + regras por inteligência) resolve, mas o técnico ainda decide no escuro:

- não sabe se a imagem da câmera **presta** para a inteligência que escolheu (escura, estourada, desfocada, objeto pequeno demais);
- não sabe se deve usar o stream extra ou o principal;
- ajusta a sensibilidade por tentativa e erro;
- não tem como **provar** ao cliente que a regra funciona antes de entregar.

O módulo avançado é um **assistente de validação**, coerente com o propósito do projeto (aprender e validar, não produto final).

## Decisão de arquitetura: um cérebro, ajustes por canal

Levantamento de mercado (ver Observações): Frigate, Viseron, Blue Iris + CodeProject.AI e Milestone/Axxon mantêm **um motor de IA compartilhado** e guardam a **configuração por câmera**. Hanwha, Hikvision e Dahua rodam na própria câmera, portanto por canal por natureza.

Para o CamFácil:

1. **Um motor só** (YOLO nano, carregado uma vez). Não existe "cérebro por câmera"; isso multiplicaria RAM e fere a regra de leveza.
2. **Ajustes por câmera** (stream, taxa, confiança mínima, tamanho mínimo, agenda).
3. **Ajustes por regra** continuam como no SDD-09 (inteligência, classes, parâmetro, cor, som, ligada ou desligada).
4. O módulo avançado vem **desligado por padrão** e **não roda em segundo plano**: cada análise é um comando do técnico (botão), com duração curta. Núcleo ocioso continua < 50 MB e < 2% CPU.

## Funcionalidades

### A. Diagnóstico da imagem ("essa câmera presta?")

Roda sobre alguns quadros (5 a 10 s) e mostra nota e motivo, em linguagem do instalador.

| Verificação | Como medir (sem rede neural nova) | Mensagem típica |
|---|---|---|
| Claridade | brilho médio e histograma (já existe `saude()`) | "Muito escura: a IA perde pessoas à noite" |
| Estouro / contraluz | fração de pixels saturados | "Contraluz: pessoas viram silhueta" |
| Contraste | desvio padrão (já existe) | "Imagem lavada" |
| Foco e nitidez | variância do Laplaciano (já existe) | "Desfocada ou lente suja" |
| Ruído | variância em áreas lisas, entre quadros | "Muito ruído; reduza o ganho da câmera" |
| Câmera obstruída ou movida | comparação com imagem de referência salva | "A cena mudou: câmera movida ou tapada" |
| Tamanho do objeto | altura em pixels das detecções reais na região da regra, no tamanho do stream | "Carros nesta linha têm ~28 px: use o stream principal" |

Os limites de nota (por exemplo, altura mínima de pessoa em pixels) são **hipóteses a calibrar** com imagens reais; começam conservadores e viram constantes documentadas após o teste de campo.

### B. Veredito por inteligência

Cruza o diagnóstico com a inteligência e as classes da regra:

- "Boa para passagem de pessoa; ruim para carro à noite";
- "Para este ponto, o stream extra basta" ou "troque para o principal";
- quando a nota é ruim, diz **o que ajustar** (iluminação, ângulo, exposição, posição da linha), nunca só reprova.

### C. Teste ao vivo da regra (30 s) e relatório para o cliente

Roda a regra escolhida por 30 s e mostra: quantas detecções, confiança média, quantas trilhas perdidas, ms por quadro. Reaproveita o avaliador do SDD-09 (`avaliacao.rs`).

**O relatório é exportável** (decisão do Emerson). Serve para o instalador mostrar ao cliente que a câmera não está num bom lugar ou mal instalada e que precisaria ser reposicionada ou reinstalada. Como isso costuma gerar custo e o cliente resiste, o relatório é redigido como **aviso técnico**, não como acusação:

- cabeçalho: cliente/local (campo livre), câmera, data e hora, versão do programa;
- foto do quadro analisado, sem caixas do YOLO (regra 4: o cliente vê só o balão com o nome da regra);
- semáforo por item (claridade, foco, ruído, tamanho do objeto) com a frase em português simples;
- resultado do teste de 30 s em números simples (quantas passagens a regra viu, confiança média);
- **recomendação em uma frase** ("reposicionar a câmera mais baixa", "melhorar a iluminação do ponto", "trocar para stream principal");
- rodapé: "Aviso técnico gerado automaticamente. Não substitui laudo de instalação."
- formatos: imagem (PNG) e página para impressão/PDF, gerados no próprio programa, sem dependência nova pesada (a conferir na Fase 2).

### D. Ajustes por câmera

- **Stream de análise**: extra ou principal, com o aviso de custo já usado no ao vivo. O modelo reduz para 640×640; o principal só ajuda de verdade com objeto pequeno ou distante.
- **Stream por regra** (decisão do Emerson): cada regra declara o que precisa — *automático* (padrão), *extra* ou *principal*. Uma câmera decodifica **um** stream de análise por vez (decodificar dois dobraria o custo), então vale a regra mais exigente entre as regras ligadas:
  1. se alguma regra ligada pede *principal*, a câmera analisa no principal e o programa **avisa** quais regras causaram isso e quanto pesa;
  2. se a regra pede *principal* e a câmera está no extra, o programa **solicita a mudança** (botão "Trocar para o principal") ou, se o técnico recusar, **deixa o aviso** na regra ("pode perder carro distante");
  3. desligar a regra exigente (SDD-09) devolve a câmera ao stream leve;
  4. no *automático*, o veredito B sugere o stream pelo tamanho do objeto medido naquela regra.
  Nunca troca sozinho sem avisar: a mudança sempre aparece para o técnico.
- **Taxa de análise** (quadros por segundo analisados).
- **Confiança mínima** e **tamanho mínimo/máximo** da caixa (corta falso positivo).
- **Agenda** (dias e horários em que as regras valem).
- **Cooldown** entre alertas repetidos.

### E. Saúde da câmera (alerta opcional)

Avisa quando a câmera escureceu, desfocou, foi movida ou saiu do ar (usa os mesmos cálculos de A, amostrados de tempos em tempos, desligado por padrão).

### F. Ideias para depois (não entram na primeira versão)

- **Fatiamento da imagem** para objeto pequeno (inferência em blocos): melhora a detecção distante, mas multiplica o custo; entra só como opção "pesada" com aviso.
- **Suavização** das detecções entre quadros (menos piscar de caixa e de contagem).
- **Cor do veículo** (histograma da região do carro).
- **Anomalia**: carro parado em lugar incomum.
- **Resumo em texto** com modelo de visão e linguagem: exige modelo grande e provavelmente nuvem; fere offline-first. Só como integração externa opcional, fora do núcleo.

## Fora do escopo (PRD §6 e regras)

Busca em gravações, gravação contínua, painel multicliente, ronda virtual e dependência de nuvem para o essencial. Não há Python nem Node no caminho de execução: a biblioteca Supervision (Python) serve só como **referência de conceitos**; a implementação é em Rust.

## Interface (regra 9, zero rolagem)

Tudo numa tela de `100dvh`:

- botão "Avaliar imagem" na câmera abre um cartão com abas: **Imagem**, **Veredito**, **Teste**, **Ajustes**;
- resultado em semáforo (ok, atenção, problema) com uma frase por item; detalhes em acordeão;
- textos em `pt-BR.json`, cores só por tokens.

## Orçamento de recursos

| Item | Custo esperado |
|---|---|
| Módulo ligado e ocioso | 0 a mais (nada roda sozinho) |
| Diagnóstico A/B | alguns quadros, < 1 s de CPU |
| Teste C (30 s) | o mesmo da regra comum; sem modelo extra |
| Saúde E | 1 amostra a cada N minutos por câmera |
| Novas dependências | nenhuma prevista (cálculos em Rust sobre `Quadro`) |

Medir com `scripts/budget.sh` antes de fechar cada fase.

## Fases

1. **Fase 1**: A (claridade, estouro, foco, ruído, obstrução) + stream por câmera e por regra com aviso (D).
2. **Fase 2**: tamanho do objeto em pixels + veredito B + teste C + relatório exportável (aviso técnico).
3. **Fase 3**: confiança, tamanho mínimo, agenda e cooldown por câmera; saúde E.
4. **Fase 4**: itens de F, um por vez, cada um com ADR e medição de custo.

## Critérios de aceite da Fase 1

- Imagem escura, desfocada e estourada de teste recebem o diagnóstico certo (testes com quadros sintéticos).
- Escolher o stream por câmera muda o que o detector lê, sem reiniciar o programa; regra que pede *principal* numa câmera no extra gera o aviso ou o pedido de troca.
- Núcleo ocioso dentro do orçamento com o módulo ligado.
- Nenhuma tela rola, de 360×640 a 1920×1080.

## Perguntas em aberto

Decididas pelo Emerson em 08/11/2026:

- Stream: **também por regra**, com a câmera seguindo a regra mais exigente, aviso ou pedido de troca, nunca troca silenciosa (seção D).
- Relatório do teste C: **exportável** para o cliente, como aviso técnico (seção C).

Ainda abertas:

1. **Câmeras de referência para calibrar as notas:** sem resposta ainda. Proposta: a primeira versão sai com limites conservadores e marcados como "a calibrar"; o teste de campo coleta, **só com autorização do técnico e guardado localmente**, os números (não as imagens) de cada análise (altura em pixels, nitidez, brilho, se a detecção acertou) para ajustar os limites depois.
2. O relatório deve levar o nome da empresa do instalador (logo)? Texto livre no rodapé?
3. Como tratar duas regras na mesma câmera com pedidos opostos (uma pede extra, outra pede principal)? Proposta: vale o principal, com o aviso de custo (já descrito).

## Observações — fontes consultadas

Levantamento feito em 08/11/2026. As buscas devolveram títulos e links; o conteúdo comparativo entre produtos combina o que foi lido com conhecimento prévio sobre cada produto e deve ser conferido na documentação antes de virar compromisso de funcionalidade.

- [Milestone AI Bridge: assinar análises e receber dados de volta](https://doc.milestonesys.com/AIB/Help/latest/en-US/feature_flags/ff_aibridge/aib_adm_config_analyticsdatafromaib.htm)
- [Hanwha AI camera plug-in com Milestone XProtect](https://theboringlab.com/configure-hanwhas-ai-camera-plugin-with-milestone-xprotect/)
- [CVEDIA RT plugin para Milestone XProtect (manual)](https://bin.cvedia.com/docs/CVEDIA-RT-Plugin-Milestone-XProtect-User-Manual-2024.2.5.pdf)
- [Viseron vs Frigate: NVR com IA comparados](https://selfhosting.sh/compare/viseron-vs-frigate/)
- [Frigate NVR: visão geral](https://selfhosted-sh.pages.dev/frigate-nvr-security-cameras)
- [Blue Iris + CodeProject AI: configuração e dicas](https://agrariansolutions.edencreative.co/?p=11839)
- [AQuA: Analytical Quality Assessment for Optimizing Video Analytics Systems](https://arxiv.org/pdf/2101.09752)
- [Quality In, Accuracy Out: vídeo analítico em imagens de alta qualidade (SIA)](https://www.securityindustry.org/2020/03/05/quality-in-accuracy-out-applying-video-analytics-to-high-quality-images-for-effectiveness/)
- [Roboflow Supervision (GitHub, licença MIT)](https://github.com/roboflow/supervision) e [documentação](https://supervision.roboflow.com/latest/)

### O que a biblioteca Supervision acrescentou

Confirmado nas páginas do repositório e da documentação: objeto de detecção unificado e independente de modelo, contagem e filtro por **zonas poligonais**, **contagem de cruzamento de linha**, **rastreamento com ID persistente** (ByteTrack), tutoriais de **tempo em zona**, **estimativa de velocidade** com transformação de perspectiva, **detecção de objetos pequenos** (fatiamento), **métricas** (mAP e matriz de confusão) e conversão de datasets (YOLO, COCO, Pascal VOC).

Do conhecimento prévio, a conferir na documentação: suavização de detecções, mapa de calor, monitor de FPS e gravação de resultados em CSV/JSON.

Ideias aproveitadas neste SDD: fatiamento (F), suavização (F), métricas de qualidade para o teste C, tempo em zona (já coberto pela permanência) e velocidade por perspectiva (possível extensão futura, exige calibrar a cena).
