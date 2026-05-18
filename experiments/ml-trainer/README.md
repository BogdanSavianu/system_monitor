## Run

```bash
cargo run --release --manifest-path experiments/ml-trainer/Cargo.toml -- \
  --dataset-dir ./experiments/dataset_large \
  --window 24 \
  --train-ratio 0.8 \
  --out ./experiments/dataset_large/model_report.json
```


```bash
cargo run --release --manifest-path experiments/ml-trainer/Cargo.toml -- \
  --manifest ./experiments/dataset_large/manifest.txt
```

CSV format expected by the loader:

- Required: `scenario,label,run_id,step,elapsed_s,leaked_kb_step,leaked_kb_total,workload_kb_this_step`
- Optional: `observed_memory_kb`

When `observed_memory_kb` is present, the trainer uses it directly for realistic-feature construction.
When it is missing, the trainer back-fills observed memory using the existing synthetic reconstruction path for backwards compatibility.

The trainer now uses a single 4-feature layout (same as the main app runtime).
