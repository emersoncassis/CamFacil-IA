# SDD-07 — Modo estúdio

Status: em desenvolvimento (fase 7) · fundações prontas, sem interface ainda

> **Somente para estudo e aprendizagem.** Modelos criados no modo estúdio não devem ser usados em produção. Para uso real, o técnico deve procurar soluções homologadas: câmeras e gravadores com IA de borda (ex.: Intelbras) ou plataformas corporativas de IA em nuvem (ex.: Zion, 3dEYE). O aviso aparece ao abrir o modo estúdio e em todo modelo exportado.

## Objetivo
O técnico descreve em linguagem natural uma inteligência nova ("detectar caixas de banana", "contar pallets", "seguir a trajetória de empilhadeiras") e aprende vendo como ela é montada.

## Fluxo
1. **Conversa** com a IA (Anthropic como padrão; outros provedores do SDD-04 opcionais).
2. A IA **não escreve código**: gera uma configuração de módulo (`module.toml` + regras) em JSON validado por esquema pelo núcleo — objetos, zonas, linhas, trajetória, contagem.
3. **Objetos novos sem treino**: modelo de vocabulário aberto (ex.: YOLO-World) exportado para ONNX, rodando no núcleo Rust.
4. **Trajetória, contagem e zonas**: utilidades inspiradas no Supervision, reimplementadas em Rust. Sem Python na máquina do cliente.

## Treinamento de modelo próprio (Google Colab)
1. **Coletar**: o CamFácil IA captura 100–300 fotos em horários variados.
2. **Rotular**: pré-marcação automática com o modelo de vocabulário aberto; o técnico confere e corrige (aula de como a IA aprende).
3. **Treinar**: o modo estúdio gera um notebook do Colab pronto (GPU gratuita); treina um YOLO nano e salva o progresso no Google Drive do técnico (limite de sessão).
4. **Importar**: o notebook exporta ONNX; o técnico importa como módulo marcado "modelo de estudo".

## Arquitetura web (domínio próprio, Dokploy)
| Parte | Onde roda | Responsabilidade |
|---|---|---|
| Web app + API pequena | Servidor do projeto (Dokploy) | Conversa com a Anthropic, gera o notebook. Chave de API nunca vai ao navegador; cada técnico usa a própria chave, guardada cifrada |
| Interface | Navegador do técnico | Conversa, vídeo, conferência de rótulos, pré-marcação em WebAssembly |
| Treino | Google Colab | No Drive do próprio técnico |

**As imagens dos clientes vão direto do navegador para o Drive do técnico** e não passam nem ficam no servidor do projeto.

## Bloqueios
- Reconhecimento facial e qualquer biometria: **bloqueados** no modo estúdio (dado sensível, LGPD art. 11).
- Aceite obrigatório do [termo de responsabilidade do técnico](../legal/TERMO-RESPONSABILIDADE-TECNICO.md) e registro da [autorização do cliente](../legal/TERMO-AUTORIZACAO-CLIENTE.md) antes de coletar imagens.
- Botão "apagar tudo" ao fim do estudo (fotos, rótulos, pacote).

## Situação da implementação (fase 7)

| Parte | Situação | Onde |
|---|---|---|
| Receita de módulo (JSON v1, validação pt-BR, bloqueio de biometria, SHA-256) | **Pronto** | `crates/cf-core/src/receita.rs`, `docs/esquemas/receita-v1.schema.json`, [ADR 0013](../adr/0013-formato-da-receita-do-estudio.md) |
| Gerador do notebook do Colab (Drive, fotos YOLO, YOLO nano com checkpoints no Drive, ONNX + manifesto com hash, "apagar tudo") | **Pronto** | `crates/cf-core/src/notebook.rs`; no site: `POST /api/estudio/notebook` |
| Importar modelo de estudo no programa local (receita + manifesto, depois o ONNX; confere hash; desligado; exige termo do técnico) | **Pronto, sem executar** | `crates/camfacil/src/estudio.rs`, `/api/estudio/modelos` |
| Rodar o modelo de estudo (classes próprias) | Pendente: o detector só conhece as classes do COCO | `crates/camfacil/src/ia/detector.rs`, `cf_core::deteccao` |
| Compartilhar receitas no site (publicar, listar, baixar, apagar; RLS) | **Pronto (API)** | `crates/cfia-api/src/estudio.rs`, migration `0004_receitas_estudo.sql` |
| Interface do estúdio (conversa, rotular, galeria de receitas) | Pendente; `/estudio` continua mockup | `crates/cfia-api/web/estudio.*` |
| Conversa com a IA gerando receita, vocabulário aberto, pré-marcação em WebAssembly | Pendente | — |

### API do site
- `POST /api/estudio/receitas` (logado) `{receita, autor?, publica?}` — publica ou atualiza pela `id` da receita.
- `GET /api/estudio/receitas?q=` — públicas; `GET /api/estudio/receitas/minhas` (logado).
- `GET /api/estudio/receitas/{id}/receita.json` — baixa (conta download); `DELETE /api/estudio/receitas/{id}` (dono).
- `POST /api/estudio/notebook` `{receita}` — devolve o `.ipynb`; nada fica no servidor.

### API do programa local
- `GET /api/estudio/modelos` — lista com aviso e `executa: false`.
- `POST /api/estudio/modelos` `{receita, manifesto?}` → `PUT /api/estudio/modelos/{id}/onnx` (corpo = arquivo, até 64 MB) → `DELETE /api/estudio/modelos/{id}`.
