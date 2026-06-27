#[cfg(feature = "dioxus-gui")]
use dioxus::prelude::*;
#[cfg(feature = "dioxus-gui")]
use futures_timer::Delay;
use std::time::Duration;

#[cfg(feature = "dioxus-gui")]
use super::{
    backend::{BackendEvent, GuiBackendHandle},
    replay::{controller::tick_replay, types::ReplaySource},
    state::{GuiState, LeakConfidenceStats},
    view_models::{
        cpu_rows_from_dtos, hierarchy_from_dtos, network_rows_from_dtos, thread_rows_from_dtos,
    },
};

#[cfg(feature = "dioxus-gui")]
const MAX_HISTORY_POINTS: usize = 60;
#[cfg(feature = "dioxus-gui")]
const ANOMALY_SUSTAINED_THRESHOLD: u32 = 5;
#[cfg(feature = "dioxus-gui")]
const STALE_FLAGGED_CYCLES: u64 = 900;
#[cfg(feature = "dioxus-gui")]
const STALE_CLEAN_CYCLES: u64 = 150;

#[cfg(feature = "dioxus-gui")]
fn simple_hash(input: &str) -> u64 {
    let mut hash: u64 = 1469598103934665603;
    for b in input.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

#[cfg(feature = "dioxus-gui")]
fn push_bounded(history: &mut Vec<f64>, value: f64) {
    history.push(value);
    if history.len() > MAX_HISTORY_POINTS {
        history.drain(0..history.len() - MAX_HISTORY_POINTS);
    }
}

#[cfg(feature = "dioxus-gui")]
fn compute_z_drift(series: &[f64]) -> f64 {
    if series.len() < 40 {
        return 0.0;
    }

    let recent = &series[series.len().saturating_sub(10)..];
    let baseline = &series[series.len().saturating_sub(40)..series.len().saturating_sub(10)];

    if baseline.is_empty() || recent.is_empty() {
        return 0.0;
    }

    let baseline_mean = baseline.iter().sum::<f64>() / baseline.len() as f64;
    let recent_mean = recent.iter().sum::<f64>() / recent.len() as f64;
    let variance = baseline
        .iter()
        .map(|v| {
            let d = *v - baseline_mean;
            d * d
        })
        .sum::<f64>()
        / baseline.len() as f64;
    let std = variance.sqrt().max(1e-6);

    ((recent_mean - baseline_mean) / std).abs()
}

#[cfg(feature = "dioxus-gui")]
fn apply_backend_event(state: &mut GuiState, event: BackendEvent) {
    match event {
        BackendEvent::Snapshot(snapshot) => {
            let own_pid = std::process::id();
            state.sample_cycle = state.sample_cycle.saturating_add(1);
            state.anomaly_by_pid = snapshot.anomaly_by_pid;

            for leak in state.leak_stats_by_pid.values_mut() {
                leak.is_active = false;
            }

            for sample in &snapshot.cpu {
                // skip our own pid: sampling plus the scan burst would look
                // like a leak, aka observer effect
                if sample.pid == own_pid {
                    continue;
                }
                let is_anomalous = state
                    .anomaly_by_pid
                    .get(&sample.pid)
                    .copied()
                    .unwrap_or(false);

                let leak_stat = state
                    .leak_stats_by_pid
                    .entry(sample.pid)
                    .or_insert_with(|| {
                        LeakConfidenceStats::new(
                            sample.pid,
                            sample.name.clone(),
                            state.sample_cycle,
                        )
                    });

                leak_stat.name = sample.name.clone();
                leak_stat.last_seen_cycle = state.sample_cycle;
                let cmdline = snapshot
                    .cmdline_by_pid
                    .get(&sample.pid)
                    .map(String::as_str)
                    .unwrap_or("");
                let cmdline_hash = simple_hash(cmdline);
                leak_stat.identity_key = format!(
                    "pid:{}|first:{}|cmd:{}",
                    sample.pid, leak_stat.first_seen_cycle, cmdline_hash
                );
                leak_stat.observed_entries = leak_stat.observed_entries.saturating_add(1);

                if is_anomalous {
                    leak_stat.anomalous_streak = leak_stat.anomalous_streak.saturating_add(1);
                } else {
                    leak_stat.anomalous_streak = 0;
                }

                if leak_stat.is_warmup_complete() {
                    leak_stat.total_entries = leak_stat.total_entries.saturating_add(1);
                    if is_anomalous && leak_stat.anomalous_streak >= ANOMALY_SUSTAINED_THRESHOLD {
                        leak_stat.anomalous_entries = leak_stat.anomalous_entries.saturating_add(1);
                    }
                }

                if is_anomalous && leak_stat.anomalous_streak >= ANOMALY_SUSTAINED_THRESHOLD {
                    if leak_stat.first_detected_at.is_none() {
                        leak_stat.first_detected_at = Some(snapshot.collected_at);
                    }
                    leak_stat.last_detected_at = Some(snapshot.collected_at);
                }

                leak_stat.why_flagged = if leak_stat.last_seen_anomalous {
                    format!(
                        "recent anomaly=true, confidence={:.1}%, evidence={}",
                        leak_stat.confidence() * 100.0,
                        leak_stat.total_entries
                    )
                } else {
                    format!(
                        "historical anomaly, confidence={:.1}%",
                        leak_stat.confidence() * 100.0,
                    )
                };

                leak_stat.is_active = true;
                leak_stat.last_seen_anomalous = is_anomalous;
            }

            let mut detected_leaks = state
                .leak_stats_by_pid
                .values()
                .filter(|entry| entry.anomalous_entries > 0)
                .filter(|entry| !entry.ignored_for_session && !entry.ignored_persistent)
                .cloned()
                .collect::<Vec<_>>();
            detected_leaks.sort_by(|a, b| {
                b.is_pinned
                    .cmp(&a.is_pinned)
                    .then_with(|| b.triage_severity().cmp(&a.triage_severity()))
                    .then_with(|| {
                        b.confidence_visible()
                            .unwrap_or(0.0)
                            .total_cmp(&a.confidence_visible().unwrap_or(0.0))
                    })
                    .then_with(|| b.anomalous_entries.cmp(&a.anomalous_entries))
                    .then_with(|| b.total_entries.cmp(&a.total_entries))
                    .then_with(|| a.pid.cmp(&b.pid))
            });
            state.detected_leaks = detected_leaks;

            let current_cycle = state.sample_cycle;
            state.leak_stats_by_pid.retain(|_pid, entry| {
                if entry.is_active {
                    return true;
                }
                let idle = current_cycle.saturating_sub(entry.last_seen_cycle);
                if entry.anomalous_entries > 0 {
                    idle < STALE_FLAGGED_CYCLES
                } else {
                    idle < STALE_CLEAN_CYCLES
                }
            });

            state
                .deep_scan_by_pid
                .retain(|pid, _| state.leak_stats_by_pid.contains_key(pid));
            state
                .reachability_by_pid
                .retain(|pid, _| state.leak_stats_by_pid.contains_key(pid));

            state
                .last_pid_by_name
                .retain(|_, pid| state.rows.iter().any(|row| row.pid == *pid));

            for sample in &snapshot.cpu {
                if let Some(&prev_pid) = state.last_pid_by_name.get(&sample.name)
                    && prev_pid != sample.pid
                {
                    *state
                        .restart_count_by_name
                        .entry(sample.name.clone())
                        .or_insert(0) += 1;
                }
                state
                    .last_pid_by_name
                    .insert(sample.name.clone(), sample.pid);
            }

            state.load_avg = snapshot.load_avg;
            state.num_cores = snapshot.num_cores;
            state.username_by_pid = snapshot.username_by_pid.clone();
            state.rows = cpu_rows_from_dtos(
                &snapshot.cpu,
                &state.anomaly_by_pid,
                &snapshot.username_by_pid,
            );
            state.process_tree_roots = hierarchy_from_dtos(&snapshot.hierarchy_roots);
            state.thread_rows = thread_rows_from_dtos(&snapshot.threads);
            state.network_rows = network_rows_from_dtos(&snapshot.network);
            state.cmdline_by_pid = snapshot.cmdline_by_pid;

            if state.process_tree_expanded.is_empty() {
                for root in &state.process_tree_roots {
                    state.process_tree_expanded.insert(root.pid);
                }
            }

            for row in &state.rows {
                push_bounded(
                    state.cpu_top_history_by_pid.entry(row.pid).or_default(),
                    row.cpu_top,
                );
                push_bounded(
                    state
                        .physical_mem_history_by_pid
                        .entry(row.pid)
                        .or_default(),
                    row.physical_mem as f64 / 1000.0,
                );
            }

            state
                .cpu_top_history_by_pid
                .retain(|pid, _| state.rows.iter().any(|row| row.pid == *pid));
            state
                .physical_mem_history_by_pid
                .retain(|pid, _| state.rows.iter().any(|row| row.pid == *pid));

            push_bounded(&mut state.system_cpu_history, snapshot.total_cpu_top);
            push_bounded(
                &mut state.system_mem_used_history_mb,
                snapshot.system_mem_used_kb as f64 / 1000.0,
            );

            state.status_line = format!("last sample at {:?}", snapshot.collected_at);

            let cpu_drift_z = compute_z_drift(&state.system_cpu_history);
            let mem_drift_z = compute_z_drift(&state.system_mem_used_history_mb);
            let is_alert = cpu_drift_z >= 3.0 || mem_drift_z >= 3.0;
            state.drift_status.cpu_drift_z = cpu_drift_z;
            state.drift_status.mem_drift_z = mem_drift_z;
            state.drift_status.is_alert = is_alert;
            state.drift_status.summary = if is_alert {
                format!(
                    "distribution drift alert (cpu z={:.2}, mem z={:.2})",
                    cpu_drift_z, mem_drift_z
                )
            } else {
                format!(
                    "drift stable (cpu z={:.2}, mem z={:.2})",
                    cpu_drift_z, mem_drift_z
                )
            };
        }
        BackendEvent::ProcessControlSuccess {
            pid,
            operation,
            signal,
        } => {
            state.status_line = format!(
                "process {}: {} sent successfully (signal {})",
                pid, operation, signal
            );
        }
        BackendEvent::ProcessControlError {
            pid,
            operation,
            message,
        } => {
            state.status_line = format!("process {}: {} failed: {}", pid, operation, message);
        }
        BackendEvent::ReplayMemoryHistoryByName { name, samples } => {
            let count = samples.len();
            if count == 0 {
                state.replay.apply_samples(
                    Vec::new(),
                    ReplaySource::None,
                    format!("No samples found for '{}'", name),
                );
            } else {
                state.replay.apply_samples(
                    samples,
                    ReplaySource::SqliteByName,
                    format!("Loaded {} samples for '{}' (by name)", count, name),
                );
            }
        }
        BackendEvent::ReplayScansList { scans } => {
            state.replay.set_scan_markers(scans);
        }
        BackendEvent::ReplayLeakedBlocks { scan_id, blocks } => {
            state.replay.set_selected_scan_blocks(scan_id, blocks);
        }
        BackendEvent::ReplayAllocationSeries { series } => {
            state.replay.set_allocation_series(series);
        }
        BackendEvent::MatchingProcessNames(names) => {
            state.replay.set_name_suggestions(names);
        }
        BackendEvent::SessionsList(sessions) => {
            state.replay.set_sessions_list(sessions);
        }
        BackendEvent::MemoryBaselines(baselines) => {
            state.memory_baseline_by_name = baselines;
        }
        BackendEvent::Error(err) => state.status_line = err,
        BackendEvent::Stopped => state.status_line = "sampler stopped".to_string(),
        BackendEvent::DeepScanAvailability(available) => {
            state.deep_scan_available = available;
        }
        BackendEvent::DeepScanStarted { pid } => state.begin_deep_scan(pid),
        BackendEvent::DeepScanUpdate {
            pid,
            snapshot_count,
            report,
        } => state.update_deep_scan(pid, snapshot_count, report),
        BackendEvent::DeepScanStopped { pid } => state.stop_deep_scan(pid),
        BackendEvent::DeepScanError { pid, message } => state.fail_deep_scan(pid, message),
        BackendEvent::ReachabilityAvailability(available) => {
            state.reachability_available = available;
        }
        BackendEvent::ReachabilityScanStarted { pid } => state.begin_reachability_scan(pid),
        BackendEvent::ReachabilityScanResult(report) => state.complete_reachability_scan(report),
        BackendEvent::ReachabilityScanError { pid, message } => {
            state.fail_reachability_scan(pid, message)
        }
    }
}

#[cfg(feature = "dioxus-gui")]
pub async fn run_sync_loop(
    mut state: Signal<GuiState>,
    mut backend: Signal<Option<GuiBackendHandle>>,
) {
    loop {
        if let Some(handle) = backend.write().as_mut() {
            while let Ok(event) = handle.events_rx.try_recv() {
                state.with_mut(|state| apply_backend_event(state, event));
            }
        }

        let is_playing = state.with(|s| s.replay.is_playing);
        if is_playing {
            state.with_mut(|state| {
                tick_replay(&mut state.replay);
            });
        }

        // playback wants ~10 Hz for smooth cursor motion. idle only polls
        // events_rx, so 250 ms keeps latency fine while cutting the wake rate.
        // snapshots still arrive every ~2 s, the real refresh cadence.
        let next_poll = if is_playing {
            Duration::from_millis(100)
        } else {
            Duration::from_millis(250)
        };
        Delay::new(next_poll).await;
    }
}
