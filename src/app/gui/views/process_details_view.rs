use dioxus::prelude::*;
use plotters::prelude::*;

use crate::app::gui::view_models::{NetworkRowViewModel, ProcessRowViewModel, ThreadRowViewModel};

use super::render_line_chart_svg;

pub struct ProcessDetailsProps<'a> {
    pub selected_row: Option<&'a ProcessRowViewModel>,
    pub selected_threads: &'a [&'a ThreadRowViewModel],
    pub selected_network: Option<&'a NetworkRowViewModel>,
    pub selected_cmdline: Option<&'a str>,
    pub cpu_top_history: &'a [f64],
    pub physical_mem_history_mb: &'a [f64],
    pub restart_count: u32,
    pub mem_baseline_mb: Option<f64>,
    pub expanded: bool,
    pub on_toggle_expand: EventHandler<()>,
}

pub fn render_process_details(props: ProcessDetailsProps) -> Element {
    let ProcessDetailsProps {
        selected_row,
        selected_threads,
        selected_network,
        selected_cmdline,
        cpu_top_history,
        physical_mem_history_mb,
        restart_count,
        mem_baseline_mb,
        expanded,
        on_toggle_expand,
    } = props;

    let cmdline_text = selected_cmdline.unwrap_or("(not available)");
    let max_cpu = cpu_top_history
        .iter()
        .copied()
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let cpu_history_svg = render_line_chart_svg(
        cpu_top_history,
        "% CPU",
        RGBColor(15, 118, 110),
        RGBColor(13, 148, 136),
    );
    let has_cpu_history_svg = cpu_history_svg.is_some();
    let cpu_history_svg_markup = cpu_history_svg.unwrap_or_default();

    let max_mem_mb = physical_mem_history_mb
        .iter()
        .copied()
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let memory_history_svg = render_line_chart_svg(
        physical_mem_history_mb,
        "MB",
        RGBColor(2, 132, 199),
        RGBColor(14, 165, 233),
    );
    let has_memory_history_svg = memory_history_svg.is_some();
    let memory_history_svg_markup = memory_history_svg.unwrap_or_default();

    rsx! {
        aside {
            class: "details-panel",
            div {
                class: "details-header",
                h2 { "Selected process" }
                button {
                    class: "details-expand-btn",
                    onclick: move |_| on_toggle_expand.call(()),
                    if expanded { "Back to split view" } else { "Open full screen" }
                }
            }

            if let Some(selected) = selected_row {

                div {
                    class: "details-section",
                    h3 { "Command line" }
                    p {
                        class: "details-mono",
                        "{cmdline_text}"
                    }
                }

                div {
                    class: "details-section",
                    h3 { "Thread overview" }
                    p { class: "details-subtitle", "{selected_threads.len()} threads" }
                    if selected_threads.is_empty() {
                        p { class: "details-empty", "No thread data available for this process." }
                    } else {
                        table {
                            class: "details-table",
                            thead {
                                tr {
                                    th { "TID" }
                                    th { "Name" }
                                    th { "State" }
                                    th { "CPU top" }
                                }
                            }
                            tbody {
                                for thread in selected_threads.iter().take(8) {
                                    tr {
                                        td { "{thread.tid}" }
                                        td { "{thread.thread_name}" }
                                        td { "{thread.state.unwrap_or('?')}" }
                                        td { "{thread.cpu_top:.2}%" }
                                    }
                                }
                            }
                        }
                    }
                }

                div {
                    class: "details-section",
                    h3 { "Network overview" }
                    if let Some(network) = selected_network {
                        div {
                            class: "details-grid",
                            div {
                                class: "details-label", "TCP open"
                                div { class: "details-value", "{network.tcp_open}" }
                            }
                            div {
                                class: "details-label", "TCP established"
                                div { class: "details-value", "{network.tcp_established}" }
                            }
                            div {
                                class: "details-label", "TCP listen"
                                div { class: "details-value", "{network.tcp_listen}" }
                            }
                            div {
                                class: "details-label", "UDP open"
                                div { class: "details-value", "{network.udp_open}" }
                            }
                            div {
                                class: "details-label", "Total sockets"
                                div { class: "details-value", "{network.total_sockets}" }
                            }
                        }
                    } else {
                        p { class: "details-empty", "No network data available for this process." }
                    }
                }

                if expanded {
                    div {
                        class: "details-section",
                        h3 { "CPU top history" }
                        if !has_cpu_history_svg {
                            p { class: "details-empty", "Not enough samples yet to draw history." }
                        } else {
                            p { class: "details-subtitle", "Peak: {max_cpu:.2}%" }
                            div {
                                class: "graph-line-wrap",
                                div {
                                    class: "graph-svg-host",
                                    dangerous_inner_html: "{cpu_history_svg_markup}",
                                }
                            }
                        }
                    }

                    div {
                        class: "details-section",
                        h3 { "Physical memory history" }
                        if !has_memory_history_svg {
                            p { class: "details-empty", "Not enough samples yet to draw history." }
                        } else {
                            p { class: "details-subtitle", "Peak: {max_mem_mb:.2} MB" }
                            div {
                                class: "graph-line-wrap",
                                div {
                                    class: "graph-svg-host",
                                    dangerous_inner_html: "{memory_history_svg_markup}",
                                }
                            }
                        }
                    }
                }
                p { class: "details-subtitle", "Process details snapshot" }

                div {
                    class: "details-grid",
                    div {
                        class: "details-label", "PID"
                        div { class: "details-value", "{selected.pid}" }
                    }
                    div {
                        class: "details-label", "Name"
                        div { class: "details-value", "{selected.name}" }
                    }
                    div {
                        class: "details-label", "Owner"
                        div { class: "details-value", "{selected.username}" }
                    }
                    div {
                        class: "details-label", "State"
                        div { class: "details-value", "{selected.state}" }
                    }
                    div {
                        class: "details-label", "CPU top"
                        div { class: "details-value", "{selected.cpu_top:.2}%" }
                    }
                    div {
                        class: "details-label", "CPU rel"
                        div { class: "details-value", "{selected.cpu_rel:.2}%" }
                    }
                    div {
                        class: "details-label", "Virtual memory"
                        div { class: "details-value", "{selected.virtual_mem / 1000} MB" }
                    }
                    div {
                        class: "details-label", "Physical memory"
                        div { class: "details-value", "{selected.physical_mem / 1000} MB" }
                    }
                    div {
                        class: "details-label", "Swap"
                        div { class: "details-value", "{selected.swap_mem / 1000} MB" }
                    }
                    div {
                        class: "details-label", "Open FDs"
                        div { class: "details-value", "{selected.fd_count}" }
                    }
                    div {
                        class: "details-label", "Disk read"
                        div { class: "details-value", "{selected.disk_read_kb_s:.1} KB/s" }
                    }
                    div {
                        class: "details-label", "Disk write"
                        div { class: "details-value", "{selected.disk_write_kb_s:.1} KB/s" }
                    }
                    div {
                        class: "details-label", "Restarts"
                        div { class: "details-value", "{restart_count}" }
                    }
                    if let Some(baseline) = mem_baseline_mb {
                        div {
                            class: "details-label", "Mem baseline"
                            div { class: "details-value",
                                {
                                    let current = selected.physical_mem as f64 / 1000.0;
                                    let drift = current - baseline;
                                    format!("{baseline:.1} MB avg ({drift:+.1} MB drift)")
                                }
                            }
                        }
                    }
                }
            } else {
                p { class: "details-empty", "Select a row to inspect process details." }
            }
        }
    }
}
