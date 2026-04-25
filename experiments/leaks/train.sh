#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DATASET_DIR="${1:-$ROOT_DIR/experiments/dataset_large}"
WINDOW="${2:-24}"
TRAIN_RATIO="${3:-0.8}"
VALID_DATASET_DIR="${4:-}"

if [[ ! -d "$DATASET_DIR" ]]; then
  echo "dataset dir not found: $DATASET_DIR" >&2
  exit 1
fi

MANIFEST="$DATASET_DIR/manifest.txt"
if [[ ! -f "$MANIFEST" ]]; then
  find "$DATASET_DIR" -maxdepth 1 -type f -name '*.csv' | sort > "$MANIFEST"
fi

VALID_MANIFEST=""
if [[ -n "$VALID_DATASET_DIR" ]]; then
  if [[ ! -d "$VALID_DATASET_DIR" ]]; then
    echo "validation dataset dir not found: $VALID_DATASET_DIR" >&2
    exit 1
  fi

  VALID_MANIFEST="$VALID_DATASET_DIR/manifest.txt"
  if [[ ! -f "$VALID_MANIFEST" ]]; then
    find "$VALID_DATASET_DIR" -maxdepth 1 -type f -name '*.csv' | sort > "$VALID_MANIFEST"
  fi
fi

CSV_COUNT="$(wc -l < "$MANIFEST" | tr -d ' ')"
if [[ "$CSV_COUNT" -eq 0 ]]; then
  echo "no csv files found in: $DATASET_DIR" >&2
  exit 1
fi

if [[ -n "$VALID_MANIFEST" ]]; then
  VALID_CSV_COUNT="$(wc -l < "$VALID_MANIFEST" | tr -d ' ')"
  if [[ "$VALID_CSV_COUNT" -eq 0 ]]; then
    echo "no csv files found in validation dataset: $VALID_DATASET_DIR" >&2
    exit 1
  fi
fi

TOTAL_ROWS=0
while IFS= read -r csv; do
  rows="$(awk 'END { if (NR > 0) print NR - 1; else print 0 }' "$csv")"
  TOTAL_ROWS=$((TOTAL_ROWS + rows))
done < "$MANIFEST"

VALID_TOTAL_ROWS=0
if [[ -n "$VALID_MANIFEST" ]]; then
  while IFS= read -r csv; do
    rows="$(awk 'END { if (NR > 0) print NR - 1; else print 0 }' "$csv")"
    VALID_TOTAL_ROWS=$((VALID_TOTAL_ROWS + rows))
  done < "$VALID_MANIFEST"
fi

TS="$(date +%Y%m%d_%H%M%S)"
if [[ -n "$VALID_MANIFEST" ]]; then
  LOG_PATH="$DATASET_DIR/training_external_${TS}.log"
  REPORT_PATH="$DATASET_DIR/model_report_external.json"
  MODEL_PATH="$DATASET_DIR/model_external.json"
else
  LOG_PATH="$DATASET_DIR/training_split_${TS}.log"
  REPORT_PATH="$DATASET_DIR/model_report_split.json"
  MODEL_PATH="$DATASET_DIR/model_split.json"
fi

if [[ -n "$VALID_MANIFEST" ]]; then
  echo "training with window=$WINDOW mode=external_validation_dataset"
else
  echo "training with window=$WINDOW train_ratio=$TRAIN_RATIO mode=in_dataset_run_split"
fi
echo "dataset=$DATASET_DIR"
echo "input_csv_files=$CSV_COUNT input_rows=$TOTAL_ROWS"
if [[ -n "$VALID_MANIFEST" ]]; then
  echo "validation_dataset=$VALID_DATASET_DIR"
  echo "validation_input_csv_files=$VALID_CSV_COUNT validation_input_rows=$VALID_TOTAL_ROWS"
fi
echo "log=$LOG_PATH"
echo "report=$REPORT_PATH"
echo "model=$MODEL_PATH"

(
  cd "$ROOT_DIR"
  if [[ -n "$VALID_MANIFEST" ]]; then
    cargo run --release --manifest-path experiments/ml-trainer/Cargo.toml -- \
      --manifest "$MANIFEST" \
      --valid-manifest "$VALID_MANIFEST" \
      --window "$WINDOW" \
      --model-out "$MODEL_PATH" \
      --out "$REPORT_PATH"
  else
    cargo run --release --manifest-path experiments/ml-trainer/Cargo.toml -- \
      --manifest "$MANIFEST" \
      --window "$WINDOW" \
      --train-ratio "$TRAIN_RATIO" \
      --model-out "$MODEL_PATH" \
      --out "$REPORT_PATH"
  fi
) | tee "$LOG_PATH"

echo "training log saved to $LOG_PATH"
