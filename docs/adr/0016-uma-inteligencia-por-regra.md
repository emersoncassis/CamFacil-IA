# ADR 0016 · Uma inteligência por regra, com classes, cor e som

**Situação:** aceita · 2026-11-08 · complementa o [SDD-02](../sdd/SDD-02-modulos-ia.md) e define o [SDD-09](../sdd/SDD-09-inteligencias-por-regra.md)

## Contexto

Os módulos médios eram globais: ligou, vale para toda regra, e a linha contava tudo. O técnico precisa de regras específicas ("só carro", "só moto", "passou de 3 carros"), sem pagar mais CPU por isso.

## Decisão

1. Cada regra tem **uma** inteligência (`passagem`, `presenca`, `densidade`, `permanencia`, `calor`) e uma **lista de classes**. O detector continua único; a regra só filtra o que ele já viu.
2. Guardar tudo num JSON `regras.ajustes` em vez de várias colunas: o backup e o MCP levam o campo inteiro, e o catálogo cresce sem migração nova.
3. Regra sem `ajustes` é `legado` e mantém o comportamento antigo (nada quebra, nem backup antigo).
4. A **disponibilidade** vem do módulo ligado e é checada no servidor (409) e na tela (opção apagada com o motivo). Combinação inválida é 400.
5. Cor e som são ajustes da regra: cores viram tokens (`--linha-*`), sons são gerados pelo Web Audio (zero arquivo, zero dependência).
6. Inteligências **não se somam** numa regra. Para duas coisas na mesma área, duas regras.

## Consequências

- Nenhuma dependência nova; binário e RAM no mesmo patamar (um `Vec<usize>` por regra e um filtro por trilha).
- Trocar a inteligência de uma regra existente exige recriar (a edição muda classes, tempo, cor e som).
- Com modelo de estudo ligado e nenhuma classe marcada, a regra vale para os objetos dele (como antes).
