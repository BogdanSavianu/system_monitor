#[cfg(feature = "dioxus-gui")]
use dioxus_desktop::{Config, WindowBuilder};
use system_monitor::util::ParseError;
#[cfg(feature = "dioxus-gui")]
use tracing::{info, warn};

#[cfg(feature = "dioxus-gui")]
use crate::app::gui::replay::types::ReplaySample;
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
    components::{AppNav, LeakReportModalProps, ReplayScanModal, render_leak_report_modal},
    runtime::run_sync_loop,
    settings_store::{
        GuiPersistentSettings, gui_settings_file_path, load_gui_settings, save_gui_settings,
    },
    state::{GuiPage, GuiState, LeaksSortKey, MonitorSortKey},
    views::{
        LeaksViewProps, ProcessTreeViewProps, ProcessesViewProps, ReplayViewProps, SettingsView,
        render_leaks_view, render_process_tree_view, render_processes_view, render_replay_view,
        render_system_view,
    },
};
#[cfg(feature = "dioxus-gui")]
use crate::app::factory::MonitorBuildSettings;
#[cfg(feature = "dioxus-gui")]
use system_monitor::util::Pid;

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
fn apply_process_control_status(
    state: &mut GuiState,
    result: Option<Result<(), String>>,
    verb: &str,
    pid: Pid,
) {
    state.status_line = match result {
        Some(Ok(())) => format!("queued {} request for pid {}", verb, pid),
        Some(Err(err)) => format!("failed to queue {} request for pid {}: {}", verb, pid, err),
        None => "backend is not available".to_string(),
    };
}

#[cfg(feature = "dioxus-gui")]
fn make_terminate_cb(
    mut state: Signal<GuiState>,
    backend: Signal<Option<GuiBackendHandle>>,
) -> Callback<Pid> {
    Callback::new(move |pid| {
        let result = backend.with(|slot| slot.as_ref().map(|h| h.terminate_process(pid)));
        state.with_mut(|s| {
            s.close_process_context();
            apply_process_control_status(s, result, "terminate", pid);
        });
    })
}

#[cfg(feature = "dioxus-gui")]
fn make_kill_cb(
    mut state: Signal<GuiState>,
    backend: Signal<Option<GuiBackendHandle>>,
) -> Callback<Pid> {
    Callback::new(move |pid| {
        let result = backend.with(|slot| slot.as_ref().map(|h| h.force_kill_process(pid)));
        state.with_mut(|s| {
            s.close_process_context();
            apply_process_control_status(s, result, "kill", pid);
        });
    })
}

#[cfg(feature = "dioxus-gui")]
fn save_settings(settings: GuiPersistentSettings) {
    if let Err(err) = save_gui_settings(settings) {
        warn!(target: "app::gui_settings", error = %err, "failed to persist gui settings");
    }
}

#[cfg(feature = "dioxus-gui")]
fn save_and_restart(settings: GuiPersistentSettings, backend: Signal<Option<GuiBackendHandle>>) {
    save_settings(settings);
    restart_backend_with_settings(backend, settings);
}

#[cfg(feature = "dioxus-gui")]
pub fn run_gui_app() -> Result<(), ParseError> {
    LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(WindowBuilder::new().with_title("System Monitor")))
        .launch(GuiApp);
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
        state.tz_offset_hours = persisted_settings.tz_offset_hours;
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

    let state_read = state.read();
    let selected_pid = state_read.selected_pid;
    let details_expanded = state_read.details_expanded;
    let active_page = state_read.active_page;
    let monitor_sort_key = state_read.monitor_sort_key;
    let monitor_sort_direction = state_read.monitor_sort_direction;
    let leaks_sort_key = state_read.leaks_sort_key;
    let leaks_sort_direction = state_read.leaks_sort_direction;
    let process_context_pid = state_read.process_context_pid;
    let settings_storage_enabled = state_read.settings_storage_enabled;
    let settings_anomaly_enabled = state_read.settings_anomaly_enabled;
    let tz_offset_hours = state_read.tz_offset_hours;
    let view_filter_text = state_read.filter_text.clone();
    let tree_filter_text = state_read.tree_filter_text.clone();
    let tree_focus_pid = state_read.tree_focus_pid;
    let tree_stats: std::collections::HashMap<
        system_monitor::util::Pid,
        crate::app::gui::views::NodeStats,
    > = state_read
        .rows
        .iter()
        .map(|row| {
            (
                row.pid,
                crate::app::gui::views::NodeStats {
                    cpu_top: row.cpu_top,
                    physical_mem_mb: row.physical_mem as f64 / 1000.0,
                    is_anomalous: row.is_anomalous,
                },
            )
        })
        .collect();
    let replay_state = state_read.replay.clone();

    let leak_alert_count = state_read
        .detected_leaks
        .iter()
        .filter(|l| l.is_active)
        .count();

    let monitor_settings_for_db = MonitorBuildSettings::from_env();
    let db_path_str = monitor_settings_for_db
        .storage_db_path
        .display()
        .to_string();
    let db_size_bytes = std::fs::metadata(&monitor_settings_for_db.storage_db_path)
        .map(|m| m.len())
        .ok();

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

                AppNav {
                    active_page: active_page,
                    leak_alert_count: leak_alert_count,
                    on_change: move |next_page| {
                        state.with_mut(|s| s.active_page = next_page);
                        backend.with(|slot| {
                            if let Some(b) = slot.as_ref() {
                                let _ = b.set_active_page(next_page);
                            }
                        });
                    }
                }

                if active_page == GuiPage::Monitor {
                    {render_processes_view(ProcessesViewProps {
                        rows: &state_read.rows,
                        thread_rows: &state_read.thread_rows,
                        network_rows: &state_read.network_rows,
                        cmdline_by_pid: &state_read.cmdline_by_pid,
                        cpu_top_history_by_pid: &state_read.cpu_top_history_by_pid,
                        physical_mem_history_by_pid: &state_read.physical_mem_history_by_pid,
                        restart_count_by_name: &state_read.restart_count_by_name,
                        memory_baseline_by_name: &state_read.memory_baseline_by_name,
                        selected_pid,
                        details_expanded,
                        monitor_sort_key,
                        monitor_sort_direction,
                        filter_text: &view_filter_text,
                        subtree_filter_pids: state_read.subtree_filter_pids.as_ref(),
                        process_context_pid,
                        on_filter_change: Callback::new(move |value| {
                            state.with_mut(|state| state.filter_text = value);
                        }),
                        on_clear_subtree_filter: Callback::new(move |_| {
                            state.with_mut(|state| state.subtree_filter_pids = None);
                        }),
                        on_sort_change: Callback::new(move |key: MonitorSortKey| {
                            state.with_mut(|state| state.toggle_monitor_sort(key));
                        }),
                        on_select: Callback::new(move |pid| {
                            state.with_mut(|state| {
                                state.selected_pid = Some(pid);
                            });
                        }),
                        on_open_context_menu: Callback::new(move |pid| {
                            state.with_mut(|state| state.open_process_context(pid));
                        }),
                        on_close_context_menu: Callback::new(move |_| {
                            state.with_mut(|state| state.close_process_context());
                        }),
                        on_terminate_process: make_terminate_cb(state, backend),
                        on_kill_process: make_kill_cb(state, backend),
                        on_toggle_details: Callback::new(move |_| {
                            state.with_mut(|state| {
                                state.details_expanded = !state.details_expanded;
                            });
                        }),
                    })}
                } else if active_page == GuiPage::Leaks {
                    {render_leaks_view(LeaksViewProps {
                        detected_leaks: &state_read.detected_leaks,
                        physical_mem_history_by_pid: &state_read.physical_mem_history_by_pid,
                        dismissed_alert_pids: &state_read.dismissed_alert_pids,
                        process_context_pid,
                        leaks_sort_key,
                        leaks_sort_direction,
                        tz_offset_hours,
                        deep_scan_by_pid: &state_read.deep_scan_by_pid,
                        reachability_by_pid: &state_read.reachability_by_pid,
                        on_sort_change: Callback::new(move |key: LeaksSortKey| {
                            state.with_mut(|state| state.toggle_leaks_sort(key));
                        }),
                        on_dismiss_alert: Callback::new(move |pid: Pid| {
                            state.with_mut(|state| state.dismiss_leak_alert(pid));
                        }),
                        on_open_context_menu: Callback::new(move |pid| {
                            state.with_mut(|state| state.open_process_context(pid));
                        }),
                        on_close_context_menu: Callback::new(move |_| {
                            state.with_mut(|state| state.close_process_context());
                        }),
                        on_terminate_process: make_terminate_cb(state, backend),
                        on_kill_process: make_kill_cb(state, backend),
                        on_open_report: Callback::new(move |pid: Pid| {
                            state.with_mut(|s| s.open_leak_report(pid));
                        }),
                    })}
                } else if active_page == GuiPage::Tree {
                    {render_process_tree_view(ProcessTreeViewProps {
                        roots: &state_read.process_tree_roots,
                        expanded: &state_read.process_tree_expanded,
                        tree_filter_text: &tree_filter_text,
                        tree_focus_pid,
                        stats: &tree_stats,
                        on_filter_change: Callback::new(move |value| {
                            state.with_mut(|state| state.set_tree_filter_text(value));
                        }),
                        on_toggle_node: Callback::new(move |pid| {
                            state.with_mut(|state| state.toggle_tree_node(pid));
                        }),
                        on_expand_all: Callback::new(move |_| {
                            state.with_mut(|state| state.expand_all_tree_nodes());
                        }),
                        on_collapse_all: Callback::new(move |_| {
                            state.with_mut(|state| state.collapse_all_tree_nodes());
                        }),
                        on_focus_pid: Callback::new(move |pid| {
                            state.with_mut(|state| state.set_tree_focus_pid(pid));
                        }),
                        on_jump_to_monitor: Callback::new(move |pids: std::collections::HashSet<Pid>| {
                            state.with_mut(|state| {
                                state.active_page = GuiPage::Monitor;
                                state.subtree_filter_pids = Some(pids);
                            });
                            backend.with(|slot| {
                                if let Some(b) = slot.as_ref() {
                                    let _ = b.set_active_page(GuiPage::Monitor);
                                }
                            });
                        }),
                    })}
                } else if active_page == GuiPage::Replay {
                    {render_replay_view(ReplayViewProps {
                        replay: &replay_state,
                        tz_offset_hours,
                        on_name_lookup_change: Callback::new(move |value| {
                            state.with_mut(|s| s.replay.set_name_lookup_input(value));
                        }),
                        on_name_search: Callback::new(move |value: String| {
                            let _ = backend.with(|slot| {
                                slot.as_ref().map(|handle| handle.search_process_names(value))
                            });
                        }),
                        on_name_pick: Callback::new(move |name: String| {
                            state.with_mut(|s| s.replay.pick_name_suggestion(name));
                        }),
                        on_start_change: Callback::new(move |value| {
                            state.with_mut(|state| state.set_replay_start_input(value));
                        }),
                        on_end_change: Callback::new(move |value| {
                            state.with_mut(|state| state.set_replay_end_input(value));
                        }),
                        on_preset: Callback::new(move |ms: i64| {
                            state.with_mut(|state| state.replay.apply_preset_range(ms));
                        }),
                        on_load: Callback::new(move |_| {
                            let request = state.with(|s| s.replay.build_name_request());
                            match request {
                                Ok((name, start_ms, end_ms)) => {
                                    let send_result = backend.with(|slot| {
                                        slot.as_ref().map(|handle| {
                                            let r = handle.fetch_replay_memory_by_name(
                                                name.clone(),
                                                start_ms,
                                                end_ms,
                                            );
                                            let _ = handle.fetch_replay_scans_by_name(
                                                name.clone(),
                                                start_ms,
                                                end_ms,
                                            );
                                            let _ = handle.fetch_replay_allocation_by_name(
                                                name.clone(),
                                                start_ms,
                                                end_ms,
                                            );
                                            r
                                        })
                                    });
                                    state.with_mut(|s| match send_result {
                                        Some(Ok(())) => {
                                            s.replay.mark_loading();
                                            s.status_line = format!(
                                                "loading replay for '{}' in [{} - {}]",
                                                name, start_ms, end_ms
                                            );
                                        }
                                        Some(Err(err)) => {
                                            s.replay.set_error(format!(
                                                "failed to request replay: {}",
                                                err
                                            ));
                                        }
                                        None => {
                                            s.replay
                                                .set_error("backend is not available".to_string());
                                        }
                                    });
                                }
                                Err(err) => {
                                    state.with_mut(|s| s.replay.set_error(err));
                                }
                            }
                        }),
                        on_toggle_play_pause: Callback::new(move |_| {
                            state.with_mut(|state| state.replay.toggle_play_pause());
                        }),
                        on_stop: Callback::new(move |_| {
                            state.with_mut(|state| state.replay.stop());
                        }),
                        on_seek_back: Callback::new(move |_| {
                            state.with_mut(|state| state.replay.seek_by_seconds(-10));
                        }),
                        on_seek_forward: Callback::new(move |_| {
                            state.with_mut(|state| state.replay.seek_by_seconds(10));
                        }),
                        on_speed_change: Callback::new(move |value| {
                            state.with_mut(|state| state.set_replay_speed_from_input(value));
                        }),
                        on_seek_ratio_change: Callback::new(move |value| {
                            state.with_mut(|state| state.set_replay_seek_ratio_from_input(value));
                        }),
                        on_export: Callback::new(move |_| {
                            let samples = state.with(|s| s.replay.samples.clone());
                            let msg = export_replay_csv(&samples);
                            state.with_mut(|s| s.replay.message = msg);
                        }),
                        on_load_sessions: Callback::new(move |_| {
                            let name_filter =
                                state.with(|s| s.replay.name_lookup_input.trim().to_string());
                            let send_result = backend.with(|slot| {
                                slot.as_ref()
                                    .map(|handle| handle.fetch_sessions_list(name_filter))
                            });
                            state.with_mut(|s| {
                                if send_result.is_some() {
                                    s.replay.mark_sessions_loading();
                                }
                            });
                        }),
                        on_session_pick: Callback::new(move |(first_ms, last_ms): (i64, i64)| {
                            state.with_mut(|s| s.replay.apply_session_range(first_ms, last_ms));
                        }),
                        on_scan_marker_click: Callback::new(move |scan_id: i64| {
                            state.with_mut(|s| s.replay.select_scan(scan_id));
                            let _ = backend.with(|slot| {
                                slot.as_ref()
                                    .map(|handle| handle.fetch_leaked_blocks_for_scan(scan_id))
                            });
                        }),
                    })}
                } else if active_page == GuiPage::System {
                    {render_system_view(
                        &state_read.system_cpu_history,
                        &state_read.system_mem_used_history_mb,
                        state_read.load_avg,
                        state_read.num_cores,
                    )}
                } else {
                    SettingsView {
                        storage_enabled: settings_storage_enabled,
                        anomaly_enabled: settings_anomaly_enabled,
                        db_path: db_path_str,
                        db_size_bytes: db_size_bytes,
                        tz_offset_hours: tz_offset_hours,
                        on_storage_toggle: move |enabled| {
                            let settings = state.with_mut(|s| {
                                s.settings_storage_enabled = enabled;
                                s.status_line = "applying settings...".to_string();
                                GuiPersistentSettings {
                                    storage_enabled: s.settings_storage_enabled,
                                    anomaly_enabled: s.settings_anomaly_enabled,
                                    tz_offset_hours: s.tz_offset_hours,
                                }
                            });
                            save_and_restart(settings, backend);
                        },
                        on_anomaly_toggle: move |enabled| {
                            let settings = state.with_mut(|s| {
                                s.settings_anomaly_enabled = enabled;
                                s.status_line = "applying settings...".to_string();
                                GuiPersistentSettings {
                                    storage_enabled: s.settings_storage_enabled,
                                    anomaly_enabled: s.settings_anomaly_enabled,
                                    tz_offset_hours: s.tz_offset_hours,
                                }
                            });
                            save_and_restart(settings, backend);
                        },
                        on_tz_change: move |offset: i8| {
                            let settings = state.with_mut(|s| {
                                s.tz_offset_hours = offset;
                                GuiPersistentSettings {
                                    storage_enabled: s.settings_storage_enabled,
                                    anomaly_enabled: s.settings_anomaly_enabled,
                                    tz_offset_hours: s.tz_offset_hours,
                                }
                            });
                            save_settings(settings);
                        },
                        on_reset: move |_| {
                            let settings = state.with_mut(|s| {
                                s.settings_storage_enabled = false;
                                s.settings_anomaly_enabled = false;
                                s.tz_offset_hours = 2;
                                s.status_line = "applying settings...".to_string();
                                GuiPersistentSettings::default()
                            });
                            save_and_restart(settings, backend);
                        },
                        on_reset_history: move |_| {
                            backend.with(|slot| {
                                if let Some(b) = slot.as_ref() {
                                    let _ = b.reset_history();
                                }
                            });
                        },
                    }
                }
            }

            if active_page == GuiPage::Leaks {
                if let Some(open_pid) = state_read.leak_report_open_pid {
                    {
                        let deep_state = state_read
                            .deep_scan_by_pid
                            .get(&open_pid)
                            .cloned();
                        let reach_state = state_read
                            .reachability_by_pid
                            .get(&open_pid)
                            .cloned();
                        let name = state_read
                            .detected_leaks
                            .iter()
                            .find(|l| l.pid == open_pid)
                            .map(|l| l.name.clone())
                            .unwrap_or_default();
                        let deep_available = state_read.deep_scan_available;
                        let reach_available = state_read.reachability_available;
                        let group_by_line = state_read.leak_report_group_by_line;
                        render_leak_report_modal(LeakReportModalProps {
                            pid: open_pid,
                            name,
                            deep: deep_state,
                            reach: reach_state,
                            deep_available,
                            reach_available,
                            group_by_line,
                            on_toggle_group: Callback::new(move |_| {
                                state.with_mut(|s| s.toggle_leak_report_grouping());
                            }),
                            on_close: Callback::new(move |_| {
                                state.with_mut(|s| s.close_leak_report());
                            }),
                            on_start_continuous: Callback::new(move |pid: Pid| {
                                backend.with(|slot| {
                                    if let Some(b) = slot.as_ref() {
                                        let _ = b.start_continuous_scan(pid);
                                    }
                                });
                            }),
                            on_stop_continuous: Callback::new(move |pid: Pid| {
                                backend.with(|slot| {
                                    if let Some(b) = slot.as_ref() {
                                        let _ = b.stop_continuous_scan(pid);
                                    }
                                });
                            }),
                            on_reachability_scan: Callback::new(move |pid: Pid| {
                                backend.with(|slot| {
                                    if let Some(b) = slot.as_ref() {
                                        let _ = b.reachability_scan(pid);
                                    }
                                });
                            }),
                        })
                    }
                }
            }

            // scan-detail modal, opened by clicking a diamond marker on the
            // replay timeline
            if active_page == GuiPage::Replay {
                if let Some(scan_id) = replay_state.selected_scan_id {
                    if let Some(scan) = replay_state
                        .scan_markers
                        .iter()
                        .find(|m| m.id == scan_id)
                        .cloned()
                    {
                        ReplayScanModal {
                            scan: scan,
                            blocks: replay_state.selected_scan_blocks.clone(),
                            loading: replay_state.blocks_loading,
                            tz_offset_hours: tz_offset_hours,
                            on_close: move |_| {
                                state.with_mut(|s| s.replay.close_scan_modal());
                            },
                        }
                    }
                }
            }
        }
    }
}

#[cfg(feature = "dioxus-gui")]
fn export_replay_csv(samples: &[ReplaySample]) -> String {
    if samples.is_empty() {
        return "no samples to export".to_string();
    }

    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let ts = samples.first().map(|s| s.collected_at_ms).unwrap_or(0);
    let path = format!("{home}/replay_export_{ts}.csv");

    let mut wtr = csv::Writer::from_path(&path).unwrap();
    wtr.write_record(["collected_at_ms", "physical_mem_mb", "is_anomalous"])
        .unwrap();
    for s in samples {
        wtr.write_record(&[
            s.collected_at_ms.to_string(),
            format!("{:.4}", s.physical_mem_mb),
            s.is_anomalous.to_string(),
        ])
        .unwrap();
    }
    match wtr.flush() {
        Ok(()) => format!("exported {} samples to {}", samples.len(), path),
        Err(e) => format!("export failed: {e}"),
    }
}
