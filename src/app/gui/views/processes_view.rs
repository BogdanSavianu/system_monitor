use dioxus::prelude::*;
use system_monitor::util::Pid;

use crate::app::gui::components::FilterBar;
use crate::app::gui::view_models::ProcessRowViewModel;

use super::process_row_view::ProcessRowView;

#[component]
pub fn ProcessesView(
    rows: Vec<ProcessRowViewModel>,
    selected_pid: Option<Pid>,
    filter_text: String,
    on_filter_change: EventHandler<String>,
    on_select: EventHandler<Pid>,
) -> Element {
    let filter = filter_text.to_lowercase();

    rsx! {
        div {
            FilterBar {
                filter_text: filter_text,
                on_change: on_filter_change,
            }

            table {
                thead {
                    tr {
                        th { "PID" }
                        th { "Name" }
                        th { "CPU top" }
                        th { "CPU rel" }
                        th { "Virtual Memory" }
                        th { "Physical Memory" }
                    }
                }
                tbody {
                    for row in rows.iter().filter(|row| {
                        if filter.is_empty() {
                            true
                        } else {
                            row.pid.to_string().contains(&filter)
                                || row.name.to_lowercase().contains(&filter)
                        }
                    }) {
                        ProcessRowView {
                            row: row.clone(),
                            selected: selected_pid == Some(row.pid),
                            on_select: on_select.clone(),
                        }
                    }
                }
            }
        }
    }
}
