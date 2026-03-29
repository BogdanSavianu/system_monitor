use dioxus::prelude::*;
use system_monitor::util::Pid;

use crate::app::gui::view_models::ProcessRowViewModel;

#[component]
pub fn ProcessRowView(
    row: ProcessRowViewModel,
    selected: bool,
    on_select: EventHandler<Pid>,
) -> Element {
    let physical_mem_mb = row.physical_mem / 1000;
    let virtual_mem_mb = row.virtual_mem / 1000;
    rsx! {
        tr {
            class: if selected { "selected-row" } else { "" },

            td {
                button {
                    onclick: move |_| on_select.call(row.pid),
                    "{row.pid}"
                }
            }
            td { "{row.name}" }
            td { "{row.cpu_top:.2}%" }
            td { "{row.cpu_rel:.2}%" }
            td { "{virtual_mem_mb} MB" }
            td { "{physical_mem_mb} MB" }
        }
    }
}
