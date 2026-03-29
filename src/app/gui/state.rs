use system_monitor::util::Pid;

use super::view_models::ProcessRowViewModel;

#[derive(Debug, Clone)]
pub struct GuiState {
    pub rows: Vec<ProcessRowViewModel>,
    pub selected_pid: Option<Pid>,
    pub filter_text: String,
    pub status_line: String,
}

impl GuiState {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            selected_pid: None,
            filter_text: String::new(),
            status_line: "waiting for first sample...".to_string(),
        }
    }
}
