use dioxus::prelude::*;
use std::collections::HashMap;
use system_monitor::util::Pid;

use crate::app::gui::components::FilterBar;
use crate::app::gui::state::{MonitorSortKey, SortDirection};
use crate::app::gui::view_models::{NetworkRowViewModel, ProcessRowViewModel, ThreadRowViewModel};

use super::{render_process_details, render_process_row};

pub fn render_processes_view(
    rows: &[ProcessRowViewModel],
    thread_rows: &[ThreadRowViewModel],
    network_rows: &[NetworkRowViewModel],
    cmdline_by_pid: &HashMap<Pid, String>,
    cpu_top_history_by_pid: &HashMap<Pid, Vec<f64>>,
    physical_mem_history_by_pid: &HashMap<Pid, Vec<f64>>,
    selected_pid: Option<u32>,
    details_expanded: bool,
    monitor_sort_key: MonitorSortKey,
    monitor_sort_direction: SortDirection,
    filter_text: &str,
    on_filter_change: EventHandler<String>,
    on_sort_change: EventHandler<MonitorSortKey>,
    on_select: EventHandler<u32>,
    on_toggle_details: EventHandler<()>,
) -> Element {
    let filter = filter_text.to_lowercase();
    let selected_row = selected_pid.and_then(|pid| rows.iter().find(|row| row.pid == pid));
    let has_selected_row = selected_row.is_some();
    let mut selected_threads: Vec<_> = selected_pid
        .map(|pid| {
            thread_rows
                .iter()
                .filter(|row| row.pid == pid)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    selected_threads.sort_by(|a, b| b.cpu_top.total_cmp(&a.cpu_top));
    let selected_network =
        selected_pid.and_then(|pid| network_rows.iter().find(|row| row.pid == pid));
    let selected_cmdline =
        selected_pid.and_then(|pid| cmdline_by_pid.get(&pid).map(String::as_str));
    let selected_cpu_history = selected_pid
        .and_then(|pid| cpu_top_history_by_pid.get(&pid).map(Vec::as_slice))
        .unwrap_or(&[]);
    let selected_mem_history = selected_pid
        .and_then(|pid| physical_mem_history_by_pid.get(&pid).map(Vec::as_slice))
        .unwrap_or(&[]);

    let mut sorted_rows = rows
        .iter()
        .filter(|row| {
            if filter.is_empty() {
                true
            } else {
                row.pid.to_string().contains(&filter) || row.name.to_lowercase().contains(&filter)
            }
        })
        .collect::<Vec<_>>();

    sorted_rows.sort_by(|a, b| {
        let ordering = match monitor_sort_key {
            MonitorSortKey::Pid => a.pid.cmp(&b.pid),
            MonitorSortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            MonitorSortKey::CpuTop => a.cpu_top.total_cmp(&b.cpu_top),
            MonitorSortKey::CpuRel => a.cpu_rel.total_cmp(&b.cpu_rel),
            MonitorSortKey::VirtualMemory => a.virtual_mem.cmp(&b.virtual_mem),
            MonitorSortKey::PhysicalMemory => a.physical_mem.cmp(&b.physical_mem),
        };

        if monitor_sort_direction == SortDirection::Asc {
            ordering
        } else {
            ordering.reverse()
        }
    });

    let sort_marker = |key: MonitorSortKey| {
        if monitor_sort_key == key {
            if monitor_sort_direction == SortDirection::Asc {
                " ^"
            } else {
                " v"
            }
        } else {
            ""
        }
    };

    rsx! {
        if details_expanded && has_selected_row {
            div {
                class: "details-page",
                {render_process_details(
                    selected_row,
                    &selected_threads,
                    selected_network,
                    selected_cmdline,
                    selected_cpu_history,
                    selected_mem_history,
                    details_expanded,
                    on_toggle_details,
                )}
            }
        } else {
            div {
                class: if has_selected_row {
                    "main-grid"
                } else {
                    "main-grid main-grid-single"
                },

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
                                th {
                                    class: "sortable-header",
                                    onclick: move |_| on_sort_change.call(MonitorSortKey::Pid),
                                    "PID{sort_marker(MonitorSortKey::Pid)}"
                                }
                                th {
                                    class: "sortable-header",
                                    onclick: move |_| on_sort_change.call(MonitorSortKey::Name),
                                    "Name{sort_marker(MonitorSortKey::Name)}"
                                }
                                th {
                                    class: "sortable-header",
                                    onclick: move |_| on_sort_change.call(MonitorSortKey::CpuTop),
                                    "CPU top{sort_marker(MonitorSortKey::CpuTop)}"
                                }
                                th {
                                    class: "sortable-header",
                                    onclick: move |_| on_sort_change.call(MonitorSortKey::CpuRel),
                                    "CPU rel{sort_marker(MonitorSortKey::CpuRel)}"
                                }
                                th {
                                    class: "sortable-header",
                                    onclick: move |_| on_sort_change.call(MonitorSortKey::VirtualMemory),
                                    "Virtual Memory{sort_marker(MonitorSortKey::VirtualMemory)}"
                                }
                                th {
                                    class: "sortable-header",
                                    onclick: move |_| on_sort_change.call(MonitorSortKey::PhysicalMemory),
                                    "Physical Memory{sort_marker(MonitorSortKey::PhysicalMemory)}"
                                }
                            }
                        }
                        tbody {
                            for row in sorted_rows {
                                {render_process_row(row, selected_pid == Some(row.pid), on_select)}
                            }
                        }
                    }
                }

                if has_selected_row {
                    {render_process_details(
                        selected_row,
                        &selected_threads,
                        selected_network,
                        selected_cmdline,
                        selected_cpu_history,
                        selected_mem_history,
                        details_expanded,
                        on_toggle_details,
                    )}
                }
            }
        }
    }
}
