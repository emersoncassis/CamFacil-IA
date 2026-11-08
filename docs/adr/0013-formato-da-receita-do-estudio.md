# ADR 0013 · Formato da receita do modo estúdio

**Situação:** aceita · 2026-11-07 · detalha o [SDD-07](../sdd/SDD-07-modo-estudio.md)

> O número 0012 fica reservado para a fase 6c, desenvolvida em paralelo.

## Contexto

O modo estúdio quer que o técnico monte a própria inteligência (objetos, regras, treino no Colab) e **compartilhe o que aprendeu** com outros técnicos. Para isso é preciso um artefato pequeno, legível e conferível que:

- descreva o que o modelo aprende e como a cena é lida (zonas, linhas, trajetórias, contagens);
- gere o notebook do Colab e acompanhe o ONNX exportado;
- possa ser publicado no site **sem nunca carregar fotos de clientes**;
- deixe claro, sempre, que é **somente estudo**;
- impeça reconhecimento facial e biometria (dado sensível, LGPD art. 11).

## Decisão

1. **Receita em JSON, versão 1** (`formato: "camfacil-receita"`, `versao: 1`), com: `id` (slug, vira pasta no Drive e nome do ONNX), `nome`, `descricao`, `somente_estudo: true`, `aviso` (texto fixo), `modelo` (`base` yolo11n/yolov8n, `lado`, `classes` na ordem dos índices, `confianca`), `objetos`, `zonas`, `linhas`, `trajetorias`, `contagens` e `estudo` (apelido do autor, tipo de local, nº de fotos, épocas, métricas, notas). Coordenadas normalizadas de 0 a 1.
2. **Validação oficial em Rust** (`cf_core::receita`), mensagens em pt-BR, todas de uma vez. O esquema JSON (`docs/esquemas/receita-v1.schema.json`, servido em `/esquemas/receita-v1.schema.json`) documenta o formato para ferramentas externas; um teste garante que ele e a struct não divergem. Não usamos crate de JSON Schema: seria dependência grande para regras que precisam de código de qualquer forma (referências entre regras, área de polígono, bloqueio de termos).
3. **Sem fotos por construção:** `deny_unknown_fields` em todos os níveis, receita de até 64 KB, textos com limite e recusa de "palavra" acima de 120 caracteres ou `data:image` (base64 escondido). A receita guarda só a **quantidade** de fotos.
4. **Bloqueio de biometria:** lista de termos (sem acento, por palavra, com prefixos) checada em id, nome, descrição, objetos, classes, regras e notas: rosto, face/facial, biometria, íris, retina, impressão digital, emoção, etnia/raça/cor da pele, gênero/sexo, "identificar/reconhecer pessoa", CPF etc. Detectar **pessoa** como objeto continua permitido; identificar **quem** é a pessoa, não. Conservador de propósito: um falso positivo ("face da caixa") só pede outra palavra.
5. **Hash:** SHA-256 do JSON canônico (ordem fixa dos campos do serde). O notebook embute a receita como literal e confere o hash antes de treinar; o manifesto do Colab registra `receita_sha256` e `onnx_sha256`. O programa local confere o `onnx_sha256` ao importar.
6. **Mesma validação nos dois lados:** o `cfia-api` passa a depender do `cf-core` (o binário só liga o código usado; o build Docker passa a compilar também o SQLite embutido do `cf-core`, alguns segundos a mais, e copia `docs/esquemas`).
7. **Compartilhamento no site:** tabela `receitas_estudo` com RLS (leitura pública só do que é `publica`, escrita só do dono, contagem de downloads pelo sistema) e um contexto novo `Ctx::Publico` para visitantes sem login. Autor é um apelido opcional, nunca e-mail.

## Consequências

- O técnico tem um caminho completo de estudo: receita → notebook → ONNX + manifesto → importar no programa local → publicar a receita com as métricas.
- **O modelo importado ainda não roda:** o detector local só conhece as classes do COCO (`cf_core::deteccao::CLASSES`). O modelo fica guardado, conferido e marcado "modelo de estudo", desligado. Executar classes arbitrárias é a próxima etapa (pós-processamento com `nc` variável e balão com o nome da receita).
- Mudanças incompatíveis no formato sobem `versao` e ganham nova ADR; leitores antigos recusam com mensagem clara.
- A lista de termos bloqueados é heurística e pode ser contornada por quem quer burlar; ela evita o uso ingênuo e documenta a regra. A responsabilidade continua coberta pelo termo do técnico (`docs/legal/`), exigido também para importar modelo de estudo.
