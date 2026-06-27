use dioxus::prelude::*;
use system_monitor::util::Pid;

use crate::app::gui::view_models::ProcessRowViewModel;

fn fmt_kb_s(kb_s: f64) -> String {
    if kb_s < 0.1 {
        "-".to_string()
    } else if kb_s < 1024.0 {
        format!("{:.0} KB/s", kb_s)
    } else {
        format!("{:.1} MB/s", kb_s / 1024.0)
    }
}

pub struct ProcessRowProps<'a> {
    pub row: &'a ProcessRowViewModel,
    pub selected: bool,
    pub process_context_pid: Option<Pid>,
    pub mem_growth_rate: Option<f64>,
    pub on_select: EventHandler<Pid>,
    pub on_open_context_menu: EventHandler<Pid>,
    pub on_close_context_menu: EventHandler<()>,
    pub on_terminate_process: EventHandler<Pid>,
    pub on_kill_process: EventHandler<Pid>,
}

pub fn render_process_row(props: ProcessRowProps) -> Element {
    let ProcessRowProps {
        row,
        selected,
        process_context_pid,
        mem_growth_rate,
        on_select,
        on_open_context_menu,
        on_close_context_menu,
        on_terminate_process,
        on_kill_process,
    } = props;

    let physical_mem_mb = row.physical_mem / 1000;
    let virtual_mem_mb = row.virtual_mem / 1000;
    let growth_label = match mem_growth_rate {
        Some(r) => format!("{:+.2}", r),
        None => "-".to_string(),
    };
    let pid = row.pid;
    let name = row.name.as_str();
    let cpu_top = row.cpu_top;
    let cpu_rel = row.cpu_rel;
    let state = row.state;
    let is_anomalous = row.is_anomalous;
    let is_action_open = process_context_pid == Some(pid);
    let disk_read_label = fmt_kb_s(row.disk_read_kb_s);
    let disk_write_label = fmt_kb_s(row.disk_write_kb_s);

    rsx! {
        tr {
            class: if selected {
                "selected-row"
            } else if is_anomalous {
                "anomaly-row"
            } else {
                ""
            },

            td {
                class: "pid-cell",
                if is_anomalous {
                    span {
                        class: "hazard-indicator",
                        title: "This process is suspected to leak memory",
                        "⚠"
                    }
                }
                button {
                    onclick: move |_| on_select.call(pid),
                    "{pid}"
                }
            }
            td { "{name}" }
            td { "{state}" }
            td { "{cpu_top:.2}%" }
            td { "{cpu_rel:.2}%" }
            td { "{virtual_mem_mb} MB" }
            td { "{physical_mem_mb} MB" }
            td { "{growth_label}" }
            td { "{disk_read_label}" }
            td { "{disk_write_label}" }
            td {
                if is_action_open {
                    div {
                        class: "process-row-actions",
                        button {
                            onclick: move |_| on_terminate_process.call(pid),
                            "Close"
                        }
                        button {
                            onclick: move |_| on_kill_process.call(pid),
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
                        onclick: move |_| on_open_context_menu.call(pid),
                        "X"
                    }
                }
            }
        }
    }
}
