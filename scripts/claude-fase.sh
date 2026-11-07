#!/usr/bin/env sh
# Roda uma fase do CamFácil IA no Claude Code, em modo não interativo.
# Uso: scripts/claude-fase.sh 1        (usa prompts/fase-1-*.md)
#      scripts/claude-fase.sh 1 -i     (abre em modo interativo)
set -eu

FASE="${1:-1}"
PROMPT_FILE="$(ls prompts/fase-"$FASE"-*.md 2>/dev/null | head -n 1 || true)"

if [ -z "$PROMPT_FILE" ]; then
  echo "Nenhum prompt encontrado para a fase $FASE em prompts/." >&2
  exit 1
fi

if ! command -v claude >/dev/null 2>&1; then
  echo "Claude Code não encontrado. Instale: https://docs.claude.com/en/docs/claude-code" >&2
  exit 1
fi

echo "▶ Fase $FASE com $PROMPT_FILE"
if [ "${2:-}" = "-i" ]; then
  claude "$(cat "$PROMPT_FILE")"
else
  claude -p "$(cat "$PROMPT_FILE")" --permission-mode acceptEdits
fi
