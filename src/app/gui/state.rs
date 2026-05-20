use std::collections::{HashMap, HashSet};

use system_monitor::util::Pid;

use super::view_models::{
    NetworkRowViewModel, ProcessHierarchyNodeViewModel, ProcessRowViewModel, ThreadRowViewModel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPage {
    Monitor,
    Leaks,
    Tree,
    System,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    fn toggled(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorSortKey {
    Pid,
    Name,
    CpuTop,
    CpuRel,
    VirtualMemory,
    PhysicalMemory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaksSortKey {
    Pid,
    Name,
    Confidence,
    Anomalous,
    Total,
    Status,
}

#[derive(Debug, Clone)]
pub struct LeakConfidenceStats {
    pub pid: Pid,
    pub name: String,
    pub total_entries: u64,
    pub anomalous_entries: u64,
    pub is_active: bool,
    pub observed_entries: u64,
    pub last_seen_anomalous: bool,
}

impl LeakConfidenceStats {
    pub const WARMUP_POINTS: u64 = 24;

    pub fn new(pid: Pid, name: String) -> Self {
        Self {
            pid,
            name,
            total_entries: 0,
            anomalous_entries: 0,
            observed_entries: 0,
            is_active: false,
            last_seen_anomalous: false,
        }
    }

    pub fn is_warmup_complete(&self) -> bool {
        self.observed_entries > Self::WARMUP_POINTS
    }

    pub fn confidence(&self) -> f64 {
        if self.total_entries == 0 {
            return 0.0;
        }
        self.anomalous_entries as f64 / self.total_entries as f64
    }
}

#[derive(Debug, Clone)]
pub struct GuiState {
    pub rows: Vec<ProcessRowViewModel>,
    pub thread_rows: Vec<ThreadRowViewModel>,
    pub network_rows: Vec<NetworkRowViewModel>,
    pub cmdline_by_pid: HashMap<Pid, String>,
    pub anomaly_by_pid: HashMap<Pid, bool>,
    pub leak_stats_by_pid: HashMap<Pid, LeakConfidenceStats>,
    pub detected_leaks: Vec<LeakConfidenceStats>,
    pub cpu_top_history_by_pid: HashMap<Pid, Vec<f64>>,
    pub physical_mem_history_by_pid: HashMap<Pid, Vec<f64>>,
    pub system_cpu_history: Vec<f64>,
    pub system_mem_used_history_mb: Vec<f64>,
    pub process_tree_roots: Vec<ProcessHierarchyNodeViewModel>,
    pub process_tree_expanded: HashSet<Pid>,
    pub selected_pid: Option<Pid>,
    pub details_expanded: bool,
    pub active_page: GuiPage,
    pub monitor_sort_key: MonitorSortKey,
    pub monitor_sort_direction: SortDirection,
    pub leaks_sort_key: LeaksSortKey,
    pub leaks_sort_direction: SortDirection,
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
            anomaly_by_pid: HashMap::new(),
            leak_stats_by_pid: HashMap::new(),
            detected_leaks: Vec::new(),
            cpu_top_history_by_pid: HashMap::new(),
            physical_mem_history_by_pid: HashMap::new(),
            system_cpu_history: Vec::new(),
            system_mem_used_history_mb: Vec::new(),
            process_tree_roots: Vec::new(),
            process_tree_expanded: HashSet::new(),
            selected_pid: None,
            details_expanded: false,
            active_page: GuiPage::Monitor,
            monitor_sort_key: MonitorSortKey::Pid,
            monitor_sort_direction: SortDirection::Asc,
            leaks_sort_key: LeaksSortKey::Confidence,
            leaks_sort_direction: SortDirection::Desc,
            settings_storage_enabled: false,
            settings_anomaly_enabled: false,
            filter_text: String::new(),
            status_line: "waiting for first sample...".to_string(),
        }
    }

    pub fn toggle_monitor_sort(&mut self, key: MonitorSortKey) {
        if self.monitor_sort_key == key {
            self.monitor_sort_direction = self.monitor_sort_direction.toggled();
        } else {
            self.monitor_sort_key = key;
            self.monitor_sort_direction = SortDirection::Asc;
        }
    }

    pub fn toggle_leaks_sort(&mut self, key: LeaksSortKey) {
        if self.leaks_sort_key == key {
            self.leaks_sort_direction = self.leaks_sort_direction.toggled();
        } else {
            self.leaks_sort_key = key;
            self.leaks_sort_direction = SortDirection::Asc;
        }
    }

    pub fn toggle_tree_node(&mut self, pid: Pid) {
        if !self.process_tree_expanded.insert(pid) {
            self.process_tree_expanded.remove(&pid);
        }
    }
}
