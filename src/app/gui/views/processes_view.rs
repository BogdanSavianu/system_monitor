use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};
use system_monitor::util::Pid;

use crate::app::gui::components::FilterBar;
use crate::app::gui::state::{MonitorSortKey, SortDirection};
use crate::app::gui::view_models::{NetworkRowViewModel, ProcessRowViewModel, ThreadRowViewModel};

use super::{ProcessDetailsProps, ProcessRowProps, render_process_details, render_process_row};

pub struct ProcessesViewProps<'a> {
    pub rows: &'a [ProcessRowViewModel],
    pub thread_rows: &'a [ThreadRowViewModel],
    pub network_rows: &'a [NetworkRowViewModel],
    pub cmdline_by_pid: &'a HashMap<Pid, String>,
    pub cpu_top_history_by_pid: &'a HashMap<Pid, Vec<f64>>,
    pub physical_mem_history_by_pid: &'a HashMap<Pid, Vec<f64>>,
    pub restart_count_by_name: &'a HashMap<String, u32>,
    pub memory_baseline_by_name: &'a HashMap<String, f64>,
    pub selected_pid: Option<u32>,
    pub details_expanded: bool,
    pub monitor_sort_key: MonitorSortKey,
    pub monitor_sort_direction: SortDirection,
    pub filter_text: &'a str,
    pub subtree_filter_pids: Option<&'a HashSet<Pid>>,
    pub process_context_pid: Option<u32>,
    pub on_filter_change: EventHandler<String>,
    pub on_clear_subtree_filter: EventHandler<()>,
    pub on_sort_change: EventHandler<MonitorSortKey>,
    pub on_select: EventHandler<u32>,
    pub on_open_context_menu: EventHandler<u32>,
    pub on_close_context_menu: EventHandler<()>,
    pub on_terminate_process: EventHandler<u32>,
    pub on_kill_process: EventHandler<u32>,
    pub on_toggle_details: EventHandler<()>,
}

pub fn render_processes_view(props: ProcessesViewProps) -> Element {
    let ProcessesViewProps {
        rows,
        thread_rows,
        network_rows,
        cmdline_by_pid,
        cpu_top_history_by_pid,
        physical_mem_history_by_pid,
        restart_count_by_name,
        memory_baseline_by_name,
        selected_pid,
        details_expanded,
        monitor_sort_key,
        monitor_sort_direction,
        filter_text,
        subtree_filter_pids,
        process_context_pid,
        on_filter_change,
        on_clear_subtree_filter,
        on_sort_change,
        on_select,
        on_open_context_menu,
        on_close_context_menu,
        on_terminate_process,
        on_kill_process,
        on_toggle_details,
    } = props;

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
    let selected_restart_count = selected_row
        .map(|r| restart_count_by_name.get(&r.name).copied().unwrap_or(0))
        .unwrap_or(0);
    let selected_mem_baseline =
        selected_row.and_then(|r| memory_baseline_by_name.get(&r.name).copied());

    let mut sorted_rows = rows
        .iter()
        .filter(|row| {
            if let Some(pids) = subtree_filter_pids
                && !pids.contains(&row.pid)
            {
                return false;
            }
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
            MonitorSortKey::DiskRead => a.disk_read_kb_s.total_cmp(&b.disk_read_kb_s),
            MonitorSortKey::DiskWrite => a.disk_write_kb_s.total_cmp(&b.disk_write_kb_s),
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
                {render_process_details(ProcessDetailsProps {
                    selected_row,
                    selected_threads: &selected_threads,
                    selected_network,
                    selected_cmdline,
                    cpu_top_history: selected_cpu_history,
                    physical_mem_history_mb: selected_mem_history,
                    restart_count: selected_restart_count,
                    mem_baseline_mb: selected_mem_baseline,
                    expanded: details_expanded,
                    on_toggle_expand: on_toggle_details,
                })}
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

                    if let Some(pids) = subtree_filter_pids {
                        div {
                            class: "subtree-filter-banner",
                            span { "Showing subtree ({pids.len()} processes)" }
                            button {
                                onclick: move |_| on_clear_subtree_filter.call(()),
                                "✕ Clear"
                            }
                        }
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
                                th { "State" }
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
                                th { "Growth MB/min" }
                                th {
                                    class: "sortable-header",
                                    onclick: move |_| on_sort_change.call(MonitorSortKey::DiskRead),
                                    "Disk R{sort_marker(MonitorSortKey::DiskRead)}"
                                }
                                th {
                                    class: "sortable-header",
                                    onclick: move |_| on_sort_change.call(MonitorSortKey::DiskWrite),
                                    "Disk W{sort_marker(MonitorSortKey::DiskWrite)}"
                                }
                                th { "Action" }
                            }
                        }
                        tbody {
                            for row in sorted_rows {
                                {render_process_row(ProcessRowProps {
                                    row,
                                    selected: selected_pid == Some(row.pid),
                                    process_context_pid,
                                    mem_growth_rate: mem_growth_rate_mb_per_min(
                                        physical_mem_history_by_pid.get(&row.pid).map(Vec::as_slice).unwrap_or(&[])
                                    ),
                                    on_select,
                                    on_open_context_menu,
                                    on_close_context_menu,
                                    on_terminate_process,
                                    on_kill_process,
                                })}
                            }
                        }
                    }
                }

                if has_selected_row {
                    {render_process_details(ProcessDetailsProps {
                        selected_row,
                        selected_threads: &selected_threads,
                        selected_network,
                        selected_cmdline,
                        cpu_top_history: selected_cpu_history,
                        physical_mem_history_mb: selected_mem_history,
                        restart_count: selected_restart_count,
                        mem_baseline_mb: selected_mem_baseline,
                        expanded: details_expanded,
                        on_toggle_expand: on_toggle_details,
                    })}
                }
            }
        }
    }
}

fn mem_growth_rate_mb_per_min(history: &[f64]) -> Option<f64> {
    if history.len() < 2 {
        return None;
    }
    let span_min = (history.len() - 1) as f64 * 2.0 / 60.0;
    Some((history.last().unwrap() - history.first().unwrap()) / span_min)
}
