use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::Path,
    time::{Duration, SystemTime},
};

use tracing::warn;

use crate::{dto::ProcessCpuSampleDTO, util::Pid};

use super::{FeatureRow, RuntimeLeakModel};

const DEFAULT_WINDOW_SIZE: usize = 24;
const DEFAULT_STALE_AFTER_CYCLES: u64 = 120;
const MIN_MEMORY_SLOPE_KB_PER_S: f64 = 3.0;
const MIN_MEMORY_DELTA_MEAN_KB: f64 = 12.0;
const MIN_MEMORY_DELTA_MAX_KB: f64 = 64.0;
const MIN_POSITIVE_STREAK: u8 = 2;
const MIN_NEGATIVE_STREAK: u8 = 2;

#[derive(Debug, Clone)]
pub struct AnomalyTransition {
    pub pid: Pid,
    pub process_name: String,
    pub is_anomalous: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DetectorResult {
    pub anomaly_by_pid: HashMap<Pid, bool>,
    pub transitions: Vec<AnomalyTransition>,
}

#[derive(Debug, Clone)]
struct RuntimeSamplePoint {
    elapsed_s: f64,
    observed_memory_kb: f64,
    workload_value: f64,
}

#[derive(Debug, Clone)]
struct ProcessWindowState {
    first_seen: SystemTime,
    last_seen_cycle: u64,
    points: VecDeque<RuntimeSamplePoint>,
}

#[derive(Debug, Clone, Default)]
struct ProcessAnomalyState {
    is_anomalous: bool,
    positive_streak: u8,
    negative_streak: u8,
}

#[derive(Debug)]
pub struct MemoryLeakDetector {
    model: RuntimeLeakModel,
    window_size: usize,
    stale_after_cycles: u64,
    cycle: u64,
    process_windows: HashMap<Pid, ProcessWindowState>,
    anomaly_state_by_pid: HashMap<Pid, ProcessAnomalyState>,
}

impl MemoryLeakDetector {
    pub fn load_from_path<P: AsRef<Path>>(path: P, window_size: usize) -> Result<Self, String> {
        let model = RuntimeLeakModel::load_from_path(path)?;
        Ok(Self {
            model,
            window_size: window_size.max(2),
            stale_after_cycles: DEFAULT_STALE_AFTER_CYCLES,
            cycle: 0,
            process_windows: HashMap::new(),
            anomaly_state_by_pid: HashMap::new(),
        })
    }

    pub fn with_defaults<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        Self::load_from_path(path, DEFAULT_WINDOW_SIZE)
    }

    pub fn evaluate(
        &mut self,
        collected_at: SystemTime,
        cpu_samples: &[ProcessCpuSampleDTO],
    ) -> DetectorResult {
        self.cycle = self.cycle.saturating_add(1);

        let mut active_pids: HashSet<Pid> = HashSet::with_capacity(cpu_samples.len());
        let mut anomaly_by_pid = HashMap::with_capacity(cpu_samples.len());
        let mut transitions = Vec::new();

        for sample in cpu_samples {
            active_pids.insert(sample.pid);

            let process_state =
                self.process_windows
                    .entry(sample.pid)
                    .or_insert_with(|| ProcessWindowState {
                        first_seen: collected_at,
                        last_seen_cycle: self.cycle,
                        points: VecDeque::with_capacity(self.window_size),
                    });

            process_state.last_seen_cycle = self.cycle;
            let elapsed_s = collected_at
                .duration_since(process_state.first_seen)
                .unwrap_or(Duration::ZERO)
                .as_secs_f64();

            process_state.points.push_back(RuntimeSamplePoint {
                elapsed_s,
                observed_memory_kb: sample.physical_mem as f64,
                workload_value: sample.cpu_top.max(0.0),
            });

            if process_state.points.len() > self.window_size {
                process_state.points.pop_front();
            }

            let predicted_positive = if process_state.points.len() < self.window_size {
                false
            } else {
                let row = build_runtime_feature_row(&process_state.points);
                if !is_meaningful_growth(&row) {
                    false
                } else {
                    match self.model.predict_labels(&[row]) {
                        Ok(pred) => pred.first().copied().unwrap_or(0) == 1,
                        Err(err) => {
                            warn!(
                                target: "monitor::anomaly",
                                error = ?err,
                                pid = sample.pid,
                                "failed to run anomaly prediction"
                            );
                            false
                        }
                    }
                }
            };

            let state = self.anomaly_state_by_pid.entry(sample.pid).or_default();

            if predicted_positive {
                state.positive_streak = state.positive_streak.saturating_add(1);
                state.negative_streak = 0;
            } else {
                state.negative_streak = state.negative_streak.saturating_add(1);
                state.positive_streak = 0;
            }

            let was_anomalous = state.is_anomalous;
            let is_anomalous = if was_anomalous {
                state.negative_streak < MIN_NEGATIVE_STREAK
            } else {
                state.positive_streak >= MIN_POSITIVE_STREAK
            };

            if was_anomalous != is_anomalous {
                transitions.push(AnomalyTransition {
                    pid: sample.pid,
                    process_name: sample.name.clone(),
                    is_anomalous,
                });
            }

            state.is_anomalous = is_anomalous;
            anomaly_by_pid.insert(sample.pid, is_anomalous);
        }

        self.prune_stale_state(&active_pids);

        DetectorResult {
            anomaly_by_pid,
            transitions,
        }
    }

    fn prune_stale_state(&mut self, active_pids: &HashSet<Pid>) {
        let stale_cutoff = self.cycle.saturating_sub(self.stale_after_cycles);
        let stale_pids = self
            .process_windows
            .iter()
            .filter_map(|(pid, state)| {
                if state.last_seen_cycle <= stale_cutoff && !active_pids.contains(pid) {
                    Some(*pid)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for pid in stale_pids {
            self.process_windows.remove(&pid);
            self.anomaly_state_by_pid.remove(&pid);
        }
    }
}

fn is_meaningful_growth(row: &FeatureRow) -> bool {
    let mut passed = 0u8;

    if row.memory_slope >= MIN_MEMORY_SLOPE_KB_PER_S {
        passed += 1;
    }
    if row.memory_delta_mean >= MIN_MEMORY_DELTA_MEAN_KB {
        passed += 1;
    }
    if row.memory_delta_max >= MIN_MEMORY_DELTA_MAX_KB {
        passed += 1;
    }

    passed >= 2
}

fn build_runtime_feature_row(window: &VecDeque<RuntimeSamplePoint>) -> FeatureRow {
    let first = match window.front() {
        Some(first) => first,
        None => {
            return FeatureRow {
                memory_slope: 0.0,
                memory_delta_mean: 0.0,
                memory_delta_max: 0.0,
                workload_mean: 0.0,
            };
        }
    };

    let last = window.back().unwrap_or(first);

    let dt = (last.elapsed_s - first.elapsed_s).abs();
    let memory_slope = if dt < f64::EPSILON {
        0.0
    } else {
        (last.observed_memory_kb - first.observed_memory_kb) / dt
    };

    let mut positive_delta_sum = 0.0;
    let mut positive_delta_max = 0.0;

    let points = window.iter().collect::<Vec<_>>();
    for pair in points.windows(2) {
        let delta = (pair[1].observed_memory_kb - pair[0].observed_memory_kb).max(0.0);
        positive_delta_sum += delta;
        if delta > positive_delta_max {
            positive_delta_max = delta;
        }
    }

    let delta_denominator = (window.len().saturating_sub(1)).max(1) as f64;
    let memory_delta_mean = positive_delta_sum / delta_denominator;

    let workload_sum = window.iter().map(|point| point.workload_value).sum::<f64>();
    let workload_mean = workload_sum / window.len().max(1) as f64;

    FeatureRow {
        memory_slope,
        memory_delta_mean,
        memory_delta_max: positive_delta_max,
        workload_mean,
    }
}
