#!/usr/bin/env sh
# Mede o orçamento do núcleo (SDD-01): tamanho do binário, RAM e CPU ociosas.
# Uso: scripts/budget.sh   (Linux)
set -eu

LIMIT_BIN_MB=30
LIMIT_RAM_MB=50
LIMIT_CPU=2

cargo build -q --release -p camfacil
BIN=target/release/camfacil
BIN_MB=$(awk -v b="$(wc -c < "$BIN")" 'BEGIN{printf "%.1f", b/1048576}')

DATA=$(mktemp -d)
CAMFACIL_DATA="$DATA" "$BIN" --no-browser > "$DATA/log" 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null; rm -rf "$DATA"' EXIT
sleep 2

HZ=$(getconf CLK_TCK)
ticks() { awk '{print $14 + $15}' "/proc/$PID/stat"; }
T0=$(ticks); sleep 10; T1=$(ticks)
CPU=$(awk -v d=$((T1 - T0)) -v hz="$HZ" 'BEGIN{printf "%.2f", d / hz / 10 * 100}')
RAM_MB=$(awk '/VmRSS/{printf "%.1f", $2/1024}' "/proc/$PID/status")

ok() { awk -v v="$1" -v l="$2" 'BEGIN{exit !(v < l)}' && echo "ok" || echo "ACIMA"; }
printf "%-22s %8s  limite %s  %s\n" "Binário (MB)" "$BIN_MB" "$LIMIT_BIN_MB" "$(ok "$BIN_MB" $LIMIT_BIN_MB)"
printf "%-22s %8s  limite %s  %s\n" "RAM ociosa (MB)" "$RAM_MB" "$LIMIT_RAM_MB" "$(ok "$RAM_MB" $LIMIT_RAM_MB)"
printf "%-22s %8s  limite %s  %s\n" "CPU ociosa (%)" "$CPU" "$LIMIT_CPU" "$(ok "$CPU" $LIMIT_CPU)"

[ "$(ok "$BIN_MB" $LIMIT_BIN_MB)" = ok ] && [ "$(ok "$RAM_MB" $LIMIT_RAM_MB)" = ok ] && [ "$(ok "$CPU" $LIMIT_CPU)" = ok ]
