use std::collections::HashMap;

use system_monitor::util::Pid;

use super::view_models::{NetworkRowViewModel, ProcessRowViewModel, ThreadRowViewModel};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPage {
    Monitor,
    System,
    Settings,
}

#[derive(Debug, Clone)]
pub struct GuiState {
    pub rows: Vec<ProcessRowViewModel>,
    pub thread_rows: Vec<ThreadRowViewModel>,
    pub network_rows: Vec<NetworkRowViewModel>,
    pub cmdline_by_pid: HashMap<Pid, String>,
    pub cpu_top_history_by_pid: HashMap<Pid, Vec<f64>>,
    pub physical_mem_history_by_pid: HashMap<Pid, Vec<f64>>,
    pub system_cpu_history: Vec<f64>,
    pub system_mem_used_history_mb: Vec<f64>,
    pub selected_pid: Option<Pid>,
    pub details_expanded: bool,
    pub active_page: GuiPage,
    pub settings_storage_enabled: bool,
    pub settings_anomaly_enabled: bool,
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
            physical_mem_history_by_pid: HashMap::new(),
            system_cpu_history: Vec::new(),
            system_mem_used_history_mb: Vec::new(),
            selected_pid: None,
            details_expanded: false,
            active_page: GuiPage::Monitor,
            settings_storage_enabled: false,
            settings_anomaly_enabled: false,
            filter_text: String::new(),
            status_line: "waiting for first sample...".to_string(),
        }
    }
}
