use dioxus::prelude::*;
use system_monitor::leakprobe::{
    AllocScanReport, AttributedBlock, BlockClass, ReachabilityReport, attribute_leaks,
};
use system_monitor::util::Pid;

use crate::app::gui::fmt::format_bytes;
use crate::app::gui::state::{DeepScanState, DeepScanStatus, ReachabilityScanState};

/// full per-process leak report modal. renders whatever allocation-scan and reachability-scan data
/// the process has, plus their joined view when both are present.
pub struct LeakReportModalProps {
    pub pid: Pid,
    pub name: String,
    pub deep: Option<DeepScanState>,
    pub reach: Option<ReachabilityScanState>,
    pub deep_available: bool,
    pub reach_available: bool,
    pub group_by_line: bool,
    pub on_toggle_group: EventHandler<()>,
    pub on_close: EventHandler<()>,
    pub on_start_continuous: EventHandler<Pid>,
    pub on_stop_continuous: EventHandler<Pid>,
    pub on_reachability_scan: EventHandler<Pid>,
}

pub fn render_leak_report_modal(props: LeakReportModalProps) -> Element {
    let LeakReportModalProps {
        pid,
        name,
        deep,
        reach,
        deep_available,
        reach_available,
        group_by_line,
        on_toggle_group,
        on_close,
        on_start_continuous,
        on_stop_continuous,
        on_reachability_scan,
    } = props;

    let deep_report = deep.as_ref().and_then(|d| d.report.as_ref()).cloned();
    let reach_report = reach.as_ref().and_then(|r| r.report.as_ref()).cloned();
    let deep_status = deep.as_ref().map(|d| d.status);
    let reach_status = reach.as_ref().map(|r| r.status);
    let deep_error = deep.as_ref().and_then(|d| d.error.clone());
    let reach_error = reach.as_ref().and_then(|r| r.error.clone());
    let deep_started_at = deep.as_ref().and_then(|d| d.started_at);
    let deep_snapshot_count = deep.as_ref().map(|d| d.snapshot_count).unwrap_or(0);

    let attributed = match (reach_report.as_ref(), deep_report.as_ref()) {
        (Some(r), d) => Some(attribute_leaks(r, d)),
        _ => None,
    };

    let deep_running = deep_status == Some(DeepScanStatus::Running);
    let reach_running = reach_status == Some(DeepScanStatus::Running);
    // toggle label keyed on allocation-scan state. reachability runs whenever the user
    // wants it, never blocked by a live capture.
    let deep_btn_label = if deep_running {
        "Stop allocation-scan capture"
    } else if deep_status == Some(DeepScanStatus::Failed) {
        "Restart allocation-scan capture"
    } else {
        "Start allocation-scan capture"
    };
    let reach_btn_label = match reach_status {
        Some(DeepScanStatus::Running) => "Scanning…",
        Some(DeepScanStatus::Done) => "Rescan",
        Some(DeepScanStatus::Failed) => "Retry",
        _ => "Run reachability scan",
    };
    let deep_btn_disabled = !deep_available;
    let reach_btn_disabled = !reach_available || reach_running;
    let deep_capture_subtitle = deep_running.then(|| {
        let elapsed = deep_started_at
            .and_then(|t| std::time::SystemTime::now().duration_since(t).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let outstanding_blocks = deep_report
            .as_ref()
            .map(|r| r.alloc_stacks.len())
            .unwrap_or(0);
        format!(
            "Capturing for {} · {} snapshots · {} live allocations tracked",
            format_elapsed(elapsed),
            deep_snapshot_count,
            outstanding_blocks
        )
    });

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
                        span { class: "leak-modal-pid", "PID {pid}" }
                        span { class: "leak-modal-name", "{name}" }
                    }
                    button {
                        class: "leak-modal-close",
                        onclick: move |_| on_close.call(()),
                        "×"
                    }
                }

                div {
                    class: "leak-modal-actions",
                    button {
                        class: if deep_running { "deep-scan-btn deep-scan-btn-active" } else { "deep-scan-btn" },
                        disabled: deep_btn_disabled,
                        title: if !deep_available {
                            "requires root / CAP_BPF and bpftrace"
                        } else {
                            ""
                        },
                        onclick: move |_| {
                            if deep_running {
                                on_stop_continuous.call(pid);
                            } else {
                                on_start_continuous.call(pid);
                            }
                        },
                        "{deep_btn_label}"
                    }
                    button {
                        class: "reachability-btn",
                        disabled: reach_btn_disabled,
                        title: if !reach_available {
                            "requires root + gcore (gdb package)"
                        } else {
                            ""
                        },
                        onclick: move |_| on_reachability_scan.call(pid),
                        "{reach_btn_label}"
                    }
                }

                if let Some(line) = deep_capture_subtitle {
                    p {
                        class: "detected-leaks-subtitle leak-modal-capture-sub",
                        "{line}"
                    }
                }

                {render_errors(deep_error.as_deref(), reach_error.as_deref())}
                {render_allocation_section(deep_report.as_ref())}
                {render_reachability_section(reach_report.as_ref())}
                {render_attributed_section(attributed.as_deref(), deep_report.is_some(), group_by_line, on_toggle_group)}
            }
        }
    }
}

fn render_errors(deep_err: Option<&str>, reach_err: Option<&str>) -> Element {
    if deep_err.is_none() && reach_err.is_none() {
        return rsx! { div {} };
    }
    rsx! {
        div {
            class: "leak-modal-section leak-modal-errors",
            if let Some(e) = deep_err {
                div { class: "deep-scan-failed", "Deep scan: {e}" }
            }
            if let Some(e) = reach_err {
                div { class: "deep-scan-failed", "Reachability scan: {e}" }
            }
        }
    }
}

fn render_allocation_section(report: Option<&AllocScanReport>) -> Element {
    let Some(report) = report else {
        return rsx! { div {} };
    };
    let verdict = report.verdict.as_str().to_string();
    let total = format_bytes(report.total_outstanding_bytes);
    let sites = report.sites.clone();
    rsx! {
        section {
            class: "leak-modal-section",
            h3 { class: "leak-modal-section-title", "Allocation scan" }
            div {
                class: "deep-scan-result",
                div {
                    class: "deep-scan-verdict deep-scan-verdict-{verdict}",
                    "{verdict} · {total} outstanding"
                }
                ul {
                    class: "deep-scan-sites",
                    for site in sites.iter().take(8) {
                        li {
                            class: "deep-scan-site",
                            span { class: "deep-scan-site-bytes", "{format_bytes(site.outstanding_bytes)}" }
                            span { class: "deep-scan-site-growth", " ({site.growth_bytes_per_s:+.0} B/s, {site.outstanding_count} allocs)" }
                            span { class: "deep-scan-site-frame", " {top_frame(&site.stack)}" }
                        }
                    }
                }
            }
        }
    }
}

fn render_reachability_section(report: Option<&ReachabilityReport>) -> Element {
    let Some(report) = report else {
        return rsx! { div {} };
    };
    let def_count = report.definitely_lost.block_count;
    let def_bytes = format_bytes(report.definitely_lost.total_bytes);
    let pos_count = report.possibly_lost.block_count;
    let pos_bytes = format_bytes(report.possibly_lost.total_bytes);
    let ind_count = report.indirectly_lost.block_count;
    let ind_bytes = format_bytes(report.indirectly_lost.total_bytes);
    let reach_bytes = format_bytes(report.still_reachable.total_bytes);
    let total_blocks = report.total_blocks;
    let allocator = report.allocator.clone();
    rsx! {
        section {
            class: "leak-modal-section",
            h3 { class: "leak-modal-section-title", "Reachability scan" }
            p {
                class: "detected-leaks-subtitle",
                "Allocator: {allocator} · {total_blocks} live blocks"
            }
            ul {
                class: "leak-modal-reach-list",
                li {
                    span { class: "deep-scan-verdict deep-scan-verdict-leaking", "Definitely lost:" }
                    " {def_count} blocks · {def_bytes}"
                }
                if pos_count > 0 {
                    li {
                        span { class: "deep-scan-verdict deep-scan-verdict-inconclusive", "Possibly lost:" }
                        " {pos_count} blocks · {pos_bytes}"
                    }
                }
                if ind_count > 0 {
                    li {
                        span { class: "deep-scan-verdict deep-scan-verdict-inconclusive", "Indirectly lost:" }
                        " {ind_count} blocks · {ind_bytes}"
                    }
                }
                li {
                    span { class: "deep-scan-verdict deep-scan-verdict-reclaimed", "Still reachable:" }
                    " {reach_bytes}"
                }
            }
        }
    }
}

fn render_attributed_section(
    attributed: Option<&[AttributedBlock]>,
    have_allocation: bool,
    group_by_line: bool,
    on_toggle_group: EventHandler<()>,
) -> Element {
    let Some(blocks) = attributed else {
        return rsx! { div {} };
    };
    if blocks.is_empty() {
        return rsx! { div {} };
    }

    // split into blocks we have a stack for and blocks we don't. the second
    // group is summarised, since a long list of identical "no source" rows adds
    // nothing
    let with_stack: Vec<&AttributedBlock> = blocks.iter().filter(|b| b.stack.is_some()).collect();
    let without_stack: Vec<&AttributedBlock> =
        blocks.iter().filter(|b| b.stack.is_none()).collect();
    let unattributed_bytes: u64 = without_stack.iter().map(|b| b.size).sum();
    let unattributed_count = without_stack.len();

    rsx! {
        section {
            class: "leak-modal-section",
            div {
                class: "leak-modal-section-head",
                h3 { class: "leak-modal-section-title", "Attributed leaks" }
                label {
                    class: "leak-modal-group-toggle",
                    title: "Aggregate leaked blocks by allocation source line (debug builds) or by function symbol (non-debug, non-stripped).",
                    input {
                        r#type: "checkbox",
                        checked: group_by_line,
                        onchange: move |_| on_toggle_group.call(()),
                    }
                    " Group by source line"
                }
            }
            if !have_allocation {
                p {
                    class: "detected-leaks-subtitle",
                    "Run a deep scan to attach allocation stacks. Blocks below are listed without source."
                }
            } else if with_stack.is_empty() {
                p {
                    class: "detected-leaks-subtitle",
                    "No leaked blocks matched the allocation-scan capture window - every block shown was allocated before the deep scan started."
                }
            } else if group_by_line {
                p {
                    class: "detected-leaks-subtitle",
                    "{with_stack.len()} attributed blocks grouped by allocation source line, or by function symbol when no source line is available."
                }
            } else {
                p {
                    class: "detected-leaks-subtitle",
                    "{with_stack.len()} of {blocks.len()} top blocks had an allocation stack from allocation-scan below."
                }
            }
            ul {
                class: "leak-modal-attributed",
                if group_by_line {
                    {render_grouped_by_line(&with_stack)}
                } else {
                    for block in with_stack.iter().take(40) {
                        li {
                            class: "leak-modal-block",
                            div {
                                class: "leak-modal-block-head",
                                span {
                                    class: "leak-modal-class leak-modal-class-{class_slug(block.class)}",
                                    "{class_label(block.class)}"
                                }
                                span { class: "leak-modal-block-size", "{format_bytes(block.size)}" }
                                span { class: "leak-modal-block-addr", " @ {block.addr:#x}" }
                            }
                            if let Some(frames) = &block.stack {
                                ul {
                                    class: "leak-modal-stack",
                                    for frame in frames {
                                        li { class: "leak-modal-stack-frame", "{frame}" }
                                    }
                                }
                            }
                        }
                    }
                }
                if unattributed_count > 0 {
                    li {
                        class: "leak-modal-block leak-modal-block-coalesced",
                        div {
                            class: "leak-modal-block-head",
                            span {
                                class: "leak-modal-class leak-modal-class-definitely",
                                "no source"
                            }
                            span { class: "leak-modal-block-size", "{format_bytes(unattributed_bytes)}" }
                            span { class: "leak-modal-block-addr", " across {unattributed_count} blocks" }
                        }
                        div {
                            class: "leak-modal-no-source",
                            "allocated before the deep scan started - allocation-scan did not capture these"
                        }
                    }
                }
            }
        }
    }
}

struct LineGroup {
    /// `Some("file.c:34")` for resolved sites, `None` for blocks whose stack had
    /// no source line (a non-debug build, or only libc frames resolved)
    key: Option<String>,
    bytes: u64,
    count: u64,
    /// most severe block class seen on this line, for the colour pill
    class: BlockClass,
    /// a representative full stack, shown collapsed under the line
    sample_stack: Vec<String>,
}

/// aggregates attributed blocks by their allocation source line, summing bytes
/// and counts, sorted by total bytes descending
fn render_grouped_by_line(with_stack: &[&AttributedBlock]) -> Element {
    use std::collections::HashMap;

    let mut order: Vec<Option<String>> = Vec::new();
    let mut groups: HashMap<Option<String>, LineGroup> = HashMap::new();

    for block in with_stack {
        let stack = block.stack.clone().unwrap_or_default();
        let key = group_key(&stack);
        let entry = groups.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            LineGroup {
                key: key.clone(),
                bytes: 0,
                count: 0,
                class: BlockClass::StillReachable,
                sample_stack: stack.clone(),
            }
        });
        entry.bytes += block.size;
        entry.count += 1;
        if class_rank(block.class) > class_rank(entry.class) {
            entry.class = block.class;
        }
    }

    let mut rows: Vec<LineGroup> = order
        .into_iter()
        .filter_map(|k| groups.remove(&k))
        .collect();
    rows.sort_by(|a, b| b.bytes.cmp(&a.bytes));

    rsx! {
        for group in rows {
            li {
                class: "leak-modal-block",
                div {
                    class: "leak-modal-block-head",
                    span {
                        class: "leak-modal-class leak-modal-class-{class_slug(group.class)}",
                        "{class_label(group.class)}"
                    }
                    span { class: "leak-modal-block-size", "{format_bytes(group.bytes)}" }
                    span {
                        class: "leak-modal-block-addr",
                        if let Some(line) = &group.key {
                            " at {line} · {group.count} blocks"
                        } else {
                            " no source line or symbol (stripped binary) · {group.count} blocks"
                        }
                    }
                }
                ul {
                    class: "leak-modal-stack",
                    for frame in group.sample_stack.iter() {
                        li { class: "leak-modal-stack-frame", "{frame}" }
                    }
                }
            }
        }
    }
}

/// the key a leaked block is grouped under. prefers the innermost resolved
/// user source line (`file:line`), which only exists for debug builds; for a
/// non-debug but non-stripped binary it falls back to the innermost non-libc
/// function symbol. fully stripped binaries yield `None`.
fn group_key(stack: &[String]) -> Option<String> {
    // 1. a resolved source line: the symbolizer appends `  →  file:line` to frames it resolved
    if let Some(line) = stack.iter().find_map(|f| {
        let idx = f.find('→')?;
        let after = f[idx + '→'.len_utf8()..].trim();
        if after.is_empty() || after.contains("libc") {
            return None;
        }
        Some(after.to_string())
    }) {
        return Some(line);
    }

    // 2. fall back to the innermost non-libc function symbol from `.symtab`
    stack.iter().find_map(|f| {
        if f.contains("libc") {
            return None;
        }
        frame_symbol(f)
    })
}

/// extracts the function name from a frame like `sym+0x12 [binary]`, dropping any
/// `+offset`, `[object]` annotation and `  →  …` suffix
fn frame_symbol(frame: &str) -> Option<String> {
    let head = frame.split('→').next().unwrap_or(frame).trim();
    if head.is_empty() || head.starts_with("0x") {
        return None;
    }
    let name = head
        .split('+')
        .next()
        .unwrap_or(head)
        .split(" [")
        .next()
        .unwrap_or(head)
        .trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

fn class_rank(c: BlockClass) -> u8 {
    match c {
        BlockClass::DefinitelyLost => 3,
        BlockClass::IndirectlyLost => 2,
        BlockClass::PossiblyLost => 1,
        BlockClass::StillReachable => 0,
    }
}

fn class_label(c: BlockClass) -> &'static str {
    match c {
        BlockClass::DefinitelyLost => "definitely lost",
        BlockClass::IndirectlyLost => "indirectly lost",
        BlockClass::PossiblyLost => "possibly lost",
        BlockClass::StillReachable => "still reachable",
    }
}

fn class_slug(c: BlockClass) -> &'static str {
    match c {
        BlockClass::DefinitelyLost => "definitely",
        BlockClass::IndirectlyLost => "indirectly",
        BlockClass::PossiblyLost => "possibly",
        BlockClass::StillReachable => "reachable",
    }
}

fn top_frame(stack: &[String]) -> String {
    stack
        .iter()
        .find(|f| !f.starts_with("0x") && !f.contains("libc"))
        .or_else(|| stack.iter().find(|f| !f.starts_with("0x")))
        .or_else(|| stack.first())
        .cloned()
        .unwrap_or_else(|| "(no stack)".to_string())
}

fn format_elapsed(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}
