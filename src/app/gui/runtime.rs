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
    view_models::cpu_rows_from_dtos,
};

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
