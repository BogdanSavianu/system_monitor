#!/usr/bin/env bash
set -euo pipefail

# Build many labeled runs for anomaly training.
# Usage:
#   ./experiments/leaks/generate_dataset.sh [out_dir] [runs] [steps]

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LEAKS_DIR="$ROOT_DIR/experiments/leaks"
OUT_DIR="${1:-$ROOT_DIR/experiments/dataset_large}"
RUNS_PER_SCENARIO="${2:-20}"
STEPS="${3:-240}"

mkdir -p "$OUT_DIR"

echo "[1/3] building generators"
make -C "$LEAKS_DIR"

echo "[2/3] generating dataset in $OUT_DIR"
for i in $(seq 1 "$RUNS_PER_SCENARIO"); do
  # Positive class runs (label=1)
  "$LEAKS_DIR/steady_leak" 32 1 "$STEPS" "$OUT_DIR/steady_r${i}.csv"
  "$LEAKS_DIR/bursty_leak" 8 1 20 1024 "$STEPS" "$OUT_DIR/bursty_r${i}.csv"
  "$LEAKS_DIR/staircase_leak" 8 4 1 "$STEPS" "$OUT_DIR/staircase_r${i}.csv"
  "$LEAKS_DIR/subtle_leak" 120 64 3 1 "$STEPS" "$OUT_DIR/subtle_r${i}.csv"
  "$LEAKS_DIR/noisy_leak" 24 60 35 300 1 "$STEPS" "$OUT_DIR/noisy_r${i}.csv"

  # Negative class runs (label=0)
  "$LEAKS_DIR/control_workload" 120 64 15 1 "$STEPS" "$OUT_DIR/control_r${i}.csv"
  "$LEAKS_DIR/cpu_spiky_stable_mem" 2048 5 300 1 "$STEPS" "$OUT_DIR/spiky_stable_r${i}.csv"

  echo "  generated run batch $i/$RUNS_PER_SCENARIO"
done

echo "[3/3] writing manifest"
find "$OUT_DIR" -maxdepth 1 -type f -name '*.csv' | sort > "$OUT_DIR/manifest.txt"
CSV_COUNT="$(wc -l < "$OUT_DIR/manifest.txt" | tr -d ' ')"
echo "done: $CSV_COUNT csv files"
echo "manifest: $OUT_DIR/manifest.txt"
