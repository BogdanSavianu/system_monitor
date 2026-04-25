#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TRAIN_DATASET_DIR="${1:-$ROOT_DIR/experiments/dataset_train}"
VALID_DATASET_DIR="${2:-$ROOT_DIR/experiments/dataset_valid}"
TEST_DATASET_DIR="${3:-$ROOT_DIR/experiments/dataset_test}"
WINDOW="${4:-24}"
TRAIN_RATIO="${5:-0.8}"

if [[ ! -d "$TRAIN_DATASET_DIR" ]]; then
  echo "train dataset dir not found: $TRAIN_DATASET_DIR" >&2
  exit 1
fi
if [[ ! -d "$VALID_DATASET_DIR" ]]; then
  echo "validation dataset dir not found: $VALID_DATASET_DIR" >&2
  exit 1
fi
if [[ ! -d "$TEST_DATASET_DIR" ]]; then
  echo "test dataset dir not found: $TEST_DATASET_DIR" >&2
  exit 1
fi

echo "[1/2] train on train, validate on valid (feature_set=realistic)"
"$ROOT_DIR/experiments/leaks/train.sh" \
  "$TRAIN_DATASET_DIR" \
  "$WINDOW" \
  "$TRAIN_RATIO" \
  "$VALID_DATASET_DIR" \
  realistic

MODEL_PATH="$TRAIN_DATASET_DIR/model_external.json"
if [[ ! -f "$MODEL_PATH" ]]; then
  echo "expected model not found after training: $MODEL_PATH" >&2
  exit 1
fi

echo "[2/2] evaluate saved model on test"
"$ROOT_DIR/experiments/leaks/eval.sh" "$MODEL_PATH" "$TEST_DATASET_DIR" "$WINDOW"

echo "realistic benchmark complete"
echo "train/valid report: $TRAIN_DATASET_DIR/model_report_external.json"
echo "test report: $TEST_DATASET_DIR/model_eval_saved.json"
