#!/usr/bin/env bash
set -euo pipefail

# Train anomaly model over a dataset directory and log metrics.
# Usage:
#   ./experiments/leaks/train.sh [dataset_dir] [window] [train_ratio]

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

CSV_LIST="$(paste -sd, "$MANIFEST")"
if [[ -z "$CSV_LIST" ]]; then
  echo "no csv files found in: $DATASET_DIR" >&2
  exit 1
fi

LOG_PATH="$DATASET_DIR/training_$(date +%Y%m%d_%H%M%S).log"

echo "training with window=$WINDOW train_ratio=$TRAIN_RATIO"
echo "dataset=$DATASET_DIR"
echo "log=$LOG_PATH"

(
  cd "$ROOT_DIR"
  cargo run --release -- --train-anomaly="$CSV_LIST" --window="$WINDOW" --train-ratio="$TRAIN_RATIO"
) | tee "$LOG_PATH"

echo "training log saved to $LOG_PATH"
