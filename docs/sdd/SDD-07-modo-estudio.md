# SDD-07 — Modo estúdio

Status: em desenvolvimento (fase 7) · versão básica funcionando de ponta a ponta (sem conversa com IA)

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
| Assistente `/estudio` em seis passos, tela única sem rolagem (360×640 a 1920×1080) | **Pronto (básico)** | `crates/cfia-api/web/estudio.{html,css,js}` |
| 1. Descrever: formulário guiado (nome, lugar, objetos, regra linha/zona/contagem) → receita conferida pelo núcleo, erros em pt-BR | **Pronto** | `POST /api/estudio/validar` (mesma validação do `cf-core`) |
| 2. Coletar: fotos de uma câmera em intervalos, numa pasta local, sem enviar | **Pronto** | programa local: `crates/camfacil/src/estudio.rs`, Módulos → Modo estúdio |
| 3. Rotular no navegador (abrir pasta/fotos, caixas por classe, teclado, "nada nesta foto", exporta `.zip` YOLO `images/ labels/ data.yaml` montado em JS) | **Pronto**; fotos nunca vão ao servidor | `estudio.js` (zip "store" próprio) |
| 4. Treinar: baixa o `.ipynb` e abre o Colab | **Pronto** | `POST /api/estudio/notebook` |
| Importar modelo de estudo no programa local (receita + manifesto, depois o ONNX; confere hash; exige termo do técnico) | **Pronto** | `crates/camfacil/src/estudio.rs`, `/api/estudio/modelos`, Módulos → Modo estúdio |
| Rodar o modelo de estudo (classes próprias, ligado à mão, um por vez, nome da classe no balão da regra, nunca caixas para o cliente) | **Pronto** | `crates/camfacil/src/ia/detector.rs`, `cf_core::deteccao::{decodificar_estudo, CLASSE_ESTUDO}` |
| Compartilhar receitas no site (publicar logado, galeria pública com busca, paginação, baixar e "usar") | **Pronto** | `crates/cfia-api/src/estudio.rs`, migration `0004_receitas_estudo.sql` |
| Conversa com a IA gerando receita (chave do técnico cifrada) | Pendente; botão aparece desabilitado e explicado | — |
| Vocabulário aberto e pré-marcação em WebAssembly | Pendente | — |
| Posição da linha/zona vinda da receita | Pendente: o técnico desenha a regra em Câmeras; a receita leva posições padrão | — |

### Como o modelo de estudo roda
- Ligar um modelo (`POST /api/estudio/modelos/{id}/ligar {ligado}`) desliga os outros e avisa o detector, que troca a sessão ONNX na hora: um modelo por vez, sem dobrar a RAM. Volta ligado depois de reiniciar o programa.
- Mesma saída do YOLO do Ultralytics (`[1, 4+nc, N]`), `nc` = classes da receita; lado e confiança da receita. Saída com `nc` diferente vira erro claro no estado do detector.
- As classes entram como `CLASSE_ESTUDO + i` (1000+), para nunca se confundirem com o COCO (`0` = pessoa). Zona, permanência e calor contam pessoa ou qualquer objeto de estudo.
- Balão da regra: "Doca · empilhadeira"; contagem do painel: "2 × pallet". Caixas só na ferramenta de rotular do técnico.

### API do site
- `POST /api/estudio/receitas` (logado) `{receita, autor?, publica?}` — publica ou atualiza pela `id` da receita.
- `GET /api/estudio/receitas?q=` — públicas; `GET /api/estudio/receitas/minhas` (logado).
- `GET /api/estudio/receitas/{id}/receita.json` — baixa (conta download); `DELETE /api/estudio/receitas/{id}` (dono).
- `POST /api/estudio/validar` `{receita}` — confere e devolve a receita canônica e o SHA-256 (ou `erros` em pt-BR); nada fica no servidor.
- `POST /api/estudio/notebook` `{receita}` — devolve o `.ipynb`; nada fica no servidor.

### API do programa local
- `GET /api/estudio/modelos` — lista com aviso, `ativo` e `executa` por modelo.
- `POST /api/estudio/modelos` `{receita, manifesto?}` → `PUT /api/estudio/modelos/{id}/onnx` (corpo = arquivo, até 64 MB) → `POST /api/estudio/modelos/{id}/ligar` `{ligado}` → `DELETE /api/estudio/modelos/{id}`.
- `POST /api/estudio/coleta` `{camera_id, receita, intervalo_s (2–3600), total (1–500)}` — guarda fotos em `pasta de dados/estudio/<receita>/fotos` (exige termo do técnico; uma coleta por vez); `GET /api/estudio/coleta`; `POST /api/estudio/coleta/parar`; `DELETE /api/estudio/coleta/{receita}` ("apagar tudo" das fotos).

## Aplicação futura: VMS sob medida
Só desenho na página `/estudio#vms`; nada funciona ainda. Ideia: o técnico usa um assistente de código (Claude Code, Codex ou outro) ligado ao CamFácil IA pelo MCP (ADR 0012) para montar um VMS próprio sobre a base do CamFácil IA: IA local já testada e integração nativa com as APIs de gravadores e câmeras (Intelbras, Hikvision, Dahua e outros). Antes de implementar: ADR própria, escopo frente ao PRD §6 e quais ferramentas MCP de fabricante entram.
