use dioxus::prelude::*;
use plotters::prelude::*;

use crate::app::gui::views::render_time_axis_chart_svg;

pub fn render_replay_chart(
    memory_points: &[(i64, f64, bool)],
    cpu_points: &[(i64, f64, bool)],
    allocation_points: &[(i64, f64, bool)],
    tz_offset_hours: i8,
) -> Element {
    let memory_svg = render_time_axis_chart_svg(
        memory_points,
        "RSS MB",
        RGBColor(5, 150, 105),
        RGBColor(16, 185, 129),
        tz_offset_hours,
    );
    let cpu_svg = if cpu_points.is_empty() {
        None
    } else {
        render_time_axis_chart_svg(
            cpu_points,
            "CPU %",
            RGBColor(37, 99, 235),
            RGBColor(59, 130, 246),
            tz_offset_hours,
        )
    };
    let allocation_svg = if allocation_points.is_empty() {
        None
    } else {
        render_time_axis_chart_svg(
            allocation_points,
            "outstanding MB (allocation)",
            RGBColor(234, 88, 12),
            RGBColor(249, 115, 22),
            tz_offset_hours,
        )
    };

    rsx! {
        section {
            class: "list-panel",
            h2 { "Replay memory timeline" }
            if let Some(svg) = memory_svg {
                div {
                    class: "graph-line-wrap",
                    div {
                        class: "graph-svg-host",
                        dangerous_inner_html: "{svg}",
                    }
                }
            } else {
                p { class: "details-empty", "No replay data visible at current cursor." }
            }

            if let Some(svg) = cpu_svg {
                h3 { class: "replay-chart-subtitle", "CPU history" }
                div {
                    class: "graph-line-wrap",
                    div {
                        class: "graph-svg-host",
                        dangerous_inner_html: "{svg}",
                    }
                }
            }

            if let Some(svg) = allocation_svg {
                h3 { class: "replay-chart-subtitle", "allocation-scan outstanding bytes" }
                div {
                    class: "graph-line-wrap",
                    div {
                        class: "graph-svg-host",
                        dangerous_inner_html: "{svg}",
                    }
                }
                p {
                    class: "detected-leaks-subtitle",
                    "Sparse series - one sample per minute of continuous allocation-scan capture."
                }
            }
        }
    }
}
