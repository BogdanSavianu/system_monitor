use dioxus::prelude::*;

use crate::app::gui::fmt::{format_bytes, format_ts_ms};
use crate::app::gui::replay::types::{ReplayLeakedBlock, ReplayScanMarker};

const MAX_STACK_FRAMES_PER_BLOCK: usize = 8;
const MAX_BLOCKS_RENDERED: usize = 100;

#[component]
pub fn ReplayScanModal(
    scan: ReplayScanMarker,
    blocks: Vec<ReplayLeakedBlock>,
    loading: bool,
    tz_offset_hours: i8,
    on_close: EventHandler<()>,
) -> Element {
    let title_ts = format_ts_ms(scan.started_at_ms, tz_offset_hours);
    let lost_total = scan.total_lost_bytes;
    let block_count = blocks.len();

    rsx! {
        div {
            class: "leak-modal-backdrop",
            onclick: move |_| on_close.call(()),

            div {
                class: "leak-modal",
                onclick: move |e| e.stop_propagation(),

                div {
                    class: "leak-modal-header",
                    div {
                        class: "leak-modal-title",
                        span { class: "leak-modal-pid", "PID {scan.pid}" }
                        span { class: "leak-modal-name", "Scan at {title_ts}" }
                    }
                    button {
                        class: "leak-modal-close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }

                p {
                    class: "detected-leaks-subtitle",
                    "{format_bytes(lost_total)} lost across {scan.total_blocks} blocks"
                }

                if loading {
                    p { class: "detected-leaks-subtitle", "Loading leaked blocks…" }
                } else if blocks.is_empty() {
                    p { class: "detected-leaks-subtitle", "No leaked blocks recorded for this scan." }
                } else {
                    ul {
                        class: "leak-modal-attributed",
                        for block in blocks.iter().take(MAX_BLOCKS_RENDERED) {
                            {render_block(block)}
                        }
                    }
                    if block_count > MAX_BLOCKS_RENDERED {
                        p {
                            class: "detected-leaks-subtitle",
                            "Showing {MAX_BLOCKS_RENDERED} of {block_count} blocks (largest first)."
                        }
                    }
                }
            }
        }
    }
}

fn render_block(block: &ReplayLeakedBlock) -> Element {
    // db stores "definitely_lost", "indirectly_lost", "possibly_lost",
    // "still_reachable". map each to its css suffix and a readable label.
    let (class_suffix, class_label) = if block.class.starts_with("definitely") {
        ("definitely", "definitely lost")
    } else if block.class.starts_with("indirectly") {
        ("indirectly", "indirectly lost")
    } else if block.class.starts_with("possibly") {
        ("possibly", "possibly lost")
    } else {
        ("reachable", "still reachable")
    };
    let class_attr = format!("leak-modal-class leak-modal-class-{}", class_suffix);
    let addr_text = format!("{:#x}", block.addr);
    let size_text = format_bytes(block.size);

    // cap the per-block stack: a 30-frame stack across 100 blocks is ~3000
    // dom nodes per modal, which dominates open and every later diff.
    let stack_lines: Vec<&str> = block
        .stack_text
        .as_deref()
        .map(|s| s.lines().take(MAX_STACK_FRAMES_PER_BLOCK).collect())
        .unwrap_or_default();
    let truncated_frames = block
        .stack_text
        .as_deref()
        .map(|s| s.lines().count())
        .unwrap_or(0)
        .saturating_sub(stack_lines.len());

    rsx! {
        li {
            class: "leak-modal-block",
            div {
                class: "leak-modal-block-head",
                span { class: "{class_attr}", "{class_label}" }
                span { class: "leak-modal-block-size", "{size_text}" }
                span { class: "leak-modal-block-addr", "at {addr_text}" }
            }
            if block.stack_text.is_some() {
                ul {
                    class: "leak-modal-stack",
                    for line in &stack_lines {
                        li { class: "leak-modal-stack-frame", "{line}" }
                    }
                    if truncated_frames > 0 {
                        li {
                            class: "leak-modal-stack-frame leak-modal-no-source",
                            "… {truncated_frames} more frame(s)"
                        }
                    }
                }
            } else {
                p { class: "leak-modal-no-source", "no allocation source recorded" }
            }
        }
    }
}
