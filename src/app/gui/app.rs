use system_monitor::util::ParseError;
#[cfg(feature = "dioxus-gui")]
use tracing::{info, warn};

#[cfg(feature = "dioxus-gui")]
use dioxus::prelude::*;
#[cfg(all(feature = "dioxus-gui", debug_assertions))]
use futures_timer::Delay;
#[cfg(all(feature = "dioxus-gui", debug_assertions))]
use std::fs;
#[cfg(feature = "dioxus-gui")]
use std::time::Duration;

#[cfg(feature = "dioxus-gui")]
use super::{
    backend::{GuiBackendHandle, spawn_backend},
    components::AppNav,
    runtime::run_sync_loop,
    settings_store::{
        GuiPersistentSettings, gui_settings_file_path, load_gui_settings, save_gui_settings,
    },
    state::{GuiPage, GuiState},
    views::{ProcessesView, SettingsView},
};
#[cfg(feature = "dioxus-gui")]
use crate::app::factory::MonitorBuildSettings;

#[cfg(feature = "dioxus-gui")]
const APP_CSS: &str = include_str!("styles/app.css");
#[cfg(feature = "dioxus-gui")]
const BACKEND_SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
#[cfg(all(feature = "dioxus-gui", debug_assertions))]
const APP_CSS_DEV_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/app/gui/styles/app.css");

#[cfg(all(feature = "dioxus-gui", debug_assertions))]
fn load_initial_css() -> String {
    fs::read_to_string(APP_CSS_DEV_PATH).unwrap_or_else(|_| APP_CSS.to_string())
}

#[cfg(all(feature = "dioxus-gui", debug_assertions))]
fn use_dev_css_hot_reload(mut css: Signal<String>) {
    let mut css_last_modified = use_signal(|| {
        fs::metadata(APP_CSS_DEV_PATH)
            .ok()
            .and_then(|meta| meta.modified().ok())
    });

    use_future(move || async move {
        loop {
            let next_modified = fs::metadata(APP_CSS_DEV_PATH)
                .ok()
                .and_then(|meta| meta.modified().ok());

            let should_reload = {
                let prev = *css_last_modified.read();
                next_modified != prev
            };

            if should_reload {
                css_last_modified.set(next_modified);

                match fs::read_to_string(APP_CSS_DEV_PATH) {
                    Ok(next_css) => {
                        css.set(next_css);
                        info!(
                            target: "app::gui_css",
                            path = APP_CSS_DEV_PATH,
                            "reloaded css from disk"
                        );
                    }
                    Err(err) => {
                        warn!(
                            target: "app::gui_css",
                            path = APP_CSS_DEV_PATH,
                            error = %err,
                            "failed to reload css from disk; keeping previous css"
                        );
                    }
                }
            }

            Delay::new(Duration::from_millis(500)).await;
        }
    });
}

#[cfg(feature = "dioxus-gui")]
fn restart_backend_with_settings(
    mut backend: Signal<Option<GuiBackendHandle>>,
    settings: GuiPersistentSettings,
) {
    let monitor_settings = MonitorBuildSettings::from_env()
        .with_toggles(settings.storage_enabled, settings.anomaly_enabled);

    backend.with_mut(|slot| {
        if let Some(handle) = slot.as_mut() {
            handle.shutdown();
        }

        *slot = Some(spawn_backend(BACKEND_SAMPLE_INTERVAL, monitor_settings));
    });
}

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
    #[cfg(debug_assertions)]
    let css = use_signal(load_initial_css);
    #[cfg(not(debug_assertions))]
    let css = use_signal(|| APP_CSS.to_string());

    #[cfg(debug_assertions)]
    use_dev_css_hot_reload(css);

    let initial_settings = use_signal(|| {
        let settings_path = gui_settings_file_path();
        let loaded = load_gui_settings().unwrap_or_else(|err| {
            warn!(
                target: "app::gui_settings",
                error = %err,
                "failed to load persisted gui settings; using defaults"
            );
            GuiPersistentSettings::default()
        });

        info!(
            target: "app::gui_settings",
            path = %settings_path.display(),
            storage_enabled = loaded.storage_enabled,
            anomaly_enabled = loaded.anomaly_enabled,
            "loaded gui settings"
        );

        loaded
    });
    let persisted_settings = *initial_settings.read();

    let mut state = use_signal(move || {
        let mut state = GuiState::new();
        state.settings_storage_enabled = persisted_settings.storage_enabled;
        state.settings_anomaly_enabled = persisted_settings.anomaly_enabled;
        state
    });
    let backend = use_signal(move || {
        let monitor_settings = MonitorBuildSettings::from_env().with_toggles(
            persisted_settings.storage_enabled,
            persisted_settings.anomaly_enabled,
        );
        Some(spawn_backend(BACKEND_SAMPLE_INTERVAL, monitor_settings))
    });

    use_future(move || run_sync_loop(state, backend));

    let state_read = state.read().clone();
    let rows = state_read.rows.clone();
    let thread_rows = state_read.thread_rows.clone();
    let network_rows = state_read.network_rows.clone();
    let cmdline_by_pid = state_read.cmdline_by_pid.clone();
    let cpu_top_history_by_pid = state_read.cpu_top_history_by_pid.clone();
    let selected_pid = state_read.selected_pid;
    let details_expanded = state_read.details_expanded;
    let active_page = state_read.active_page;
    let settings_storage_enabled = state_read.settings_storage_enabled;
    let settings_anomaly_enabled = state_read.settings_anomaly_enabled;
    let status_line = state_read.status_line.clone();
    let view_filter_text = state_read.filter_text.clone();
    let active_css = css.read().clone();

    rsx! {
        div {
            class: "root",
            style { "{active_css}" }

            div {
                class: if details_expanded && active_page == GuiPage::Monitor {
                    "shell shell-fullscreen"
                } else {
                    "shell"
                },
                h1 { "System Monitor" }
                p { class: "subtitle", "{status_line}" }

                AppNav {
                    active_page: active_page,
                    on_change: move |next_page| {
                        state.with_mut(|state| state.active_page = next_page);
                    }
                }

                if active_page == GuiPage::Monitor {
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
                } else {
                    SettingsView {
                        storage_enabled: settings_storage_enabled,
                        anomaly_enabled: settings_anomaly_enabled,
                        on_storage_toggle: move |enabled| {
                            let settings = state.with_mut(|state| {
                                state.settings_storage_enabled = enabled;
                                state.status_line = "applying settings...".to_string();
                                GuiPersistentSettings {
                                    storage_enabled: state.settings_storage_enabled,
                                    anomaly_enabled: state.settings_anomaly_enabled,
                                }
                            });

                            if let Err(err) = save_gui_settings(settings) {
                                warn!(
                                    target: "app::gui_settings",
                                    error = %err,
                                    "failed to persist gui settings"
                                );
                            }

                            restart_backend_with_settings(backend, settings);
                        },
                        on_anomaly_toggle: move |enabled| {
                            let settings = state.with_mut(|state| {
                                state.settings_anomaly_enabled = enabled;
                                state.status_line = "applying settings...".to_string();
                                GuiPersistentSettings {
                                    storage_enabled: state.settings_storage_enabled,
                                    anomaly_enabled: state.settings_anomaly_enabled,
                                }
                            });

                            if let Err(err) = save_gui_settings(settings) {
                                warn!(
                                    target: "app::gui_settings",
                                    error = %err,
                                    "failed to persist gui settings"
                                );
                            }

                            restart_backend_with_settings(backend, settings);
                        },
                        on_reset: move |_| {
                            let settings = state.with_mut(|state| {
                                state.settings_storage_enabled = false;
                                state.settings_anomaly_enabled = false;
                                state.status_line = "applying settings...".to_string();
                                GuiPersistentSettings::default()
                            });

                            if let Err(err) = save_gui_settings(settings) {
                                warn!(
                                    target: "app::gui_settings",
                                    error = %err,
                                    "failed to persist gui settings"
                                );
                            }

                            restart_backend_with_settings(backend, settings);
                        },
                    }
                }
            }
        }
    }
}
