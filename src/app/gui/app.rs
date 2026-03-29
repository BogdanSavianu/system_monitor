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
    let selected_pid = state_read.selected_pid;
    let status_line = state_read.status_line.clone();
    let view_filter_text = state_read.filter_text.clone();

    rsx! {
        div {
            class: "demo-root",
            style { "{APP_CSS}" }

            div {
                class: "demo-shell",
                h1 { "System Monitor" }
                p { class: "demo-subtitle", "{status_line}" }

                ProcessesView {
                    rows: rows,
                    selected_pid: selected_pid,
                    filter_text: view_filter_text,
                    on_filter_change: move |value| {
                        state.with_mut(|state| state.filter_text = value);
                    },
                    on_select: move |pid| {
                        state.with_mut(|state| {
                            state.selected_pid = Some(pid);
                        });
                    },
                }
            }
        }
    }
}
