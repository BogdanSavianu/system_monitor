#!/usr/bin/env bash
# Compares the reachability scanner against Valgrind on a leak fixture, with the
# fixture's own known leak as ground truth
#
# Usage: ./compare_valgrind.sh [fixture] [kb_per_step] [interval_s] [steps]
set -euo pipefail

FIXTURE="${1:-calloc_leak}"
KB="${2:-128}"
INTERVAL="${3:-1}"
STEPS="${4:-8}"

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"

GROUND_KB=$((KB * STEPS))
GROUND_BYTES=$((GROUND_KB * 1024))

echo "fixture=$FIXTURE  kb_per_step=$KB  interval_s=$INTERVAL  steps=$STEPS"
echo "ground truth leaked: ${GROUND_KB} KB (${GROUND_BYTES} bytes)"
echo

make -C "$HERE" "$FIXTURE" >/dev/null

echo "===== Valgrind ====="
VG_OUT="$(valgrind --leak-check=full --error-exitcode=0 \
    "$HERE/$FIXTURE" "$KB" "$INTERVAL" "$STEPS" 2>&1)"
VG_DEF="$(echo "$VG_OUT" | grep -oP 'definitely lost:\s*\K[0-9,]+' | tr -d ',' | head -1 || true)"
echo "definitely lost: ${VG_DEF:-0} bytes"
echo

echo "===== Reachability scanner ====="
# our implementation runs against the live process so after STEPS steps
# only STEPS - 1 steps were actually leaked, that is why we do an extra step
OUR_STEPS=$(( STEPS + 1 ))
SCAN_AFTER="$(awk "BEGIN { print $STEPS * $INTERVAL + $INTERVAL / 2 }")"
EXAMPLE_BIN="$REPO/target/debug/examples/reachability_scan"
if [ ! -x "$EXAMPLE_BIN" ]; then
    cargo build --example reachability_scan --features leakprobe \
        --manifest-path "$REPO/Cargo.toml" >/dev/null 2>&1 || true
fi
OUR_OUT="$(SCAN_AFTER_S="$SCAN_AFTER" "$EXAMPLE_BIN" \
    "$HERE/$FIXTURE" "$KB" "$INTERVAL" "$OUR_STEPS" 2>&1)" || true
echo "$OUR_OUT"
OUR_DEF="$(echo "$OUR_OUT" | grep -oP 'definitely_lost:\s*\K[0-9]+' | head -1 || true)"
OUR_POSS="$(echo "$OUR_OUT" | grep -oP 'possibly_lost:\s*\K[0-9]+' | head -1 || true)"
OUR_LOST="n/a"
if [ -n "${OUR_DEF:-}" ]; then
    OUR_LOST=$(( OUR_DEF + ${OUR_POSS:-0} ))
fi
echo

echo "===== Three-way comparison (bytes) ====="
printf '%-26s %15s\n' "ground truth"              "$GROUND_BYTES"
printf '%-26s %15s\n' "valgrind def. lost"        "${VG_DEF:-n/a}"
printf '%-26s %15s\n' "ours def. lost"            "${OUR_DEF:-n/a}"
printf '%-26s %15s\n' "ours possibly lost"        "${OUR_POSS:-n/a}"
printf '%-26s %15s\n' "ours lost (def+poss)"      "$OUR_LOST"
