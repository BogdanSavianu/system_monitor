use dioxus::prelude::*;
use plotters::prelude::*;

use super::render_line_chart_svg;

pub fn render_system_view(
    system_cpu_history: &[f64],
    system_mem_used_history_mb: &[f64],
) -> Element {
    let cpu_peak = system_cpu_history
        .iter()
        .copied()
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let mem_peak = system_mem_used_history_mb
        .iter()
        .copied()
        .fold(0.0_f64, f64::max)
        .max(1.0);

    let cpu_now = system_cpu_history.last().copied().unwrap_or_default();
    let mem_now = system_mem_used_history_mb
        .last()
        .copied()
        .unwrap_or_default();

    let cpu_svg = render_line_chart_svg(
        system_cpu_history,
        "% CPU",
        RGBColor(15, 118, 110),
        RGBColor(13, 148, 136),
    );
    let mem_svg = render_line_chart_svg(
        system_mem_used_history_mb,
        "MB",
        RGBColor(2, 132, 199),
        RGBColor(14, 165, 233),
    );

    rsx! {
        div {
            class: "system-grid",

            section {
                class: "list-panel",
                h2 { "Total CPU" }
                p { class: "details-subtitle", "Current: {cpu_now:.2}% | Peak: {cpu_peak:.2}%" }
                if let Some(svg) = cpu_svg {
                    div {
                        class: "graph-line-wrap",
                        div {
                            class: "graph-svg-host",
                            dangerous_inner_html: "{svg}",
                        }
                    }
                } else {
                    p { class: "details-empty", "Not enough samples yet to draw history." }
                }
            }

            section {
                class: "list-panel",
                h2 { "Total memory used" }
                p { class: "details-subtitle", "Current: {mem_now:.2} MB | Peak: {mem_peak:.2} MB" }
                if let Some(svg) = mem_svg {
                    div {
                        class: "graph-line-wrap",
                        div {
                            class: "graph-svg-host",
                            dangerous_inner_html: "{svg}",
                        }
                    }
                } else {
                    p { class: "details-empty", "Not enough samples yet to draw history." }
                }
            }
        }
    }
}
