use dioxus::prelude::*;
use std::collections::HashMap;
use system_monitor::util::Pid;

use crate::app::gui::components::FilterBar;
use crate::app::gui::view_models::{NetworkRowViewModel, ProcessRowViewModel, ThreadRowViewModel};

use super::{render_process_details, render_process_row};

pub fn render_processes_view(
    rows: &[ProcessRowViewModel],
    thread_rows: &[ThreadRowViewModel],
    network_rows: &[NetworkRowViewModel],
    cmdline_by_pid: &HashMap<Pid, String>,
    cpu_top_history_by_pid: &HashMap<Pid, Vec<f64>>,
    selected_pid: Option<u32>,
    details_expanded: bool,
    filter_text: &str,
    on_filter_change: EventHandler<String>,
    on_select: EventHandler<u32>,
    on_toggle_details: EventHandler<()>,
) -> Element {
    let filter = filter_text.to_lowercase();
    let selected_row = selected_pid.and_then(|pid| rows.iter().find(|row| row.pid == pid));
    let mut selected_threads: Vec<_> = selected_pid
        .map(|pid| {
            thread_rows
                .iter()
                .filter(|row| row.pid == pid)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    selected_threads.sort_by(|a, b| b.cpu_top.total_cmp(&a.cpu_top));
    let selected_network = selected_pid.and_then(|pid| network_rows.iter().find(|row| row.pid == pid));
    let selected_cmdline = selected_pid.and_then(|pid| cmdline_by_pid.get(&pid).map(String::as_str));
    let selected_cpu_history = selected_pid
        .and_then(|pid| cpu_top_history_by_pid.get(&pid).map(Vec::as_slice))
        .unwrap_or(&[]);

    rsx! {
        if details_expanded {
            div {
                class: "details-page",
                {render_process_details(
                    selected_row,
                    &selected_threads,
                    selected_network,
                    selected_cmdline,
                    selected_cpu_history,
                    details_expanded,
                    on_toggle_details,
                )}
            }
        } else {
            div {
                class: "main-grid",

                div {
                    class: "list-panel",
                    FilterBar {
                        filter_text: filter_text.to_string(),
                        on_change: on_filter_change,
                    }

                    table {
                        class: "table",
                        thead {
                            tr {
                                th { "PID" }
                                th { "Name" }
                                th { "CPU top" }
                                th { "CPU rel" }
                                th { "Virtual Memory" }
                                th { "Physical Memory" }
                            }
                        }
                        tbody {
                            for row in rows.iter().filter(|row| {
                                if filter.is_empty() {
                                    true
                                } else {
                                    row.pid.to_string().contains(&filter)
                                        || row.name.to_lowercase().contains(&filter)
                                }
                            }) {
                                {render_process_row(row, selected_pid == Some(row.pid), on_select)}
                            }
                        }
                    }
                }

                {render_process_details(
                    selected_row,
                    &selected_threads,
                    selected_network,
                    selected_cmdline,
                    selected_cpu_history,
                    details_expanded,
                    on_toggle_details,
                )}
            }
        }
    }
}
