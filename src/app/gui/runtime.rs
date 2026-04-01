#[cfg(feature = "dioxus-gui")]
use dioxus::prelude::*;
#[cfg(feature = "dioxus-gui")]
use futures_timer::Delay;
#[cfg(feature = "dioxus-gui")]
use std::time::Duration;

#[cfg(feature = "dioxus-gui")]
use super::{
    backend::{BackendEvent, GuiBackendHandle},
    state::GuiState,
    view_models::{cpu_rows_from_dtos, network_rows_from_dtos, thread_rows_from_dtos},
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
                            state.rows = cpu_rows_from_dtos(&snapshot.cpu);
                            state.thread_rows = thread_rows_from_dtos(&snapshot.threads);
                            state.network_rows = network_rows_from_dtos(&snapshot.network);
                            state.cmdline_by_pid = snapshot.cmdline_by_pid;

                            for row in &state.rows {
                                let history =
                                    state.cpu_top_history_by_pid.entry(row.pid).or_default();
                                history.push(row.cpu_top);
                                if history.len() > MAX_HISTORY_POINTS {
                                    let overflow = history.len() - MAX_HISTORY_POINTS;
                                    history.drain(0..overflow);
                                }
                            }

                            state
                                .cpu_top_history_by_pid
                                .retain(|pid, _| state.rows.iter().any(|row| row.pid == *pid));

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
