#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LEAKS_DIR="$ROOT_DIR/experiments/leaks"

TRAIN_DIR="$ROOT_DIR/experiments/dataset_train"
VALID_DIR="$ROOT_DIR/experiments/dataset_valid"
TEST_DIR="$ROOT_DIR/experiments/dataset_test"
RUNS_PER_SCENARIO="20"
STEPS="240"
DEFAULT_JOBS="$(command -v nproc >/dev/null 2>&1 && nproc || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"
JOBS="$DEFAULT_JOBS"
SKIP_LIST=""

usage() {
  cat <<'EOF'
Usage:
  ./experiments/leaks/prepare_train_valid_test_datasets.sh [options]

Options:
  --train-dir <path>   Output directory for training dataset
  --valid-dir <path>   Output directory for validation dataset
  --test-dir <path>    Output directory for test dataset
  --runs <n>           Runs per scenario (default: 20)
  --steps <n>          Steps per run (default: 240)
  --jobs <n>           Parallel jobs per dataset generation
  --skip <list>        Comma-separated list: train,valid,test
  -h, --help           Show this help message

Examples:
  ./experiments/leaks/prepare_train_valid_test_datasets.sh
  ./experiments/leaks/prepare_train_valid_test_datasets.sh --runs 30 --steps 300 --jobs 8
  ./experiments/leaks/prepare_train_valid_test_datasets.sh --skip train
  ./experiments/leaks/prepare_train_valid_test_datasets.sh --train-dir ./experiments/dataset_large --skip train
EOF
}

contains_skip() {
  local name="$1"
  if [[ -z "$SKIP_LIST" ]]; then
    return 1
  fi
  local normalized=",${SKIP_LIST// /},"
  [[ "$normalized" == *",${name},"* ]]
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --train-dir)
      TRAIN_DIR="$2"
      shift 2
      ;;
    --valid-dir)
      VALID_DIR="$2"
      shift 2
      ;;
    --test-dir)
      TEST_DIR="$2"
      shift 2
      ;;
    --runs)
      RUNS_PER_SCENARIO="$2"
      shift 2
      ;;
    --steps)
      STEPS="$2"
      shift 2
      ;;
    --jobs)
      JOBS="$2"
      shift 2
      ;;
    --skip)
      SKIP_LIST="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ "$RUNS_PER_SCENARIO" -lt 1 ]]; then
  echo "--runs must be >= 1" >&2
  exit 1
fi
if [[ "$STEPS" -lt 1 ]]; then
  echo "--steps must be >= 1" >&2
  exit 1
fi
if [[ "$JOBS" -lt 1 ]]; then
  echo "--jobs must be >= 1" >&2
  exit 1
fi

for token in ${SKIP_LIST//,/ }; do
  if [[ -n "$token" && "$token" != "train" && "$token" != "valid" && "$token" != "test" ]]; then
    echo "invalid --skip entry: $token (allowed: train,valid,test)" >&2
    exit 1
  fi
done

echo "Preparing train/valid/test datasets"
echo "train_dir=$TRAIN_DIR"
echo "valid_dir=$VALID_DIR"
echo "test_dir=$TEST_DIR"
echo "runs=$RUNS_PER_SCENARIO steps=$STEPS jobs=$JOBS skip=${SKIP_LIST:-<none>}"

generate_one() {
  local split_name="$1"
  local out_dir="$2"

  if contains_skip "$split_name"; then
    echo "[skip] ${split_name}: $out_dir"
    return
  fi

  echo "[generate] ${split_name}: $out_dir"
  "$LEAKS_DIR/generate_dataset.sh" "$out_dir" "$RUNS_PER_SCENARIO" "$STEPS" "$JOBS"
}

generate_one train "$TRAIN_DIR"
generate_one valid "$VALID_DIR"
generate_one test "$TEST_DIR"

echo "Done."
