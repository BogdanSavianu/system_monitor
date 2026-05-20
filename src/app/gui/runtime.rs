#[cfg(feature = "dioxus-gui")]
use dioxus::prelude::*;
#[cfg(feature = "dioxus-gui")]
use futures_timer::Delay;
use std::time::Duration;

#[cfg(feature = "dioxus-gui")]
use super::{
    backend::{BackendEvent, GuiBackendHandle},
    state::GuiState,
    view_models::{
        cpu_rows_from_dtos, hierarchy_from_dtos, network_rows_from_dtos, thread_rows_from_dtos,
    },
};

#[cfg(feature = "dioxus-gui")]
const MAX_HISTORY_POINTS: usize = 60;

#[cfg(feature = "dioxus-gui")]
pub async fn run_sync_loop(
    mut state: Signal<GuiState>,
    mut backend: Signal<Option<GuiBackendHandle>>,
) {
    loop {
        if let Some(handle) = backend.write().as_mut() {
            while let Ok(event) = handle.events_rx.try_recv() {
                match event {
                    BackendEvent::Snapshot(snapshot) => {
                        state.with_mut(|state| {
                            state.anomaly_by_pid = snapshot.anomaly_by_pid;

                            for leak in state.leak_stats_by_pid.values_mut() {
                                leak.is_active = false;
                            }

                            for sample in &snapshot.cpu {
                                let is_anomalous = state
                                    .anomaly_by_pid
                                    .get(&sample.pid)
                                    .copied()
                                    .unwrap_or(false);

                                let leak_stat = state
                                    .leak_stats_by_pid
                                    .entry(sample.pid)
                                    .or_insert_with(|| {
                                        super::state::LeakConfidenceStats::new(
                                            sample.pid,
                                            sample.name.clone(),
                                        )
                                    });

                                leak_stat.name = sample.name.clone();
                                leak_stat.observed_entries =
                                    leak_stat.observed_entries.saturating_add(1);

                                if leak_stat.is_warmup_complete() {
                                    leak_stat.total_entries =
                                        leak_stat.total_entries.saturating_add(1);
                                    if is_anomalous {
                                        leak_stat.anomalous_entries =
                                            leak_stat.anomalous_entries.saturating_add(1);
                                    }
                                }
                                leak_stat.is_active = true;
                                leak_stat.last_seen_anomalous = is_anomalous;
                            }

                            let mut detected_leaks = state
                                .leak_stats_by_pid
                                .values()
                                .filter(|entry| entry.anomalous_entries > 0)
                                .cloned()
                                .collect::<Vec<_>>();
                            detected_leaks.sort_by(|a, b| {
                                b.confidence()
                                    .total_cmp(&a.confidence())
                                    .then_with(|| b.anomalous_entries.cmp(&a.anomalous_entries))
                                    .then_with(|| b.total_entries.cmp(&a.total_entries))
                                    .then_with(|| a.pid.cmp(&b.pid))
                            });
                            state.detected_leaks = detected_leaks;

                            state.rows = cpu_rows_from_dtos(&snapshot.cpu, &state.anomaly_by_pid);
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
                                let history =
                                    state.cpu_top_history_by_pid.entry(row.pid).or_default();
                                history.push(row.cpu_top);
                                if history.len() > MAX_HISTORY_POINTS {
                                    let overflow = history.len() - MAX_HISTORY_POINTS;
                                    history.drain(0..overflow);
                                }

                                let memory_history = state
                                    .physical_mem_history_by_pid
                                    .entry(row.pid)
                                    .or_default();
                                memory_history.push(row.physical_mem as f64 / 1000.0);
                                if memory_history.len() > MAX_HISTORY_POINTS {
                                    let overflow = memory_history.len() - MAX_HISTORY_POINTS;
                                    memory_history.drain(0..overflow);
                                }
                            }

                            state
                                .cpu_top_history_by_pid
                                .retain(|pid, _| state.rows.iter().any(|row| row.pid == *pid));
                            state
                                .physical_mem_history_by_pid
                                .retain(|pid, _| state.rows.iter().any(|row| row.pid == *pid));

                            state.system_cpu_history.push(snapshot.total_cpu_top);
                            if state.system_cpu_history.len() > MAX_HISTORY_POINTS {
                                let overflow = state.system_cpu_history.len() - MAX_HISTORY_POINTS;
                                state.system_cpu_history.drain(0..overflow);
                            }

                            state
                                .system_mem_used_history_mb
                                .push(snapshot.system_mem_used_kb as f64 / 1000.0);
                            if state.system_mem_used_history_mb.len() > MAX_HISTORY_POINTS {
                                let overflow =
                                    state.system_mem_used_history_mb.len() - MAX_HISTORY_POINTS;
                                state.system_mem_used_history_mb.drain(0..overflow);
                            }

                            state.status_line =
                                format!("last sample at {:?}", snapshot.collected_at);
                        });
                    }
                    BackendEvent::Error(err) => {
                        state.with_mut(|state| state.status_line = err);
                    }
                    BackendEvent::Stopped => {
                        state.with_mut(|state| {
                            state.status_line = "sampler stopped".to_string();
                        });
                    }
                }
            }
        }

        Delay::new(Duration::from_millis(100)).await;
    }
}
