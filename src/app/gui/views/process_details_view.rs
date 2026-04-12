use dioxus::prelude::*;
use plotters::prelude::*;

use crate::app::gui::view_models::{NetworkRowViewModel, ProcessRowViewModel, ThreadRowViewModel};

fn render_cpu_history_svg(cpu_top_history: &[f64]) -> Option<String> {
    if cpu_top_history.is_empty() {
        return None;
    }

    let width = 720;
    let height = 220;
    let x_max = cpu_top_history.len().saturating_sub(1).max(1);
    let y_max = cpu_top_history
        .iter()
        .copied()
        .fold(0.0_f64, f64::max)
        .max(1.0)
        * 1.10;

    let mut svg = String::new();
    {
        let backend = SVGBackend::with_string(&mut svg, (width, height));
        let root = backend.into_drawing_area();
        root.fill(&RGBColor(248, 250, 252)).ok()?;

        let mut chart = ChartBuilder::on(&root)
            .margin(10)
            .x_label_area_size(24)
            .y_label_area_size(44)
            .build_cartesian_2d(0usize..x_max, 0f64..y_max)
            .ok()?;

        chart
            .configure_mesh()
            .disable_x_mesh()
            .light_line_style(RGBColor(219, 227, 236))
            .axis_style(RGBColor(100, 116, 139))
            .y_desc("% CPU")
            .x_desc("samples")
            .label_style(
                ("sans-serif", 12)
                    .into_font()
                    .color(&RGBColor(100, 116, 139)),
            )
            .draw()
            .ok()?;

        chart
            .draw_series(LineSeries::new(
                cpu_top_history
                    .iter()
                    .enumerate()
                    .map(|(idx, sample)| (idx, *sample)),
                &RGBColor(15, 118, 110),
            ))
            .ok()?;

        chart
            .draw_series(cpu_top_history.iter().enumerate().map(|(idx, sample)| {
                Circle::new((idx, *sample), 2, RGBColor(13, 148, 136).filled())
            }))
            .ok()?;

        root.present().ok()?;
    }

    let svg = svg.replacen(
        "<svg ",
        &format!(
            "<svg viewBox=\"0 0 {width} {height}\" preserveAspectRatio=\"none\" style=\"width:100%;height:100%;display:block;\" "
        ),
        1,
    );

    Some(svg)
}

pub fn render_process_details(
    selected_row: Option<&ProcessRowViewModel>,
    selected_threads: &[&ThreadRowViewModel],
    selected_network: Option<&NetworkRowViewModel>,
    selected_cmdline: Option<&str>,
    cpu_top_history: &[f64],
    expanded: bool,
    on_toggle_expand: EventHandler<()>,
) -> Element {
    let cmdline_text = selected_cmdline.unwrap_or("(not available)");
    let max_cpu = cpu_top_history
        .iter()
        .copied()
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let cpu_history_svg = render_cpu_history_svg(&cpu_top_history);
    let has_cpu_history_svg = cpu_history_svg.is_some();
    let cpu_history_svg_markup = cpu_history_svg.unwrap_or_default();

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
                }
            } else {
                p { class: "details-empty", "Select a row to inspect process details." }
            }
        }
    }
}
