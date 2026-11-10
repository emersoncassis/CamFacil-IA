---
name: novo-modulo-ia
description: Cria um novo módulo de IA ou um novo tipo de inteligência por regra no CamFácil IA (registro, núcleo, tela, testes e aula). Use quando pedirem uma capacidade nova de visão computacional.
---

# Novo módulo ou inteligência

O contrato por pasta (`module.toml` + trait `Module`, SDD-02) **não foi implementado**; é proposta para a comunidade. Hoje a IA é **um detector só** (YOLO11n ONNX, carregado uma vez e compartilhado) e tudo o que parece "módulo" lê o que ele viu. Antes de criar algo, decida qual caso é o seu:

**A. Nova inteligência por regra** (ex.: outro tipo de alerta sobre pessoas e veículos, SDD-09)
1. Leia `docs/sdd/SDD-09-inteligencias-por-regra.md` e `crates/cf-core/src/inteligencias.rs` (catálogo com forma, peso, módulo exigido, parâmetro e classes).
2. Acrescente a inteligência ao catálogo e a lógica em `crates/cf-core/src/deteccao.rs` (`Rastreador::processar`, `regra_casa`, eventos). Só detecta e emite evento; nunca desenha caixas, nunca envia notificações.
3. Valide forma, classes, cor e som na API (`/api/cameras/{id}/regras`, `/api/regras/{id}`; forma errada ou valor inválido responde 400). Com o módulo exigido desligado a regra **nasce desligada**, com aviso, e liga sozinha quando o módulo for ligado (`espera_modulo`). Na tela, formulário de 3 abas com textos em `web/i18n/pt-BR.json`.
4. Considere o MCP `criar_regra`, o "IA, sugira para mim" (`sugestao.rs`) e o "IA, avalie a minha regra" (`avaliacao.rs`).

**B. Novo módulo (interruptor)**
1. Registre em `MODULOS` (`crates/camfacil/src/video.rs`: id, nome, apelidos, nível leve/médio/pesado) e trate o id onde os módulos são lidos (`video.rs`, `api.rs`). Nada vem ligado (regra 10).
2. Módulo que precisa de modelo: veja o fluxo de download sob demanda com SHA-256 em `crates/camfacil/src/ia/` e `modelos/README.md`; a RAM do modelo é liberada após 1 min ocioso.
3. Comunidade/estudo: um modelo próprio entra como **receita de estúdio** (SDD-07, ADR 0013), não como módulo compilado.

Em qualquer caso:
- Testes sem rede em `cf-core` (quadros sintéticos ou trilhas simuladas, como os de `deteccao.rs`); evite fixtures pesadas no repositório.
- Acione o agente `autor-de-aulas` se a capacidade merece lição na Formação (site).
- Acione o agente `guardiao-da-leveza` antes do PR e informe RAM com o módulo ligado e fps medido (`scripts/budget.sh`).
- Atualize SDD-02 ou SDD-09 e o README (Novidades).
