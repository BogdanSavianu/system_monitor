use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

use system_monitor::leakprobe::{AllocScanReport, ReachabilityReport};
use system_monitor::util::Pid;

use super::replay::state::ReplayState;
use super::view_models::{
    NetworkRowViewModel, ProcessHierarchyNodeViewModel, ProcessRowViewModel, ThreadRowViewModel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuiPage {
    Monitor,
    Leaks,
    Replay,
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
    DiskRead,
    DiskWrite,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfidenceBand {
    Unknown,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TriageSeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepScanStatus {
    Off,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
pub struct DeepScanState {
    pub status: DeepScanStatus,
    pub report: Option<AllocScanReport>,
    pub error: Option<String>,
    pub started_at: Option<SystemTime>,
    pub snapshot_count: usize,
}

#[derive(Debug, Clone)]
pub struct ReachabilityScanState {
    pub status: DeepScanStatus,
    pub report: Option<ReachabilityReport>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DriftStatus {
    pub cpu_drift_z: f64,
    pub mem_drift_z: f64,
    pub is_alert: bool,
    pub summary: String,
}

impl DriftStatus {
    pub fn new() -> Self {
        Self {
            cpu_drift_z: 0.0,
            mem_drift_z: 0.0,
            is_alert: false,
            summary: "drift baseline warming up".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LeakConfidenceStats {
    pub pid: Pid,
    pub name: String,
    pub identity_key: String,
    pub total_entries: u64,
    pub anomalous_entries: u64,
    pub is_active: bool,
    pub observed_entries: u64,
    pub last_seen_anomalous: bool,
    pub first_seen_cycle: u64,
    pub last_seen_cycle: u64,
    pub first_detected_at: Option<SystemTime>,
    pub last_detected_at: Option<SystemTime>,
    pub anomalous_streak: u32,
    pub is_pinned: bool,
    pub ignored_for_session: bool,
    pub ignored_persistent: bool,
    pub why_flagged: String,
}

impl LeakConfidenceStats {
    pub const WARMUP_POINTS: u64 = 24;
    pub const MIN_EVIDENCE_POINTS: u64 = 10;

    pub fn new(pid: Pid, name: String, first_seen_cycle: u64) -> Self {
        Self {
            pid,
            name,
            identity_key: String::new(),
            total_entries: 0,
            anomalous_entries: 0,
            observed_entries: 0,
            is_active: false,
            anomalous_streak: 0,
            last_seen_anomalous: false,
            first_seen_cycle,
            last_seen_cycle: first_seen_cycle,
            first_detected_at: None,
            last_detected_at: None,
            is_pinned: false,
            ignored_for_session: false,
            ignored_persistent: false,
            why_flagged: String::new(),
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

    pub fn confidence_visible(&self) -> Option<f64> {
        if self.total_entries < Self::MIN_EVIDENCE_POINTS {
            return None;
        }
        Some(self.confidence())
    }

    pub fn confidence_band(&self) -> ConfidenceBand {
        let Some(conf) = self.confidence_visible() else {
            return ConfidenceBand::Unknown;
        };

        if conf >= 0.7 {
            ConfidenceBand::High
        } else if conf >= 0.4 {
            ConfidenceBand::Medium
        } else {
            ConfidenceBand::Low
        }
    }

    pub fn triage_severity(&self) -> TriageSeverity {
        match self.confidence_band() {
            ConfidenceBand::High => TriageSeverity::High,
            ConfidenceBand::Medium => TriageSeverity::Medium,
            _ => TriageSeverity::Low,
        }
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
    pub tree_filter_text: String,
    pub tree_focus_pid: Option<Pid>,
    pub drift_status: DriftStatus,
    pub sample_cycle: u64,
    pub process_context_pid: Option<Pid>,
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
    pub replay: ReplayState,
    pub status_line: String,
    pub dismissed_alert_pids: HashSet<Pid>,
    pub load_avg: (f64, f64, f64),
    pub num_cores: u8,
    pub username_by_pid: HashMap<Pid, String>,
    pub restart_count_by_name: HashMap<String, u32>,
    pub last_pid_by_name: HashMap<String, Pid>,
    pub memory_baseline_by_name: HashMap<String, f64>,
    pub tz_offset_hours: i8,
    pub subtree_filter_pids: Option<HashSet<Pid>>,
    pub deep_scan_available: bool,
    pub deep_scan_by_pid: HashMap<Pid, DeepScanState>,
    pub reachability_available: bool,
    pub reachability_by_pid: HashMap<Pid, ReachabilityScanState>,
    pub leak_report_open_pid: Option<Pid>,
    /// when set, the leak report aggregates leaked blocks by their source line
    /// instead of listing them individually. only meaningful for debug builds
    pub leak_report_group_by_line: bool,
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
            tree_filter_text: String::new(),
            tree_focus_pid: None,
            drift_status: DriftStatus::new(),
            sample_cycle: 0,
            process_context_pid: None,
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
            replay: ReplayState::new(),
            status_line: "waiting for first sample...".to_string(),
            dismissed_alert_pids: HashSet::new(),
            load_avg: (0.0, 0.0, 0.0),
            num_cores: 1,
            username_by_pid: HashMap::new(),
            restart_count_by_name: HashMap::new(),
            last_pid_by_name: HashMap::new(),
            memory_baseline_by_name: HashMap::new(),
            tz_offset_hours: 2,
            subtree_filter_pids: None,
            deep_scan_available: false,
            deep_scan_by_pid: HashMap::new(),
            reachability_available: false,
            reachability_by_pid: HashMap::new(),
            leak_report_open_pid: None,
            leak_report_group_by_line: false,
        }
    }

    pub fn open_leak_report(&mut self, pid: Pid) {
        self.leak_report_open_pid = Some(pid);
    }

    pub fn toggle_leak_report_grouping(&mut self) {
        self.leak_report_group_by_line = !self.leak_report_group_by_line;
    }

    pub fn close_leak_report(&mut self) {
        self.leak_report_open_pid = None;
    }

    pub fn begin_reachability_scan(&mut self, pid: Pid) {
        self.reachability_by_pid.insert(
            pid,
            ReachabilityScanState {
                status: DeepScanStatus::Running,
                report: None,
                error: None,
            },
        );
    }

    pub fn complete_reachability_scan(&mut self, report: ReachabilityReport) {
        let entry = self
            .reachability_by_pid
            .entry(report.pid)
            .or_insert_with(|| ReachabilityScanState {
                status: DeepScanStatus::Running,
                report: None,
                error: None,
            });
        entry.status = DeepScanStatus::Done;
        entry.error = None;
        entry.report = Some(report);
    }

    pub fn fail_reachability_scan(&mut self, pid: Pid, message: String) {
        let entry = self
            .reachability_by_pid
            .entry(pid)
            .or_insert_with(|| ReachabilityScanState {
                status: DeepScanStatus::Running,
                report: None,
                error: None,
            });
        entry.status = DeepScanStatus::Failed;
        entry.error = Some(message);
    }

    pub fn begin_deep_scan(&mut self, pid: Pid) {
        self.deep_scan_by_pid.insert(
            pid,
            DeepScanState {
                status: DeepScanStatus::Running,
                report: None,
                error: None,
                started_at: Some(SystemTime::now()),
                snapshot_count: 0,
            },
        );
    }

    pub fn update_deep_scan(&mut self, pid: Pid, snapshot_count: usize, report: AllocScanReport) {
        let entry = self
            .deep_scan_by_pid
            .entry(pid)
            .or_insert_with(|| DeepScanState {
                status: DeepScanStatus::Running,
                report: None,
                error: None,
                started_at: Some(SystemTime::now()),
                snapshot_count: 0,
            });
        entry.status = DeepScanStatus::Running;
        entry.error = None;
        entry.snapshot_count = snapshot_count;
        entry.report = Some(report);
    }

    pub fn stop_deep_scan(&mut self, pid: Pid) {
        if let Some(entry) = self.deep_scan_by_pid.get_mut(&pid) {
            entry.status = DeepScanStatus::Off;
        }
    }

    pub fn fail_deep_scan(&mut self, pid: Pid, message: String) {
        let entry = self
            .deep_scan_by_pid
            .entry(pid)
            .or_insert_with(|| DeepScanState {
                status: DeepScanStatus::Failed,
                report: None,
                error: None,
                started_at: None,
                snapshot_count: 0,
            });
        entry.status = DeepScanStatus::Failed;
        entry.error = Some(message);
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

    pub fn set_tree_filter_text(&mut self, text: String) {
        self.tree_filter_text = text;
    }

    pub fn set_tree_focus_pid(&mut self, pid: Option<Pid>) {
        self.tree_focus_pid = pid;
    }

    pub fn collapse_all_tree_nodes(&mut self) {
        self.process_tree_expanded.clear();
    }

    pub fn expand_all_tree_nodes(&mut self) {
        fn collect(nodes: &[ProcessHierarchyNodeViewModel], out: &mut HashSet<Pid>) {
            for node in nodes {
                out.insert(node.pid);
                collect(&node.children, out);
            }
        }

        let mut expanded = HashSet::new();
        collect(&self.process_tree_roots, &mut expanded);
        self.process_tree_expanded = expanded;
    }

    pub fn open_process_context(&mut self, pid: Pid) {
        self.process_context_pid = Some(pid);
    }

    pub fn close_process_context(&mut self) {
        self.process_context_pid = None;
    }

    pub fn set_replay_start_input(&mut self, value: String) {
        self.replay.set_start_at_input(value);
    }

    pub fn set_replay_end_input(&mut self, value: String) {
        self.replay.set_end_at_input(value);
    }

    pub fn set_replay_speed_from_input(&mut self, value: String) {
        if let Ok(speed) = value.parse::<f64>() {
            self.replay.set_speed(speed);
        }
    }

    pub fn set_replay_seek_ratio_from_input(&mut self, value: String) {
        if let Ok(slider_value) = value.parse::<f64>() {
            self.replay.seek_to_ratio(slider_value / 1000.0);
        }
    }

    pub fn dismiss_leak_alert(&mut self, pid: Pid) {
        self.dismissed_alert_pids.insert(pid);
    }
}
