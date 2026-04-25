#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LEAKS_DIR="$ROOT_DIR/experiments/leaks"
OUT_DIR="${1:-$ROOT_DIR/experiments/dataset_large}"
RUNS_PER_SCENARIO="${2:-20}"
STEPS="${3:-240}"
DEFAULT_JOBS="$(command -v nproc >/dev/null 2>&1 && nproc || getconf _NPROCESSORS_ONLN 2>/dev/null || echo 4)"
JOBS="${4:-$DEFAULT_JOBS}"
SCENARIO_JOBS="${5:-1}"

if [[ "$JOBS" -lt 1 ]]; then
  echo "jobs must be >= 1" >&2
  exit 1
fi
if [[ "$SCENARIO_JOBS" -lt 1 ]]; then
  echo "scenario_jobs must be >= 1" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"

rand_range() {
  local min="$1"
  local max="$2"
  echo $(( min + RANDOM % (max - min + 1) ))
}

echo "[1/3] building generators"
make -C "$LEAKS_DIR"

echo "[2/3] generating dataset in $OUT_DIR with jobs=$JOBS scenario_jobs=$SCENARIO_JOBS"
for i in $(seq 1 "$RUNS_PER_SCENARIO"); do
  (
    run_seed="$(date +%s)-$i-$$-$RANDOM-$RANDOM"

    steady_kb="$(rand_range 24 44)"

    bursty_base_kb="$(rand_range 6 14)"
    bursty_every="$(rand_range 14 28)"
    bursty_spike_kb="$(rand_range 768 1600)"

    staircase_start_kb="$(rand_range 4 12)"
    staircase_increment_kb="$(rand_range 3 7)"

    subtle_allocs="$(rand_range 100 150)"
    subtle_kb_per_alloc="$(rand_range 48 80)"
    subtle_leak_pct="$(rand_range 2 6)"

    noisy_base_kb="$(rand_range 18 32)"
    noisy_jitter_pct="$(rand_range 40 80)"
    noisy_cpu_spike_pct="$(rand_range 25 45)"
    noisy_max_cpu_spike_ms="$(rand_range 180 420)"

    control_allocs="$(rand_range 90 150)"
    control_kb_per_alloc="$(rand_range 48 84)"
    control_burst_every="$(rand_range 10 22)"

    spiky_base_kb="$(rand_range 1536 3072)"
    spiky_every="$(rand_range 4 8)"
    spiky_ms="$(rand_range 200 420)"

    run_scenario() {
      "$@" &
      scenario_pids+=("$!")
      scenario_pids_count=$((scenario_pids_count + 1))

      if (( scenario_pids_count >= SCENARIO_JOBS )); then
        for pid in "${scenario_pids[@]}"; do
          wait "$pid" || scenario_failed=1
        done
        scenario_pids=()
        scenario_pids_count=0
      fi
    }

    scenario_failed=0
    scenario_pids=()
    scenario_pids_count=0

    # positive class runs - label=1
    run_scenario env LEAK_RUN_SEED="${run_seed}:steady" "$LEAKS_DIR/steady_leak" "$steady_kb" 1 "$STEPS" "$OUT_DIR/steady_r${i}.csv"
    run_scenario env LEAK_RUN_SEED="${run_seed}:bursty" "$LEAKS_DIR/bursty_leak" "$bursty_base_kb" 1 "$bursty_every" "$bursty_spike_kb" "$STEPS" "$OUT_DIR/bursty_r${i}.csv"
    run_scenario env LEAK_RUN_SEED="${run_seed}:staircase" "$LEAKS_DIR/staircase_leak" "$staircase_start_kb" "$staircase_increment_kb" 1 "$STEPS" "$OUT_DIR/staircase_r${i}.csv"
    run_scenario env LEAK_RUN_SEED="${run_seed}:subtle" "$LEAKS_DIR/subtle_leak" "$subtle_allocs" "$subtle_kb_per_alloc" "$subtle_leak_pct" 1 "$STEPS" "$OUT_DIR/subtle_r${i}.csv"
    run_scenario env LEAK_RUN_SEED="${run_seed}:noisy" "$LEAKS_DIR/noisy_leak" "$noisy_base_kb" "$noisy_jitter_pct" "$noisy_cpu_spike_pct" "$noisy_max_cpu_spike_ms" 1 "$STEPS" "$OUT_DIR/noisy_r${i}.csv"

    # negative class runs - label=0
    run_scenario env LEAK_RUN_SEED="${run_seed}:control" "$LEAKS_DIR/control_workload" "$control_allocs" "$control_kb_per_alloc" "$control_burst_every" 1 "$STEPS" "$OUT_DIR/control_r${i}.csv"
    run_scenario env LEAK_RUN_SEED="${run_seed}:spiky_stable" "$LEAKS_DIR/cpu_spiky_stable_mem" "$spiky_base_kb" "$spiky_every" "$spiky_ms" 1 "$STEPS" "$OUT_DIR/spiky_stable_r${i}.csv"

    if (( scenario_pids_count > 0 )); then
      for pid in "${scenario_pids[@]}"; do
        wait "$pid" || scenario_failed=1
      done
    fi

    if (( scenario_failed != 0 )); then
      echo "run batch $i failed" >&2
      exit 1
    fi

    echo "  generated run batch $i/$RUNS_PER_SCENARIO (steady_kb=$steady_kb bursty_base=$bursty_base_kb bursty_spike=$bursty_spike_kb staircase_inc=$staircase_increment_kb subtle_leak_pct=$subtle_leak_pct noisy_base=$noisy_base_kb control_allocs=$control_allocs spiky_base=$spiky_base_kb)"
  ) &

  if (( i % JOBS == 0 )); then
    wait
  fi
done

wait

echo "[3/3] writing manifest"
find "$OUT_DIR" -maxdepth 1 -type f -name '*.csv' | sort > "$OUT_DIR/manifest.txt"
CSV_COUNT="$(wc -l < "$OUT_DIR/manifest.txt" | tr -d ' ')"
echo "done: $CSV_COUNT csv files"
echo "manifest: $OUT_DIR/manifest.txt"

echo "summary: files per scenario"
for prefix in steady bursty staircase subtle noisy control spiky_stable; do
  count="$(find "$OUT_DIR" -maxdepth 1 -type f -name "${prefix}_r*.csv" | wc -l | tr -d ' ')"
  echo "  ${prefix}: ${count}"
done

echo "summary: rows"
TOTAL_ROWS=0
while IFS= read -r csv; do
  rows="$(awk 'END { if (NR > 0) print NR - 1; else print 0 }' "$csv")"
  TOTAL_ROWS=$((TOTAL_ROWS + rows))
done < "$OUT_DIR/manifest.txt"
echo "  total_data_rows=${TOTAL_ROWS}"
