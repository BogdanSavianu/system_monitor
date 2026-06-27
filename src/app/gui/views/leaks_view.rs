use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};
use system_monitor::util::Pid;

use crate::app::gui::fmt::{format_bytes, format_ts};
use crate::app::gui::state::{
    ConfidenceBand, DeepScanState, DeepScanStatus, LeakConfidenceStats, LeaksSortKey,
    ReachabilityScanState, SortDirection,
};

pub struct LeaksViewProps<'a> {
    pub detected_leaks: &'a [LeakConfidenceStats],
    pub physical_mem_history_by_pid: &'a HashMap<Pid, Vec<f64>>,
    pub dismissed_alert_pids: &'a HashSet<Pid>,
    pub process_context_pid: Option<Pid>,
    pub leaks_sort_key: LeaksSortKey,
    pub leaks_sort_direction: SortDirection,
    pub tz_offset_hours: i8,
    pub deep_scan_by_pid: &'a HashMap<Pid, DeepScanState>,
    pub reachability_by_pid: &'a HashMap<Pid, ReachabilityScanState>,
    pub on_sort_change: EventHandler<LeaksSortKey>,
    pub on_dismiss_alert: EventHandler<Pid>,
    pub on_open_context_menu: EventHandler<Pid>,
    pub on_close_context_menu: EventHandler<()>,
    pub on_terminate_process: EventHandler<Pid>,
    pub on_kill_process: EventHandler<Pid>,
    pub on_open_report: EventHandler<Pid>,
}

pub fn render_leaks_view(props: LeaksViewProps) -> Element {
    let LeaksViewProps {
        detected_leaks,
        physical_mem_history_by_pid,
        dismissed_alert_pids,
        process_context_pid,
        leaks_sort_key,
        leaks_sort_direction,
        tz_offset_hours,
        deep_scan_by_pid,
        reachability_by_pid,
        on_sort_change,
        on_dismiss_alert,
        on_open_context_menu,
        on_close_context_menu,
        on_terminate_process,
        on_kill_process,
        on_open_report,
    } = props;

    let alerts: Vec<&LeakConfidenceStats> = detected_leaks
        .iter()
        .filter(|l| {
            l.is_active
                && l.confidence_band() == ConfidenceBand::High
                && !dismissed_alert_pids.contains(&l.pid)
        })
        .collect();
    let mut sorted_leaks = detected_leaks.iter().collect::<Vec<_>>();
    sorted_leaks.sort_by(|a, b| {
        let ordering = match leaks_sort_key {
            LeaksSortKey::Pid => a.pid.cmp(&b.pid),
            LeaksSortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            LeaksSortKey::Confidence => a.confidence().total_cmp(&b.confidence()),
            LeaksSortKey::Anomalous => a.anomalous_entries.cmp(&b.anomalous_entries),
            LeaksSortKey::Total => a.total_entries.cmp(&b.total_entries),
            LeaksSortKey::Status => {
                let a_status = if a.is_active { "active" } else { "ended" };
                let b_status = if b.is_active { "active" } else { "ended" };
                a_status.cmp(b_status)
            }
        };

        if leaks_sort_direction == SortDirection::Asc {
            ordering
        } else {
            ordering.reverse()
        }
    });

    let sort_marker = |key: LeaksSortKey| {
        if leaks_sort_key == key {
            if leaks_sort_direction == SortDirection::Asc {
                " ^"
            } else {
                " v"
            }
        } else {
            ""
        }
    };

    rsx! {
        div {
            class: "list-panel",

            for leak in &alerts {
                div {
                    class: "leak-alert-banner",
                    span {
                        class: "leak-alert-text",
                        "High-confidence leak: {leak.name} (PID {leak.pid}) - {(leak.confidence() * 100.0):.0}% anomalous"
                    }
                    button {
                        class: "leak-alert-dismiss",
                        onclick: {
                            let pid = leak.pid;
                            move |_| on_dismiss_alert.call(pid)
                        },
                        "Dismiss"
                    }
                }
            }

            div {
                class: "detected-leaks-panel",
                h3 { "Detected leak processes" }
                p {
                    class: "detected-leaks-subtitle",
                    "Confidence = anomalous entries / total entries"
                }
                p {
                    class: "detected-leaks-subtitle",
                    "Lifecycle: first/last anomaly detection timestamp"
                }

                table {
                    class: "table detected-leaks-table",
                    thead {
                        tr {
                            th {
                                class: "sortable-header",
                                onclick: move |_| on_sort_change.call(LeaksSortKey::Pid),
                                "PID{sort_marker(LeaksSortKey::Pid)}"
                            }
                            th {
                                class: "sortable-header",
                                onclick: move |_| on_sort_change.call(LeaksSortKey::Name),
                                "Name{sort_marker(LeaksSortKey::Name)}"
                            }
                            th {
                                class: "sortable-header",
                                onclick: move |_| on_sort_change.call(LeaksSortKey::Confidence),
                                "Confidence{sort_marker(LeaksSortKey::Confidence)}"
                            }
                            th {
                                class: "sortable-header",
                                onclick: move |_| on_sort_change.call(LeaksSortKey::Anomalous),
                                "Anomalous{sort_marker(LeaksSortKey::Anomalous)}"
                            }
                            th {
                                class: "sortable-header",
                                onclick: move |_| on_sort_change.call(LeaksSortKey::Total),
                                "Total{sort_marker(LeaksSortKey::Total)}"
                            }
                            th {
                                class: "sortable-header",
                                onclick: move |_| on_sort_change.call(LeaksSortKey::Status),
                                "Status{sort_marker(LeaksSortKey::Status)}"
                            }
                            th { "Growth MB/min" }
                            th { "Lifecycle" }
                            th { "Analysis" }
                            th { "Action" }
                        }
                    }
                    tbody {
                        if sorted_leaks.is_empty() {
                            tr {
                                td {
                                    colspan: "10",
                                    class: "detected-leaks-empty",
                                    "No leak detections yet"
                                }
                            }
                        } else {
                            for leak in sorted_leaks {
                                tr {
                                    td { "{leak.pid}" }
                                    td { "{leak.name}" }
                                    td {
                                        if let Some(conf) = leak.confidence_visible() {
                                            "{(conf * 100.0):.1}%"
                                        } else {
                                            "n/a (warming evidence {leak.total_entries}/{LeakConfidenceStats::MIN_EVIDENCE_POINTS})"
                                        }
                                    }
                                    td { "{leak.anomalous_entries}" }
                                    td { "{leak.total_entries}" }
                                    td {
                                        if leak.is_active {
                                            span { class: "status-active", "active" }
                                        } else {
                                            span { class: "status-inactive", "ended" }
                                        }
                                    }
                                    td {
                                        {
                                            let growth = physical_mem_history_by_pid
                                                .get(&leak.pid)
                                                .and_then(|h| leak_growth_rate_mb_per_min(h));
                                            match growth {
                                                Some(r) => format!("{:+.2}", r),
                                                None => "-".to_string(),
                                            }
                                        }
                                    }
                                    td {
                                        div {
                                            class: "leak-lifecycle",
                                            div { class: "leak-lifecycle-row", "first: {format_ts(leak.first_detected_at, tz_offset_hours)}" }
                                            div { class: "leak-lifecycle-row", "last: {format_ts(leak.last_detected_at, tz_offset_hours)}" }
                                        }
                                    }
                                    td {
                                        {render_analysis_summary(
                                            leak.pid,
                                            deep_scan_by_pid.get(&leak.pid),
                                            reachability_by_pid.get(&leak.pid),
                                            on_open_report,
                                        )}
                                    }
                                    td {
                                        if leak.is_active {
                                            if process_context_pid == Some(leak.pid) {
                                                div {
                                                    class: "process-row-actions",
                                                    button {
                                                        onclick: {
                                                            let pid = leak.pid;
                                                            move |_| on_terminate_process.call(pid)
                                                        },
                                                        "Close"
                                                    }
                                                    button {
                                                        onclick: {
                                                            let pid = leak.pid;
                                                            move |_| on_kill_process.call(pid)
                                                        },
                                                        "Force close"
                                                    }
                                                    button {
                                                        onclick: move |_| on_close_context_menu.call(()),
                                                        "Cancel"
                                                    }
                                                }
                                            } else {
                                                button {
                                                    class: "process-close-btn",
                                                    onclick: {
                                                        let pid = leak.pid;
                                                        move |_| on_open_context_menu.call(pid)
                                                    },
                                                    "X"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn leak_growth_rate_mb_per_min(history: &[f64]) -> Option<f64> {
    if history.len() < 2 {
        return None;
    }
    let span_min = (history.len() - 1) as f64 * 2.0 / 60.0;
    Some((history.last().unwrap() - history.first().unwrap()) / span_min)
}

fn render_analysis_summary(
    pid: Pid,
    deep: Option<&DeepScanState>,
    reach: Option<&ReachabilityScanState>,
    on_open_report: EventHandler<Pid>,
) -> Element {
    let deep_status = deep.map(|d| d.status);
    let reach_status = reach.map(|r| r.status);
    // allocation-scan is continuous: "running" means capture is on. "off" with a report
    // still has data to show, and no scan ever finishes "done".
    let deep_capturing = deep_status == Some(DeepScanStatus::Running);
    let deep_has_data = deep.map(|d| d.report.is_some()).unwrap_or(false);
    let reach_done = reach_status == Some(DeepScanStatus::Done);

    let (pill_text, pill_class) = if deep_capturing && reach_done {
        ("alloc ▶ + reach", "leak-pill leak-pill-capturing")
    } else if deep_capturing {
        ("alloc ▶", "leak-pill leak-pill-capturing")
    } else if deep_has_data && reach_done {
        ("alloc + reach", "leak-pill leak-pill-joined")
    } else if reach_done {
        ("reachability", "leak-pill leak-pill-reachability")
    } else if deep_has_data {
        ("allocation", "leak-pill leak-pill-allocation")
    } else if reach_status == Some(DeepScanStatus::Running) {
        ("scanning…", "leak-pill leak-pill-running")
    } else if deep_status == Some(DeepScanStatus::Failed)
        || reach_status == Some(DeepScanStatus::Failed)
    {
        ("failed", "leak-pill leak-pill-failed")
    } else {
        ("no scans", "leak-pill leak-pill-empty")
    };

    let summary = analysis_summary_line(deep, reach);

    rsx! {
        div {
            class: "leak-analysis-summary",
            span { class: "{pill_class}", "{pill_text}" }
            if let Some(line) = summary {
                span { class: "leak-analysis-summary-line", "{line}" }
            }
            button {
                class: "leak-view-report-btn",
                onclick: move |_| on_open_report.call(pid),
                "View report"
            }
        }
    }
}

/// one-line summary of the most informative scan currently available.
fn analysis_summary_line(
    deep: Option<&DeepScanState>,
    reach: Option<&ReachabilityScanState>,
) -> Option<String> {
    if let Some(r) = reach.and_then(|s| s.report.as_ref()) {
        let total = r.definitely_lost.total_bytes
            + r.indirectly_lost.total_bytes
            + r.possibly_lost.total_bytes;
        let count = r.definitely_lost.block_count
            + r.indirectly_lost.block_count
            + r.possibly_lost.block_count;
        return Some(format!(
            "reachability: {} blocks lost · {}",
            count,
            format_bytes(total)
        ));
    }
    if let Some(d) = deep.and_then(|s| s.report.as_ref()) {
        return Some(format!(
            "allocation: {} · {} outstanding",
            d.verdict.as_str(),
            format_bytes(d.total_outstanding_bytes)
        ));
    }
    None
}
