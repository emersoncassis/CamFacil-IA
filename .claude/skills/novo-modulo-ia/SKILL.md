---
name: novo-modulo-ia
description: Cria o esqueleto de um novo módulo de IA do CamFácil IA (module.toml, implementação do trait Module, testes e aula). Use quando pedirem um módulo novo de visão computacional.
---

# Novo módulo de IA

1. Leia só `docs/sdd/SDD-02-modulos-ia.md` (contrato) e um módulo existente como referência.
2. Pergunte (ou deduza do pedido): id, nome, nível (leve/médio/pesado), se precisa de modelo ONNX.
3. Crie `modules/<id>/module.toml` seguindo o contrato.
4. Implemente o trait `Module` em `crates/cf-modules/src/<id>.rs` e registre no índice de módulos.
   - Só detecta e emite `Event`; nunca desenha caixas, nunca envia notificações.
   - Reaproveite o pré-processamento e o rastreamento já existentes.
5. Teste com um vídeo curto em `tests/fixtures/` (sem rede).
6. Acione o agente `autor-de-aulas` para a aula do módulo.
7. Acione o agente `guardiao-da-leveza` antes do PR.
8. Informe no PR: RAM com o módulo ligado e fps medido.
