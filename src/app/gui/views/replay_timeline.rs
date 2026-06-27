use dioxus::prelude::*;

use crate::app::gui::fmt::format_bytes;
use crate::app::gui::replay::state::ReplayState;
use crate::app::gui::replay::types::ReplayScanMarker;

pub fn render_replay_timeline(
    replay: &ReplayState,
    cursor_label: &str,
    on_seek_ratio_change: EventHandler<String>,
    on_scan_marker_click: EventHandler<i64>,
) -> Element {
    let ratio_value = (replay.current_ratio() * 1000.0).round() as i64;
    let annotation_svg = build_annotation_svg(&replay.samples);

    // needs at least two samples to bracket the time range, else the ratio
    // math collapses to 0/1.
    let first_ms = replay.samples.first().map(|s| s.collected_at_ms);
    let last_ms = replay.samples.last().map(|s| s.collected_at_ms);

    rsx! {
        div {
            class: "replay-timeline",
            p {
                class: "detected-leaks-subtitle",
                "Cursor: {cursor_label}"
            }
            if let (Some(first), Some(last)) = (first_ms, last_ms) {
                if last > first {
                    div {
                        class: "replay-scan-marker-row",
                        for marker in replay.scan_markers.iter() {
                            if marker.started_at_ms >= first && marker.started_at_ms <= last {
                                {render_scan_marker(marker, first, last, replay.selected_scan_id, on_scan_marker_click)}
                            }
                        }
                    }
                }
            }
            if let Some(svg) = annotation_svg {
                div {
                    class: "replay-annotation-strip",
                    dangerous_inner_html: "{svg}",
                }
            }
            input {
                r#type: "range",
                min: "0",
                max: "1000",
                value: "{ratio_value}",
                disabled: replay.samples.is_empty(),
                oninput: move |event| on_seek_ratio_change.call(event.value()),
            }
        }
    }
}

fn render_scan_marker(
    marker: &ReplayScanMarker,
    first_ms: i64,
    last_ms: i64,
    selected_id: Option<i64>,
    on_click: EventHandler<i64>,
) -> Element {
    let span = (last_ms - first_ms).max(1) as f64;
    let ratio = ((marker.started_at_ms - first_ms) as f64 / span).clamp(0.0, 1.0);
    let left_pct = ratio * 100.0;

    // dominant leak class decides the marker colour.
    let (class_suffix, class_label) = if marker.definitely_lost_bytes
        >= marker.indirectly_lost_bytes
        && marker.definitely_lost_bytes >= marker.possibly_lost_bytes
        && marker.definitely_lost_bytes > 0
    {
        ("definitely", "definitely lost")
    } else if marker.indirectly_lost_bytes >= marker.possibly_lost_bytes
        && marker.indirectly_lost_bytes > 0
    {
        ("indirectly", "indirectly lost")
    } else if marker.possibly_lost_bytes > 0 {
        ("possibly", "possibly lost")
    } else {
        ("reachable", "reachable only")
    };
    let class_attr = if selected_id == Some(marker.id) {
        format!(
            "replay-scan-marker replay-scan-marker-{} replay-scan-marker-selected",
            class_suffix
        )
    } else {
        format!("replay-scan-marker replay-scan-marker-{}", class_suffix)
    };
    let title = format!(
        "scan #{} · PID {} · {} blocks · {} ({})",
        marker.id,
        marker.pid,
        marker.total_blocks,
        format_bytes(marker.total_lost_bytes),
        class_label
    );
    let id = marker.id;
    let style = format!("left: {:.2}%;", left_pct);

    rsx! {
        button {
            r#type: "button",
            class: "{class_attr}",
            style: "{style}",
            title: "{title}",
            onclick: move |_| on_click.call(id),
            ""
        }
    }
}

fn build_annotation_svg(
    samples: &[crate::app::gui::replay::types::ReplaySample],
) -> Option<String> {
    if samples.is_empty() {
        return None;
    }

    let n = samples.len();
    let width = 1000u32;
    let height = 12u32;
    let rect_w = (width as f64 / n as f64).max(1.0);

    let mut rects = String::new();
    for (i, sample) in samples.iter().enumerate() {
        let x = i as f64 * rect_w;
        let color = if sample.is_anomalous {
            "rgba(239,68,68,0.75)"
        } else {
            "rgba(16,185,129,0.25)"
        };
        rects.push_str(&format!(
            r#"<rect x="{x:.1}" y="0" width="{rect_w:.1}" height="{height}" fill="{color}"/>"#
        ));
    }

    Some(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" preserveAspectRatio="none" style="width:100%;height:{height}px;display:block;">{rects}</svg>"#
    ))
}
