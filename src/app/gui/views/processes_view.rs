use dioxus::prelude::*;
use std::collections::HashMap;
use system_monitor::util::Pid;

use crate::app::gui::components::FilterBar;
use crate::app::gui::view_models::{NetworkRowViewModel, ProcessRowViewModel, ThreadRowViewModel};

use super::{ProcessDetailsView, ProcessRowView};

#[component]
pub fn ProcessesView(
    rows: Vec<ProcessRowViewModel>,
    thread_rows: Vec<ThreadRowViewModel>,
    network_rows: Vec<NetworkRowViewModel>,
    cmdline_by_pid: HashMap<Pid, String>,
    cpu_top_history_by_pid: HashMap<Pid, Vec<f64>>,
    selected_pid: Option<Pid>,
    details_expanded: bool,
    filter_text: String,
    on_filter_change: EventHandler<String>,
    on_select: EventHandler<Pid>,
    on_toggle_details: EventHandler<()>,
) -> Element {
    let filter = filter_text.to_lowercase();
    let selected_row = rows
        .iter()
        .find(|row| Some(row.pid) == selected_pid)
        .cloned();

    let selected_threads = selected_pid
        .map(|pid| {
            let mut rows: Vec<ThreadRowViewModel> = thread_rows
                .iter()
                .filter(|row| row.pid == pid)
                .cloned()
                .collect();
            rows.sort_by(|a, b| b.cpu_top.total_cmp(&a.cpu_top));
            rows
        })
        .unwrap_or_default();

    let selected_network =
        selected_pid.and_then(|pid| network_rows.iter().find(|row| row.pid == pid).cloned());

    let selected_cmdline = selected_pid.and_then(|pid| cmdline_by_pid.get(&pid).cloned());

    let selected_cpu_history = selected_pid
        .and_then(|pid| cpu_top_history_by_pid.get(&pid).cloned())
        .unwrap_or_default();

    rsx! {
        if details_expanded {
            div {
                class: "details-page",
                ProcessDetailsView {
                    selected_row: selected_row,
                    selected_threads: selected_threads,
                    selected_network: selected_network,
                    selected_cmdline: selected_cmdline,
                    cpu_top_history: selected_cpu_history,
                    expanded: details_expanded,
                    on_toggle_expand: on_toggle_details,
                }
            }
        } else {
            div {
                class: "main-grid",

                div {
                    class: "list-panel",
                    FilterBar {
                        filter_text: filter_text,
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
                                ProcessRowView {
                                    row: row.clone(),
                                    selected: selected_pid == Some(row.pid),
                                    on_select: on_select.clone(),
                                }
                            }
                        }
                    }
                }

                ProcessDetailsView {
                    selected_row: selected_row,
                    selected_threads: selected_threads,
                    selected_network: selected_network,
                    selected_cmdline: selected_cmdline,
                    cpu_top_history: selected_cpu_history,
                    expanded: details_expanded,
                    on_toggle_expand: on_toggle_details,
                }
            }
        }
    }
}
