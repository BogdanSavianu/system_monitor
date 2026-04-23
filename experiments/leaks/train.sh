#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DATASET_DIR="${1:-$ROOT_DIR/experiments/dataset_large}"
WINDOW="${2:-24}"
TRAIN_RATIO="${3:-0.8}"

if [[ ! -d "$DATASET_DIR" ]]; then
  echo "dataset dir not found: $DATASET_DIR" >&2
  exit 1
fi

MANIFEST="$DATASET_DIR/manifest.txt"
if [[ ! -f "$MANIFEST" ]]; then
  find "$DATASET_DIR" -maxdepth 1 -type f -name '*.csv' | sort > "$MANIFEST"
fi

CSV_COUNT="$(wc -l < "$MANIFEST" | tr -d ' ')"
if [[ "$CSV_COUNT" -eq 0 ]]; then
  echo "no csv files found in: $DATASET_DIR" >&2
  exit 1
fi

TOTAL_ROWS=0
while IFS= read -r csv; do
  rows="$(awk 'END { if (NR > 0) print NR - 1; else print 0 }' "$csv")"
  TOTAL_ROWS=$((TOTAL_ROWS + rows))
done < "$MANIFEST"

LOG_PATH="$DATASET_DIR/training_$(date +%Y%m%d_%H%M%S).log"
REPORT_PATH="$DATASET_DIR/model_report.json"

echo "training with window=$WINDOW train_ratio=$TRAIN_RATIO"
echo "dataset=$DATASET_DIR"
echo "input_csv_files=$CSV_COUNT input_rows=$TOTAL_ROWS"
echo "log=$LOG_PATH"
echo "report=$REPORT_PATH"

(
  cd "$ROOT_DIR"
  cargo run --release --manifest-path experiments/ml-trainer/Cargo.toml -- \
    --manifest "$MANIFEST" \
    --window "$WINDOW" \
    --train-ratio "$TRAIN_RATIO" \
    --out "$REPORT_PATH"
) | tee "$LOG_PATH"

echo "training log saved to $LOG_PATH"
