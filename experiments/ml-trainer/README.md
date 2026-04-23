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
