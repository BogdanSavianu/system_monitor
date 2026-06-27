use dioxus::prelude::*;
use plotters::prelude::*;

use super::render_line_chart_svg;

pub fn render_system_view(
    system_cpu_history: &[f64],
    system_mem_used_history_mb: &[f64],
    load_avg: (f64, f64, f64),
    num_cores: u8,
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

    let (la1, la5, la15) = load_avg;
    let cores = num_cores.max(1) as f64;
    // load per core: >1.0 means overloaded, <1.0 means headroom
    let pct1 = la1 / cores * 100.0;
    let pct5 = la5 / cores * 100.0;
    let pct15 = la15 / cores * 100.0;

    let color = |pct: f64| -> &'static str {
        if pct >= 100.0 {
            "load-high"
        } else if pct >= 70.0 {
            "load-med"
        } else {
            "load-ok"
        }
    };
    let c1 = color(pct1);
    let c5 = color(pct5);
    let c15 = color(pct15);

    rsx! {
        div {
            class: "system-grid",

            section {
                class: "list-panel",
                h2 { "Load average  ·  {num_cores} cores" }
                div {
                    class: "details-grid",
                    div { class: "details-label", "1 min"
                        div { class: "details-value {c1}", "{la1:.2} / {num_cores} = {pct1:.0}%" }
                    }
                    div { class: "details-label", "5 min"
                        div { class: "details-value {c5}", "{la5:.2} / {num_cores} = {pct5:.0}%" }
                    }
                    div { class: "details-label", "15 min"
                        div { class: "details-value {c15}", "{la15:.2} / {num_cores} = {pct15:.0}%" }
                    }
                }
                p { class: "details-subtitle", "avg. processes in run queue ÷ cores" }
                p { class: "details-subtitle", "above 100% means some processes are waiting" }
            }

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
