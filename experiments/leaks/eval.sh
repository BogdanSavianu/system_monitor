#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MODEL_PATH="${1:-}"
DATASET_DIR="${2:-$ROOT_DIR/experiments/dataset_test}"
WINDOW="${3:-24}"

if [[ -z "$MODEL_PATH" ]]; then
  echo "usage: $0 <model_path> [dataset_dir] [window]" >&2
  exit 1
fi
if [[ ! -f "$MODEL_PATH" ]]; then
  echo "model file not found: $MODEL_PATH" >&2
  exit 1
fi
if [[ ! -d "$DATASET_DIR" ]]; then
  echo "dataset dir not found: $DATASET_DIR" >&2
  exit 1
fi

MANIFEST="$DATASET_DIR/manifest.txt"
find "$DATASET_DIR" -maxdepth 1 -type f -name '*.csv' | sort > "$MANIFEST"

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

TS="$(date +%Y%m%d_%H%M%S)"
LOG_PATH="$DATASET_DIR/eval_saved_model_${TS}.log"
REPORT_PATH="$DATASET_DIR/model_eval_saved.json"

echo "evaluating saved model"
echo "model=$MODEL_PATH"
echo "dataset=$DATASET_DIR"
echo "input_csv_files=$CSV_COUNT input_rows=$TOTAL_ROWS"
echo "window=$WINDOW"
echo "log=$LOG_PATH"
echo "report=$REPORT_PATH"

(
  cd "$ROOT_DIR"
  cargo run --release --manifest-path experiments/ml-trainer/Cargo.toml -- \
    --manifest "$MANIFEST" \
    --window "$WINDOW" \
    --model-in "$MODEL_PATH" \
    --out "$REPORT_PATH"
) | tee "$LOG_PATH"

echo "saved-model evaluation log written to $LOG_PATH"
