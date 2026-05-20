use dioxus::prelude::*;

use crate::app::gui::state::{LeakConfidenceStats, LeaksSortKey, SortDirection};

pub fn render_leaks_view(
    detected_leaks: &[LeakConfidenceStats],
    leaks_sort_key: LeaksSortKey,
    leaks_sort_direction: SortDirection,
    on_sort_change: EventHandler<LeaksSortKey>,
) -> Element {
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
            div {
                class: "detected-leaks-panel",
                h3 { "Detected leak processes" }
                p {
                    class: "detected-leaks-subtitle",
                    "Confidence = anomalous entries / total entries"
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
                        }
                    }
                    tbody {
                        if sorted_leaks.is_empty() {
                            tr {
                                td {
                                    colspan: "6",
                                    class: "detected-leaks-empty",
                                    "No leak detections yet"
                                }
                            }
                        } else {
                            for leak in sorted_leaks {
                                tr {
                                    td { "{leak.pid}" }
                                    td { "{leak.name}" }
                                    td { "{(leak.confidence() * 100.0):.1}%" }
                                    td { "{leak.anomalous_entries}" }
                                    td { "{leak.total_entries}" }
                                    td {
                                        if leak.is_active {
                                            span { class: "status-active", "active" }
                                        } else {
                                            span { class: "status-inactive", "ended" }
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
