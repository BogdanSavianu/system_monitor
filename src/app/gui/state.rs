use std::collections::HashMap;

use system_monitor::util::Pid;

use super::view_models::{NetworkRowViewModel, ProcessRowViewModel, ThreadRowViewModel};

#[derive(Debug, Clone)]
pub struct GuiState {
    pub rows: Vec<ProcessRowViewModel>,
    pub thread_rows: Vec<ThreadRowViewModel>,
    pub network_rows: Vec<NetworkRowViewModel>,
    pub cmdline_by_pid: HashMap<Pid, String>,
    pub cpu_top_history_by_pid: HashMap<Pid, Vec<f64>>,
    pub selected_pid: Option<Pid>,
    pub details_expanded: bool,
    pub filter_text: String,
    pub status_line: String,
}

impl GuiState {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            thread_rows: Vec::new(),
            network_rows: Vec::new(),
            cmdline_by_pid: HashMap::new(),
            cpu_top_history_by_pid: HashMap::new(),
            selected_pid: None,
            details_expanded: false,
            filter_text: String::new(),
            status_line: "waiting for first sample...".to_string(),
        }
    }
}
