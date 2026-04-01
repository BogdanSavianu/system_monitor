use system_monitor::util::ParseError;

#[cfg(feature = "dioxus-gui")]
use dioxus::prelude::*;
#[cfg(feature = "dioxus-gui")]
use std::time::Duration;

#[cfg(feature = "dioxus-gui")]
use super::{
    backend::spawn_backend, runtime::run_sync_loop, state::GuiState, views::ProcessesView,
};

#[cfg(feature = "dioxus-gui")]
const APP_CSS: &str = include_str!("styles/app.css");

#[cfg(feature = "dioxus-gui")]
pub fn run_gui_app() -> Result<(), ParseError> {
    LaunchBuilder::desktop().launch(GuiApp);
    Ok(())
}

#[cfg(not(feature = "dioxus-gui"))]
pub fn run_gui_app() -> Result<(), ParseError> {
    Err(ParseError::ParsingError(
        "dioxus gui is disabled; run with `--features dioxus-gui`".to_string(),
    ))
}

#[cfg(feature = "dioxus-gui")]
#[allow(non_snake_case)]
fn GuiApp() -> Element {
    let mut state = use_signal(GuiState::new);
    let backend = use_signal(|| Some(spawn_backend(Duration::from_secs(2))));

    use_future(move || run_sync_loop(state, backend));

    let state_read = state.read().clone();
    let rows = state_read.rows.clone();
    let thread_rows = state_read.thread_rows.clone();
    let network_rows = state_read.network_rows.clone();
    let cmdline_by_pid = state_read.cmdline_by_pid.clone();
    let cpu_top_history_by_pid = state_read.cpu_top_history_by_pid.clone();
    let selected_pid = state_read.selected_pid;
    let details_expanded = state_read.details_expanded;
    let status_line = state_read.status_line.clone();
    let view_filter_text = state_read.filter_text.clone();

    rsx! {
        div {
            class: "root",
            style { "{APP_CSS}" }

            div {
                class: if details_expanded { "shell shell-fullscreen" } else { "shell" },
                h1 { "System Monitor" }
                p { class: "subtitle", "{status_line}" }

                ProcessesView {
                    rows: rows,
                    thread_rows: thread_rows,
                    network_rows: network_rows,
                    cmdline_by_pid: cmdline_by_pid,
                    cpu_top_history_by_pid: cpu_top_history_by_pid,
                    selected_pid: selected_pid,
                    details_expanded: details_expanded,
                    filter_text: view_filter_text,
                    on_filter_change: move |value| {
                        state.with_mut(|state| state.filter_text = value);
                    },
                    on_select: move |pid| {
                        state.with_mut(|state| {
                            state.selected_pid = Some(pid);
                        });
                    },
                    on_toggle_details: move |_| {
                        state.with_mut(|state| {
                            state.details_expanded = !state.details_expanded;
                        });
                    },
                }
            }
        }
    }
}
