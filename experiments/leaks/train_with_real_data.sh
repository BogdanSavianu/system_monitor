#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TRAIN_MANIFEST="${1:-}"
VALID_MANIFEST="${2:-}"
HOLDOUT_MANIFEST="${3:-}"
OUT_DIR="${4:-$ROOT_DIR/experiments/real_retrain}"
WINDOW="${5:-24}"

if [[ -z "$TRAIN_MANIFEST" || -z "$VALID_MANIFEST" || -z "$HOLDOUT_MANIFEST" ]]; then
  echo "usage: $0 <train_manifest> <valid_manifest> <holdout_manifest> [out_dir] [window]" >&2
  exit 1
fi

for path in "$TRAIN_MANIFEST" "$VALID_MANIFEST" "$HOLDOUT_MANIFEST"; do
  if [[ ! -f "$path" ]]; then
    echo "manifest not found: $path" >&2
    exit 1
  fi
done

mkdir -p "$OUT_DIR"

TS="$(date +%Y%m%d_%H%M%S)"
TRAIN_LOG="$OUT_DIR/retrain_${TS}.log"
TRAIN_REPORT="$OUT_DIR/retrain_report.json"
MODEL_PATH="$OUT_DIR/leak_model_realistic.json"
HOLDOUT_LOG="$OUT_DIR/holdout_eval_${TS}.log"
HOLDOUT_REPORT="$OUT_DIR/holdout_report.json"

count_manifest_rows() {
  local manifest="$1"
  local total=0
  while IFS= read -r csv; do
    [[ -z "$csv" ]] && continue
    if [[ -f "$csv" ]]; then
      rows="$(awk 'END { if (NR > 0) print NR - 1; else print 0 }' "$csv")"
      total=$((total + rows))
    fi
  done < "$manifest"
  echo "$total"
}

train_files="$(wc -l < "$TRAIN_MANIFEST" | tr -d ' ')"
valid_files="$(wc -l < "$VALID_MANIFEST" | tr -d ' ')"
holdout_files="$(wc -l < "$HOLDOUT_MANIFEST" | tr -d ' ')"
train_rows="$(count_manifest_rows "$TRAIN_MANIFEST")"
valid_rows="$(count_manifest_rows "$VALID_MANIFEST")"
holdout_rows="$(count_manifest_rows "$HOLDOUT_MANIFEST")"

echo "retraining realistic model with external validation + holdout"
echo "train_manifest=$TRAIN_MANIFEST files=$train_files rows=$train_rows"
echo "valid_manifest=$VALID_MANIFEST files=$valid_files rows=$valid_rows"
echo "holdout_manifest=$HOLDOUT_MANIFEST files=$holdout_files rows=$holdout_rows"
echo "window=$WINDOW"
echo "out_dir=$OUT_DIR"
echo "train_log=$TRAIN_LOG"
echo "train_report=$TRAIN_REPORT"
echo "model=$MODEL_PATH"
echo "holdout_log=$HOLDOUT_LOG"
echo "holdout_report=$HOLDOUT_REPORT"

(
  cd "$ROOT_DIR"
  cargo run --release --manifest-path experiments/ml-trainer/Cargo.toml -- \
    --manifest "$TRAIN_MANIFEST" \
    --valid-manifest "$VALID_MANIFEST" \
    --feature-set realistic \
    --window "$WINDOW" \
    --model-out "$MODEL_PATH" \
    --run-sanity-checks \
    --out "$TRAIN_REPORT"
) | tee "$TRAIN_LOG"

(
  cd "$ROOT_DIR"
  cargo run --release --manifest-path experiments/ml-trainer/Cargo.toml -- \
    --manifest "$HOLDOUT_MANIFEST" \
    --window "$WINDOW" \
    --model-in "$MODEL_PATH" \
    --out "$HOLDOUT_REPORT"
) | tee "$HOLDOUT_LOG"

echo "retraining complete"
echo "model=$MODEL_PATH"
echo "train_report=$TRAIN_REPORT"
echo "holdout_report=$HOLDOUT_REPORT"
