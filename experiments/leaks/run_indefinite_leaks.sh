#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LEAKS_DIR="$ROOT_DIR/experiments/leaks"
STATE_DIR="$ROOT_DIR/experiments/leaks/data/long_runs"
LOG_DIR="$STATE_DIR/logs"
PID_FILE="$STATE_DIR/leak_pids.csv"

ACTION="start"
PROFILE="balanced"
INTERVAL_S="1"
BUILD_BINARIES="1"
DRY_RUN="0"
FORCE="0"
VARIANT_MODE="fixed"
RUN_SEED=""
EXTRA_PROGRAMS="0"

usage() {
  cat <<EOF
usage: run_indefinite_leaks.sh [start|stop|status] [options]

options:
  --profile <light|balanced|aggressive>   Scenario set to run (default: balanced)
  --variant <fixed|jitter>                Parameter style (default: fixed)
  --seed <int>                            Seed for jittered params (default: current epoch)
  --interval <seconds>                    Common interval arg for scenarios (default: 1)
  --extra-programs                        Also run arena/realloc/mmap extra generators
  --pid-file <path>                       Override pid tracking file
  --no-build                              Skip make before starting
  --dry-run                               Print commands without starting processes
  --force                                 Overwrite existing pid file on start
  -h, --help                              Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    start|stop|status)
      ACTION="$1"
      shift
      ;;
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --interval)
      INTERVAL_S="${2:-}"
      shift 2
      ;;
    --variant)
      VARIANT_MODE="${2:-}"
      shift 2
      ;;
    --seed)
      RUN_SEED="${2:-}"
      shift 2
      ;;
    --pid-file)
      PID_FILE="${2:-}"
      shift 2
      ;;
    --extra-programs)
      EXTRA_PROGRAMS="1"
      shift
      ;;
    --no-build)
      BUILD_BINARIES="0"
      shift
      ;;
    --dry-run)
      DRY_RUN="1"
      shift
      ;;
    --force)
      FORCE="1"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if ! [[ "$INTERVAL_S" =~ ^[0-9]+$ ]] || [[ "$INTERVAL_S" -lt 1 ]]; then
  echo "--interval must be a positive integer" >&2
  exit 1
fi

if [[ "$VARIANT_MODE" != "fixed" && "$VARIANT_MODE" != "jitter" ]]; then
  echo "invalid --variant '$VARIANT_MODE' (expected fixed|jitter)" >&2
  exit 1
fi

if [[ -n "$RUN_SEED" ]] && ! [[ "$RUN_SEED" =~ ^-?[0-9]+$ ]]; then
  echo "--seed must be an integer" >&2
  exit 1
fi

rand_int() {
  local min="$1"
  local max="$2"
  if [[ "$max" -lt "$min" ]]; then
    echo "$min"
    return
  fi
  echo $((min + RANDOM % (max - min + 1)))
}

init_rng() {
  if [[ -n "$RUN_SEED" ]]; then
    RANDOM="$RUN_SEED"
  else
    RUN_SEED="$(date +%s)"
    RANDOM="$RUN_SEED"
  fi
}

scenario_specs_fixed() {
  case "$PROFILE" in
    light)
      cat <<EOF
steady,$LEAKS_DIR/steady_leak 24 $INTERVAL_S 0
noisy,$LEAKS_DIR/noisy_leak 24 45 35 220 $INTERVAL_S 0
EOF
      ;;
    balanced)
      cat <<EOF
steady,$LEAKS_DIR/steady_leak 24 $INTERVAL_S 0
noisy,$LEAKS_DIR/noisy_leak 24 45 35 220 $INTERVAL_S 0
  subtle,$LEAKS_DIR/subtle_leak 72 10 3 $INTERVAL_S 0
EOF
      ;;
    aggressive)
      cat <<EOF
steady,$LEAKS_DIR/steady_leak 28 $INTERVAL_S 0
noisy,$LEAKS_DIR/noisy_leak 32 55 40 280 $INTERVAL_S 0
  subtle,$LEAKS_DIR/subtle_leak 96 10 3 $INTERVAL_S 0
bursty,$LEAKS_DIR/bursty_leak 12 $INTERVAL_S 18 1536 0
staircase,$LEAKS_DIR/staircase_leak 6 4 $INTERVAL_S 0
EOF
      ;;
    *)
      echo "invalid --profile '$PROFILE' (expected light|balanced|aggressive)" >&2
      exit 1
      ;;
  esac
}

scenario_specs_jitter() {
  init_rng

  case "$PROFILE" in
    light)
      local steady_kb noisy_kb noisy_jitter noisy_spike
      steady_kb="$(rand_int 18 36)"
      noisy_kb="$(rand_int 18 34)"
      noisy_jitter="$(rand_int 28 60)"
      noisy_spike="$(rand_int 160 320)"
      cat <<EOF
steady,$LEAKS_DIR/steady_leak $steady_kb $INTERVAL_S 0
noisy,$LEAKS_DIR/noisy_leak $noisy_kb $noisy_jitter 35 $noisy_spike $INTERVAL_S 0
EOF
      ;;
    balanced)
      local steady_kb noisy_kb noisy_jitter noisy_spike subtle_allocs subtle_kb subtle_pct
      steady_kb="$(rand_int 18 38)"
      noisy_kb="$(rand_int 20 36)"
      noisy_jitter="$(rand_int 30 65)"
      noisy_spike="$(rand_int 170 340)"
      subtle_allocs="$(rand_int 50 100)"
      subtle_kb="$(rand_int 10 14)"
      subtle_pct="3"
      cat <<EOF
steady,$LEAKS_DIR/steady_leak $steady_kb $INTERVAL_S 0
noisy,$LEAKS_DIR/noisy_leak $noisy_kb $noisy_jitter 35 $noisy_spike $INTERVAL_S 0
subtle,$LEAKS_DIR/subtle_leak $subtle_allocs $subtle_kb $subtle_pct $INTERVAL_S 0
EOF
      ;;
    aggressive)
      local steady_kb noisy_kb noisy_jitter noisy_spike subtle_allocs subtle_kb subtle_pct
      local bursty_base bursty_every bursty_burst stair_start stair_increment
      steady_kb="$(rand_int 20 44)"
      noisy_kb="$(rand_int 24 42)"
      noisy_jitter="$(rand_int 35 75)"
      noisy_spike="$(rand_int 200 420)"
      subtle_allocs="$(rand_int 60 130)"
      subtle_kb="$(rand_int 10 16)"
      subtle_pct="3"
      bursty_base="$(rand_int 8 20)"
      bursty_every="$(rand_int 10 28)"
      bursty_burst="$(rand_int 1024 3584)"
      stair_start="$(rand_int 4 12)"
      stair_increment="$(rand_int 2 8)"
      cat <<EOF
steady,$LEAKS_DIR/steady_leak $steady_kb $INTERVAL_S 0
noisy,$LEAKS_DIR/noisy_leak $noisy_kb $noisy_jitter 40 $noisy_spike $INTERVAL_S 0
subtle,$LEAKS_DIR/subtle_leak $subtle_allocs $subtle_kb $subtle_pct $INTERVAL_S 0
bursty,$LEAKS_DIR/bursty_leak $bursty_base $INTERVAL_S $bursty_every $bursty_burst 0
staircase,$LEAKS_DIR/staircase_leak $stair_start $stair_increment $INTERVAL_S 0
EOF
      ;;
    *)
      echo "invalid --profile '$PROFILE' (expected light|balanced|aggressive)" >&2
      exit 1
      ;;
  esac
}

scenario_specs() {
  if [[ "$VARIANT_MODE" == "fixed" ]]; then
    scenario_specs_fixed
    if [[ "$EXTRA_PROGRAMS" == "1" ]]; then
      cat <<EOF
arena_churn,$LEAKS_DIR/arena_churn_leak 80 4 64 12 $INTERVAL_S 0
realloc,$LEAKS_DIR/realloc_leak 512 24 15 $INTERVAL_S 0
mmap_sparse,$LEAKS_DIR/mmap_sparse_leak 6 256 20 $INTERVAL_S 0
EOF
    fi
  else
    scenario_specs_jitter
    if [[ "$EXTRA_PROGRAMS" == "1" ]]; then
      local arena_allocs arena_min_kb arena_max_kb arena_leak_pct
      local realloc_base realloc_growth realloc_jump mmap_regions mmap_kb mmap_release
      arena_allocs="$(rand_int 64 128)"
      arena_min_kb="$(rand_int 2 8)"
      arena_max_kb="$(rand_int 48 128)"
      arena_leak_pct="$(rand_int 8 20)"
      realloc_base="$(rand_int 384 768)"
      realloc_growth="$(rand_int 12 40)"
      realloc_jump="$(rand_int 10 24)"
      mmap_regions="$(rand_int 4 10)"
      mmap_kb="$(rand_int 128 384)"
      mmap_release="$(rand_int 12 28)"
      cat <<EOF
arena_churn,$LEAKS_DIR/arena_churn_leak $arena_allocs $arena_min_kb $arena_max_kb $arena_leak_pct $INTERVAL_S 0
realloc,$LEAKS_DIR/realloc_leak $realloc_base $realloc_growth $realloc_jump $INTERVAL_S 0
mmap_sparse,$LEAKS_DIR/mmap_sparse_leak $mmap_regions $mmap_kb $mmap_release $INTERVAL_S 0
EOF
    fi
  fi
}

is_running() {
  local pid="$1"
  kill -0 "$pid" 2>/dev/null
}

start_action() {
  mkdir -p "$(dirname "$PID_FILE")" "$LOG_DIR"

  if [[ "$DRY_RUN" == "1" ]]; then
    if [[ "$BUILD_BINARIES" == "1" ]]; then
      echo "DRY-RUN: make -C $LEAKS_DIR"
    fi
    while IFS=, read -r scenario command; do
      [[ -z "$scenario" ]] && continue
      local log_file
      log_file="$LOG_DIR/${scenario}_DRYRUN.log"
      echo "DRY-RUN: $command > $log_file 2>&1 &"
    done < <(scenario_specs)
    echo "dry run complete"
    return
  fi

  if [[ -f "$PID_FILE" && "$FORCE" != "1" ]]; then
    echo "pid file already exists: $PID_FILE" >&2
    echo "Use --force to overwrite, or run stop/status first." >&2
    exit 1
  fi

  if [[ "$BUILD_BINARIES" == "1" ]]; then
    make -C "$LEAKS_DIR" >/dev/null
  fi

  local ts
  ts="$(date +%Y%m%d_%H%M%S)"
  : > "$PID_FILE"

  while IFS=, read -r scenario command; do
    [[ -z "$scenario" ]] && continue
    local log_file
    log_file="$LOG_DIR/${scenario}_${ts}.log"

    nohup bash -lc "$command" > "$log_file" 2>&1 &
    local pid="$!"

    echo "$scenario,$pid,$log_file,$command" >> "$PID_FILE"
    sleep 0.2

    if ! is_running "$pid"; then
      echo "failed to start scenario '$scenario'" >&2
      exit 1
    fi
  done < <(scenario_specs)

  local pid_list
  pid_list="$(awk -F, '{print $2}' "$PID_FILE" | paste -sd, -)"

  echo "indefinite leak processes started"
  echo "profile=$PROFILE"
  echo "variant=$VARIANT_MODE"
  echo "extra_programs=$EXTRA_PROGRAMS"
  echo "seed=$RUN_SEED"
  echo "pid_file=$PID_FILE"
  echo "leak_pids=$pid_list"
  echo "Use these PIDs with: --label-mode mixed --leak-pids $pid_list"
}

stop_action() {
  if [[ ! -f "$PID_FILE" ]]; then
    echo "pid file not found: $PID_FILE"
    return
  fi

  local any_running="0"
  while IFS=, read -r scenario pid _rest; do
    [[ -z "$pid" ]] && continue
    if is_running "$pid"; then
      any_running="1"
      kill "$pid" || true
      echo "stopped $scenario pid=$pid"
    else
      echo "already stopped $scenario pid=$pid"
    fi
  done < "$PID_FILE"

  if [[ "$any_running" == "1" ]]; then
    sleep 0.4
  fi

  rm -f "$PID_FILE"
  echo "stop complete"
}

status_action() {
  if [[ ! -f "$PID_FILE" ]]; then
    echo "pid file not found: $PID_FILE"
    return
  fi

  if [[ ! -s "$PID_FILE" ]]; then
    echo "pid file is empty: $PID_FILE"
    return
  fi

  local shown="0"

  while IFS=, read -r scenario pid log_file command; do
    [[ -z "$pid" ]] && continue
    shown="1"
    if is_running "$pid"; then
      echo "RUNNING scenario=$scenario pid=$pid log=$log_file"
    else
      echo "STOPPED scenario=$scenario pid=$pid log=$log_file"
    fi
    echo "  command=$command"
  done < "$PID_FILE"

  if [[ "$shown" == "0" ]]; then
    echo "no entries in pid file: $PID_FILE"
  fi
}

case "$ACTION" in
  start)
    start_action
    ;;
  stop)
    stop_action
    ;;
  status)
    status_action
    ;;
  *)
    echo "unsupported action: $ACTION" >&2
    exit 1
    ;;
esac
