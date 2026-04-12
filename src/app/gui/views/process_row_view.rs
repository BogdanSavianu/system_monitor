use dioxus::prelude::*;
use system_monitor::util::Pid;

use crate::app::gui::view_models::ProcessRowViewModel;

pub fn render_process_row(
    row: &ProcessRowViewModel,
    selected: bool,
    on_select: EventHandler<Pid>,
) -> Element {
    let physical_mem_mb = row.physical_mem / 1000;
    let virtual_mem_mb = row.virtual_mem / 1000;
    let pid = row.pid;
    let name = row.name.as_str();
    let cpu_top = row.cpu_top;
    let cpu_rel = row.cpu_rel;

    rsx! {
        tr {
            class: if selected { "selected-row" } else { "" },

            td {
                button {
                    onclick: move |_| on_select.call(pid),
                    "{pid}"
                }
            }
            td { "{name}" }
            td { "{cpu_top:.2}%" }
            td { "{cpu_rel:.2}%" }
            td { "{virtual_mem_mb} MB" }
            td { "{physical_mem_mb} MB" }
        }
    }
}
