#!/usr/bin/env bash
set -euo pipefail

DATASET_DIR="${1:-}"
MANIFEST_PATH="${2:-}"

if [[ -z "$DATASET_DIR" ]]; then
  echo "usage: $0 <dataset_dir> [manifest_path]" >&2
  exit 1
fi
if [[ ! -d "$DATASET_DIR" ]]; then
  echo "dataset dir not found: $DATASET_DIR" >&2
  exit 1
fi

if [[ -z "$MANIFEST_PATH" ]]; then
  MANIFEST_PATH="$DATASET_DIR/manifest.txt"
fi

find "$DATASET_DIR" -maxdepth 1 -type f -name '*.csv' | sort > "$MANIFEST_PATH"

count="$(wc -l < "$MANIFEST_PATH" | tr -d ' ')"
if [[ "$count" -eq 0 ]]; then
  echo "no csv files found in: $DATASET_DIR" >&2
  exit 1
fi

echo "manifest generated"
echo "dataset_dir=$DATASET_DIR"
echo "manifest=$MANIFEST_PATH"
echo "csv_files=$count"
