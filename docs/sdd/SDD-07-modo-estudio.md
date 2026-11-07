# SDD-07 — Modo estúdio

Status: proposto · após a fase 6

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
