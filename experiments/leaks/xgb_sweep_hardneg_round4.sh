#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/../.."

train_manifest="${1:-experiments/real_manifests/train_with_hardneg.txt}"
valid_manifest="${2:-experiments/real_manifests/valid.txt}"
holdout_manifest="${3:-experiments/real_manifests/holdout.txt}"
out_dir="${4:-experiments/real_retrain_xgb_round4_hardneg/sweep}"

mkdir -p "$out_dir"

configs=(
  "xgb_a 500 6 0.05 20 1.0 0.0 1.0"
  "xgb_b 800 6 0.03 20 1.0 0.0 1.0"
  "xgb_c 1000 6 0.03 15 1.0 0.0 0.9"
  "xgb_d 700 7 0.04 15 1.5 0.0 0.9"
  "xgb_e 1000 8 0.02 10 1.5 0.0 0.9"
  "xgb_f 1200 6 0.02 10 2.0 0.0 0.85"
  "xgb_g 900 7 0.025 12 1.5 0.0 0.85"
  "xgb_h 1200 8 0.015 8 2.0 0.0 0.8"
)

failed_ids=()
for cfg in "${configs[@]}"; do
  set -- $cfg
  id="$1"; n="$2"; d="$3"; lr="$4"; mcw="$5"; lam="$6"; gam="$7"; subs="$8"
  echo "running $id n=$n depth=$d lr=$lr mcw=$mcw lambda=$lam gamma=$gam subsample=$subs"

  if cargo run --release --manifest-path experiments/ml-trainer/Cargo.toml -- \
    --manifest "$train_manifest" \
    --valid-manifest "$valid_manifest" \
    --feature-set realistic \
    --algorithm xgboost \
    --window 24 \
    --xgb-n-estimators "$n" \
    --xgb-max-depth "$d" \
    --xgb-learning-rate "$lr" \
    --xgb-min-child-weight "$mcw" \
    --xgb-lambda "$lam" \
    --xgb-gamma "$gam" \
    --xgb-subsample "$subs" \
    --model-out "$out_dir/leak_model_${id}.json" \
    --out "$out_dir/retrain_${id}.json" \
    > "$out_dir/train_${id}.log" 2>&1; then
    echo "completed $id"
  else
    echo "FAILED $id (see $out_dir/train_${id}.log)"
    failed_ids+=("$id")
  fi

done

summary_tsv="$out_dir/leaderboard.tsv"
echo -e "id\taccuracy\tprecision\trecall\tf1" > "$summary_tsv"
for f in "$out_dir"/retrain_*.json; do
  [[ -f "$f" ]] || continue
  id=$(basename "$f" .json | sed 's/^retrain_//')
  jq -r --arg id "$id" '[ $id, .accuracy, .precision, .recall, .f1 ] | @tsv' "$f" >> "$summary_tsv"
done

line_count=$(wc -l < "$summary_tsv")
if [[ "$line_count" -le 1 ]]; then
  echo "no successful sweep runs"
  exit 1
fi

best_row=$(tail -n +2 "$summary_tsv" | sort -k5,5nr -k4,4nr -k3,3nr | head -n 1)
best_id=$(echo "$best_row" | cut -f1)

cp -f "$out_dir/leak_model_${best_id}.json" "$out_dir/best_model_xgboost.json"
cp -f "$out_dir/retrain_${best_id}.json" "$out_dir/best_retrain_report.json"

cargo run --release --manifest-path experiments/ml-trainer/Cargo.toml -- \
  --manifest "$holdout_manifest" \
  --window 24 \
  --model-in "$out_dir/best_model_xgboost.json" \
  --out "$out_dir/best_holdout_report.json" \
  > "$out_dir/holdout_${best_id}.log" 2>&1

cat > "$out_dir/best_model_meta.txt" <<EOF
best_id=$best_id
best_row=$best_row
failed_ids=${failed_ids[*]:-none}
train_manifest=$train_manifest
valid_manifest=$valid_manifest
holdout_manifest=$holdout_manifest
EOF

echo "=== sweep summary (sorted by f1, recall, precision) ==="
tail -n +2 "$summary_tsv" | sort -k5,5nr -k4,4nr -k3,3nr
echo "best_id=$best_id"
echo "best_model=$out_dir/best_model_xgboost.json"
echo "best_retrain_report=$out_dir/best_retrain_report.json"
echo "best_holdout_report=$out_dir/best_holdout_report.json"
echo "failed_ids=${failed_ids[*]:-none}"
