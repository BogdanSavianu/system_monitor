use std::{
    collections::{HashMap, HashSet},
    io::{BufRead, BufReader},
    process::Child,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use system_monitor::{
    dto::{
        ProcessCpuSampleDTO, ProcessHierarchyNodeDTO, ProcessNetworkSampleDTO, ThreadCpuSampleDTO,
    },
    leakprobe::{
        AllocScanReport, AllocationProfiler, BpftraceProfiler, CONTINUOUS_INTERVAL_SECS,
        ContinuousScanState, GcoreReachabilityScanner, PERSIST_EVERY_N_SNAPSHOTS,
        ReachabilityProfiler, ReachabilityReport, Symbolizer, process_continuous_chunk,
    },
    process::{ProcessControlService, TraitProcessControlService},
    storage::{
        AllocationSnapshot, PersistedLeakedBlock, PersistedReachabilityScan, SqliteSink,
        StorageSink, StoredSessionInfo,
    },
    util::{ParseError, Pid},
};
use tracing::info;

use crate::app::gui::replay::types::{
    ReplayAllocationPoint, ReplayLeakedBlock, ReplaySample, ReplayScanMarker,
};
use crate::app::gui::state::GuiPage;
use crate::app::{build_monitor_with_settings, factory::MonitorBuildSettings};

#[derive(Debug, Clone)]
pub struct CpuSnapshot {
    pub collected_at: SystemTime,
    pub cpu: Vec<ProcessCpuSampleDTO>,
    pub hierarchy_roots: Vec<ProcessHierarchyNodeDTO>,
    pub threads: Vec<ThreadCpuSampleDTO>,
    pub network: Vec<ProcessNetworkSampleDTO>,
    pub cmdline_by_pid: HashMap<Pid, String>,
    pub total_cpu_top: f64,
    pub system_mem_used_kb: u64,
    pub anomaly_by_pid: HashMap<Pid, bool>,
    pub load_avg: (f64, f64, f64),
    pub username_by_pid: HashMap<Pid, String>,
    pub num_cores: u8,
}

#[derive(Debug)]
pub enum BackendEvent {
    Snapshot(CpuSnapshot),
    ProcessControlSuccess {
        pid: Pid,
        operation: &'static str,
        signal: i32,
    },
    ProcessControlError {
        pid: Pid,
        operation: &'static str,
        message: String,
    },
    ReplayMemoryHistoryByName {
        name: String,
        samples: Vec<ReplaySample>,
    },
    ReplayScansList {
        scans: Vec<ReplayScanMarker>,
    },
    ReplayLeakedBlocks {
        scan_id: i64,
        blocks: Vec<ReplayLeakedBlock>,
    },
    ReplayAllocationSeries {
        series: Vec<ReplayAllocationPoint>,
    },
    MatchingProcessNames(Vec<String>),
    SessionsList(Vec<StoredSessionInfo>),
    MemoryBaselines(HashMap<String, f64>),
    DeepScanAvailability(bool),
    DeepScanStarted {
        pid: Pid,
    },
    DeepScanUpdate {
        pid: Pid,
        snapshot_count: usize,
        report: AllocScanReport,
    },
    DeepScanStopped {
        pid: Pid,
    },
    DeepScanError {
        pid: Pid,
        message: String,
    },
    ReachabilityAvailability(bool),
    ReachabilityScanStarted {
        pid: Pid,
    },
    ReachabilityScanResult(ReachabilityReport),
    ReachabilityScanError {
        pid: Pid,
        message: String,
    },
    Error(String),
    Stopped,
}

#[derive(Debug, Clone)]
pub enum BackendCommand {
    TerminateProcess(Pid),
    ForceKillProcess(Pid),
    FetchReplayMemoryByName {
        name: String,
        start_ms: i64,
        end_ms: i64,
    },
    FetchReplayScansByName {
        name: String,
        start_ms: i64,
        end_ms: i64,
    },
    FetchLeakedBlocksForScan {
        scan_id: i64,
    },
    FetchReplayAllocationByName {
        name: String,
        start_ms: i64,
        end_ms: i64,
    },
    PersistAllocationSnapshot {
        pid: Pid,
        name: String,
        collected_at_ms: i64,
        snapshot_count: u64,
        total_outstanding_bytes: u64,
    },
    SearchProcessNames(String),
    /// empty name_filter returns all sessions. a non-empty one restricts to
    /// sessions where the named process actually ran
    FetchSessionsList {
        name_filter: String,
    },
    SetActivePage(GuiPage),
    StartContinuousScan {
        pid: Pid,
    },
    StopContinuousScan {
        pid: Pid,
    },
    ReachabilityScan {
        pid: Pid,
    },
    ResetHistory,
}

pub struct GuiBackendHandle {
    pub events_rx: mpsc::Receiver<BackendEvent>,
    command_tx: mpsc::Sender<BackendCommand>,
    shutdown_tx: mpsc::Sender<()>,
    join_handle: Option<thread::JoinHandle<()>>,
}

impl GuiBackendHandle {
    fn send_command(&self, cmd: BackendCommand) -> Result<(), String> {
        self.command_tx
            .send(cmd)
            .map_err(|e| format!("backend channel closed: {e}"))
    }

    pub fn terminate_process(&self, pid: Pid) -> Result<(), String> {
        self.send_command(BackendCommand::TerminateProcess(pid))
    }

    pub fn force_kill_process(&self, pid: Pid) -> Result<(), String> {
        self.send_command(BackendCommand::ForceKillProcess(pid))
    }

    pub fn shutdown(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }

    pub fn fetch_replay_memory_by_name(
        &self,
        name: String,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<(), String> {
        self.send_command(BackendCommand::FetchReplayMemoryByName {
            name,
            start_ms,
            end_ms,
        })
    }

    pub fn fetch_replay_scans_by_name(
        &self,
        name: String,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<(), String> {
        self.send_command(BackendCommand::FetchReplayScansByName {
            name,
            start_ms,
            end_ms,
        })
    }

    pub fn fetch_leaked_blocks_for_scan(&self, scan_id: i64) -> Result<(), String> {
        self.send_command(BackendCommand::FetchLeakedBlocksForScan { scan_id })
    }

    pub fn fetch_replay_allocation_by_name(
        &self,
        name: String,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<(), String> {
        self.send_command(BackendCommand::FetchReplayAllocationByName {
            name,
            start_ms,
            end_ms,
        })
    }

    pub fn search_process_names(&self, prefix: String) -> Result<(), String> {
        self.send_command(BackendCommand::SearchProcessNames(prefix))
    }

    pub fn fetch_sessions_list(&self, name_filter: String) -> Result<(), String> {
        self.send_command(BackendCommand::FetchSessionsList { name_filter })
    }

    pub fn set_active_page(&self, page: GuiPage) -> Result<(), String> {
        self.send_command(BackendCommand::SetActivePage(page))
    }

    pub fn start_continuous_scan(&self, pid: Pid) -> Result<(), String> {
        self.send_command(BackendCommand::StartContinuousScan { pid })
    }

    pub fn stop_continuous_scan(&self, pid: Pid) -> Result<(), String> {
        self.send_command(BackendCommand::StopContinuousScan { pid })
    }

    pub fn reachability_scan(&self, pid: Pid) -> Result<(), String> {
        self.send_command(BackendCommand::ReachabilityScan { pid })
    }

    pub fn reset_history(&self) -> Result<(), String> {
        self.send_command(BackendCommand::ResetHistory)
    }
}

impl Drop for GuiBackendHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn spawn_backend(
    sample_interval: Duration,
    monitor_settings: MonitorBuildSettings,
) -> GuiBackendHandle {
    let (events_tx, events_rx) = mpsc::channel::<BackendEvent>();
    let (command_tx, command_rx) = mpsc::channel::<BackendCommand>();
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    // reader threads use this clone to send PersistAllocationSnapshot back into the
    // same command loop. the handle keeps the original.
    let command_tx_for_thread = command_tx.clone();

    let join_handle = thread::spawn(move || {
        let command_tx = command_tx_for_thread;
        let replay_db_path = monitor_settings.storage_db_path.clone();
        let mut monitor = build_monitor_with_settings(monitor_settings);
        let process_control = ProcessControlService::new();

        if let Err(err) = monitor.initialize_sampling() {
            let _ = events_tx.send(BackendEvent::Error(format!(
                "failed to initialize monitor: {:?}",
                err
            )));
            let _ = events_tx.send(BackendEvent::Stopped);
            return;
        }

        info!(target: "app::gui_backend", "gui backend initialized");

        // persistent read connection reused for all on-demand db queries
        let query_sink: Option<SqliteSink> = SqliteSink::new(&replay_db_path).ok();

        let profiler: Arc<BpftraceProfiler> = Arc::new(BpftraceProfiler::new());
        let _ = events_tx.send(BackendEvent::DeepScanAvailability(profiler.is_available()));
        let continuous_workers: Arc<Mutex<HashMap<Pid, ContinuousScanWorker>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let reachability: Arc<dyn ReachabilityProfiler> = Arc::new(GcoreReachabilityScanner::new());
        let _ = events_tx.send(BackendEvent::ReachabilityAvailability(
            reachability.is_available(),
        ));
        let reach_in_flight: Arc<Mutex<HashSet<Pid>>> = Arc::new(Mutex::new(HashSet::new()));

        // latest allocation-scan report per pid. the reachability join reads it to attach
        // allocation stacks to leaked blocks.
        let recent_alloc_reports: Arc<Mutex<HashMap<Pid, AllocScanReport>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let tx = events_tx.clone();
        let db_path = replay_db_path.clone();
        thread::spawn(move || {
            let Ok(sink) = SqliteSink::new(&db_path) else {
                return;
            };
            if let Ok(baselines) = sink.query_memory_baselines() {
                let _ = tx.send(BackendEvent::MemoryBaselines(baselines));
            }
        });

        let mut active_page = GuiPage::Monitor;

        loop {
            while let Ok(command) = command_rx.try_recv() {
                match command {
                    BackendCommand::TerminateProcess(pid) => {
                        match process_control.terminate_process(pid) {
                            Ok(result) => {
                                let _ = events_tx.send(BackendEvent::ProcessControlSuccess {
                                    pid,
                                    operation: "terminate",
                                    signal: result.signal,
                                });
                            }
                            Err(err) => {
                                let _ = events_tx.send(BackendEvent::ProcessControlError {
                                    pid,
                                    operation: "terminate",
                                    message: err.to_string(),
                                });
                            }
                        }
                    }
                    BackendCommand::ForceKillProcess(pid) => {
                        match process_control.force_kill_process(pid) {
                            Ok(result) => {
                                let _ = events_tx.send(BackendEvent::ProcessControlSuccess {
                                    pid,
                                    operation: "kill",
                                    signal: result.signal,
                                });
                            }
                            Err(err) => {
                                let _ = events_tx.send(BackendEvent::ProcessControlError {
                                    pid,
                                    operation: "kill",
                                    message: err.to_string(),
                                });
                            }
                        }
                    }
                    BackendCommand::FetchReplayMemoryByName {
                        name,
                        start_ms,
                        end_ms,
                    } => {
                        let Some(ref sink) = query_sink else {
                            let _ = events_tx.send(BackendEvent::Error("db not available".into()));
                            continue;
                        };
                        let event = sink
                            .query_process_memory_history_by_name(&name, start_ms, end_ms)
                            .map(|points| {
                                let samples = points
                                    .into_iter()
                                    .map(|p| ReplaySample {
                                        collected_at_ms: p.collected_at_ms,
                                        physical_mem_mb: p.physical_mem_kb as f64 / 1000.0,
                                        cpu_percent: p.cpu_top,
                                        is_anomalous: p.is_anomalous,
                                    })
                                    .collect();
                                BackendEvent::ReplayMemoryHistoryByName { name, samples }
                            })
                            .unwrap_or_else(|e| {
                                BackendEvent::Error(format!("replay query failed: {e}"))
                            });
                        let _ = events_tx.send(event);
                    }
                    BackendCommand::FetchReplayScansByName {
                        name,
                        start_ms,
                        end_ms,
                    } => {
                        let Some(ref sink) = query_sink else {
                            let _ = events_tx.send(BackendEvent::Error("db not available".into()));
                            continue;
                        };
                        let event = sink
                            .query_reachability_scans_by_name(&name, start_ms, end_ms)
                            .map(|scans| {
                                let markers = scans
                                    .into_iter()
                                    .map(|s| {
                                        let total_lost = s.definitely_lost_bytes
                                            + s.indirectly_lost_bytes
                                            + s.possibly_lost_bytes;
                                        ReplayScanMarker {
                                            id: s.id,
                                            pid: s.pid,
                                            started_at_ms: s.started_at_ms,
                                            total_blocks: s.total_blocks,
                                            total_lost_bytes: total_lost,
                                            definitely_lost_bytes: s.definitely_lost_bytes,
                                            possibly_lost_bytes: s.possibly_lost_bytes,
                                            indirectly_lost_bytes: s.indirectly_lost_bytes,
                                        }
                                    })
                                    .collect();
                                BackendEvent::ReplayScansList { scans: markers }
                            })
                            .unwrap_or_else(|e| {
                                BackendEvent::Error(format!("scans query failed: {e}"))
                            });
                        let _ = events_tx.send(event);
                    }
                    BackendCommand::FetchLeakedBlocksForScan { scan_id } => {
                        let Some(ref sink) = query_sink else {
                            let _ = events_tx.send(BackendEvent::Error("db not available".into()));
                            continue;
                        };
                        let event = sink
                            .query_leaked_blocks_for_scan(scan_id)
                            .map(|blocks| {
                                let mapped = blocks
                                    .into_iter()
                                    .map(|b| ReplayLeakedBlock {
                                        addr: b.addr,
                                        size: b.size,
                                        class: b.class,
                                        stack_text: b.stack_text,
                                    })
                                    .collect();
                                BackendEvent::ReplayLeakedBlocks {
                                    scan_id,
                                    blocks: mapped,
                                }
                            })
                            .unwrap_or_else(|e| {
                                BackendEvent::Error(format!("blocks query failed: {e}"))
                            });
                        let _ = events_tx.send(event);
                    }
                    BackendCommand::FetchReplayAllocationByName {
                        name,
                        start_ms,
                        end_ms,
                    } => {
                        let Some(ref sink) = query_sink else {
                            let _ = events_tx.send(BackendEvent::Error("db not available".into()));
                            continue;
                        };
                        let event = sink
                            .query_allocation_snapshots_by_name(&name, start_ms, end_ms)
                            .map(|rows| {
                                let series = rows
                                    .into_iter()
                                    .map(|r| ReplayAllocationPoint {
                                        collected_at_ms: r.collected_at_ms,
                                        outstanding_mb: r.total_outstanding_bytes as f64
                                            / (1024.0 * 1024.0),
                                    })
                                    .collect();
                                BackendEvent::ReplayAllocationSeries { series }
                            })
                            .unwrap_or_else(|e| {
                                BackendEvent::Error(format!("allocation query failed: {e}"))
                            });
                        let _ = events_tx.send(event);
                    }
                    BackendCommand::PersistAllocationSnapshot {
                        pid,
                        name,
                        collected_at_ms,
                        snapshot_count,
                        total_outstanding_bytes,
                    } => {
                        // short-lived write sink. the persistent query_sink is
                        // read-oriented and shared immutably.
                        if let Ok(mut sink) = SqliteSink::new(&replay_db_path) {
                            let row = AllocationSnapshot {
                                pid,
                                name,
                                collected_at_ms,
                                snapshot_count,
                                total_outstanding_bytes,
                            };
                            if let Err(err) = sink.persist_allocation_snapshot(&row) {
                                tracing::warn!(
                                    target: "app::gui_backend",
                                    pid,
                                    error = ?err,
                                    "failed to persist allocation snapshot"
                                );
                            }
                        }
                    }
                    BackendCommand::FetchSessionsList { name_filter } => {
                        let Some(ref sink) = query_sink else {
                            let _ = events_tx.send(BackendEvent::Error("db not available".into()));
                            continue;
                        };
                        let trimmed = name_filter.trim();
                        let result = if trimmed.is_empty() {
                            sink.query_sessions_list(20)
                        } else {
                            sink.query_sessions_list_for_name(trimmed, 20)
                        };
                        let event = result.map(BackendEvent::SessionsList).unwrap_or_else(|e| {
                            BackendEvent::Error(format!("sessions query failed: {e}"))
                        });
                        let _ = events_tx.send(event);
                    }
                    BackendCommand::SearchProcessNames(prefix) => {
                        let Some(ref sink) = query_sink else {
                            let _ = events_tx.send(BackendEvent::Error("db not available".into()));
                            continue;
                        };
                        let event = sink
                            .query_matching_process_names(&prefix, 20)
                            .map(BackendEvent::MatchingProcessNames)
                            .unwrap_or_else(|e| {
                                BackendEvent::Error(format!("name search failed: {e}"))
                            });
                        let _ = events_tx.send(event);
                    }
                    BackendCommand::SetActivePage(page) => {
                        active_page = page;
                    }
                    BackendCommand::ResetHistory => {
                        // truncation needs its own writer connection.
                        match SqliteSink::new(&replay_db_path) {
                            Ok(mut sink) => {
                                if let Err(err) = sink.truncate_history() {
                                    let _ = events_tx.send(BackendEvent::Error(format!(
                                        "reset history failed: {err}"
                                    )));
                                } else {
                                    let _ = events_tx
                                        .send(BackendEvent::Error("history cleared".to_string()));
                                    // drop in-memory caches that mirror history rows.
                                    recent_alloc_reports.lock().unwrap().clear();
                                }
                            }
                            Err(err) => {
                                let _ = events_tx.send(BackendEvent::Error(format!(
                                    "reset history: cannot open db: {err}"
                                )));
                            }
                        }
                    }
                    BackendCommand::ReachabilityScan { pid } => {
                        if !reachability.is_available() {
                            let _ = events_tx.send(BackendEvent::ReachabilityScanError {
                                pid,
                                message: "reachability scan unavailable (needs root + gcore)"
                                    .to_string(),
                            });
                            continue;
                        }
                        {
                            let mut guard = reach_in_flight.lock().unwrap();
                            if !guard.insert(pid) {
                                continue;
                            }
                        }
                        let name = monitor
                            .state()
                            .processes
                            .get(&pid)
                            .map(|p| p.name.clone())
                            .unwrap_or_default();
                        let _ = events_tx.send(BackendEvent::ReachabilityScanStarted { pid });

                        let tx = events_tx.clone();
                        let scanner = Arc::clone(&reachability);
                        let in_flight = Arc::clone(&reach_in_flight);
                        let alloc_cache = Arc::clone(&recent_alloc_reports);
                        let db_path = replay_db_path.clone();
                        thread::spawn(move || {
                            let event = match scanner.reachability_scan(pid, &name) {
                                Ok(report) => {
                                    // join with the recent same-pid allocation-scan report
                                    // so in-window blocks get allocation stacks.
                                    let alloc = alloc_cache.lock().unwrap().get(&pid).cloned();
                                    if std::env::var_os("LEAKPROBE_DEBUG").is_some() {
                                        let alloc_count = alloc
                                            .as_ref()
                                            .map(|a| a.alloc_stacks.len())
                                            .unwrap_or(0);
                                        let total_leaked = report.all_leaked.len();
                                        let matches_full = report
                                            .all_leaked
                                            .iter()
                                            .filter(|b| {
                                                alloc
                                                    .as_ref()
                                                    .map(|a| a.alloc_stacks.contains_key(&b.addr))
                                                    .unwrap_or(false)
                                            })
                                            .count();
                                        let matches_top = report
                                            .definitely_lost
                                            .top_blocks
                                            .iter()
                                            .filter(|b| {
                                                alloc
                                                    .as_ref()
                                                    .map(|a| a.alloc_stacks.contains_key(&b.addr))
                                                    .unwrap_or(false)
                                            })
                                            .count();
                                        let sample_leaked: Vec<String> = report
                                            .all_leaked
                                            .iter()
                                            .take(5)
                                            .map(|b| format!("{:#x}", b.addr))
                                            .collect();
                                        let now = SystemTime::now();
                                        let alloc_age_s = alloc
                                            .as_ref()
                                            .and_then(|a| {
                                                let end = a.started_at + a.duration;
                                                now.duration_since(end).ok()
                                            })
                                            .map(|d| d.as_secs_f64())
                                            .unwrap_or(f64::NAN);
                                        let alloc_window_s = alloc
                                            .as_ref()
                                            .map(|a| a.duration.as_secs_f64())
                                            .unwrap_or(0.0);
                                        eprintln!(
                                            "[leakprobe] join pid={pid}: \
                                             allocation-scan alloc_stacks={alloc_count}, \
                                             reachability-scan all_leaked={total_leaked} \
                                             (matched {matches_full} full / {matches_top} top-20); \
                                             allocation window={alloc_window_s:.1}s, reachability snapshot is \
                                             {alloc_age_s:.1}s after the allocation scan ended; \
                                             sample reachability leaked addrs: {sample_leaked:?}"
                                        );
                                    }
                                    persist_reachability_scan(&db_path, &report, alloc.as_ref());
                                    BackendEvent::ReachabilityScanResult(report)
                                }
                                Err(err) => BackendEvent::ReachabilityScanError {
                                    pid,
                                    message: err.to_string(),
                                },
                            };
                            let _ = tx.send(event);
                            in_flight.lock().unwrap().remove(&pid);
                        });
                    }
                    BackendCommand::StartContinuousScan { pid } => {
                        // idempotent: a capture already running for this pid is left alone.
                        {
                            let mut workers = continuous_workers.lock().unwrap();
                            // sweep workers whose target exited so a fresh start replaces them.
                            workers.retain(|_, w| !w.is_finished());
                            if workers.contains_key(&pid) {
                                continue;
                            }
                        }
                        if !profiler.is_available() {
                            let _ = events_tx.send(BackendEvent::DeepScanError {
                                pid,
                                message:
                                    "allocation-scan capture unavailable (needs root + bpftrace)"
                                        .to_string(),
                            });
                            continue;
                        }
                        let name = monitor
                            .state()
                            .processes
                            .get(&pid)
                            .map(|p| p.name.clone())
                            .unwrap_or_default();

                        match profiler.start_continuous(pid, CONTINUOUS_INTERVAL_SECS) {
                            Ok((mut child, symbolizer)) => {
                                let stdout = child.stdout.take().expect("child stdout piped");
                                let finished = Arc::new(AtomicBool::new(false));
                                let tx = events_tx.clone();
                                let cache = Arc::clone(&recent_alloc_reports);
                                let finished_for_thread = Arc::clone(&finished);
                                let cmd_tx = command_tx.clone();
                                let reader_handle = thread::spawn(move || {
                                    continuous_reader_loop(
                                        pid,
                                        name.clone(),
                                        stdout,
                                        symbolizer,
                                        cache,
                                        tx,
                                        cmd_tx,
                                    );
                                    finished_for_thread.store(true, Ordering::Release);
                                });
                                let worker = ContinuousScanWorker {
                                    child: Some(child),
                                    reader_handle: Some(reader_handle),
                                    finished,
                                };
                                continuous_workers.lock().unwrap().insert(pid, worker);
                                let _ = events_tx.send(BackendEvent::DeepScanStarted { pid });
                            }
                            Err(err) => {
                                let _ = events_tx.send(BackendEvent::DeepScanError {
                                    pid,
                                    message: err.to_string(),
                                });
                            }
                        }
                    }
                    BackendCommand::StopContinuousScan { pid } => {
                        // dropping the worker sigterms the child and joins the reader.
                        let worker = continuous_workers.lock().unwrap().remove(&pid);
                        drop(worker);
                        let _ = events_tx.send(BackendEvent::DeepScanStopped { pid });
                    }
                }
            }

            let snapshot = (|| -> Result<CpuSnapshot, ParseError> {
                let observation = monitor.sample_observation_cycle()?;
                let hierarchy_roots = monitor.sample_process_hierarchy_tree()?;
                let threads = if active_page == GuiPage::Monitor {
                    monitor.sample_thread_cpu_usage()?
                } else {
                    Vec::new()
                };
                let uid_to_username = build_username_map();
                let cmdline_by_pid: HashMap<Pid, String> = monitor
                    .state()
                    .processes
                    .iter()
                    .map(|(pid, process)| (*pid, process.cmdline.clone()))
                    .collect();
                let username_by_pid: HashMap<Pid, String> = monitor
                    .state()
                    .processes
                    .iter()
                    .map(|(pid, process)| {
                        let name = uid_to_username
                            .get(&process.uid)
                            .cloned()
                            .unwrap_or_else(|| process.uid.to_string());
                        (*pid, name)
                    })
                    .collect();

                Ok(CpuSnapshot {
                    collected_at: observation.collected_at,
                    cpu: observation.cpu,
                    hierarchy_roots,
                    threads,
                    network: observation.network,
                    cmdline_by_pid,
                    total_cpu_top: observation.total_cpu_top,
                    system_mem_used_kb: observation.system_mem_used_kb,
                    anomaly_by_pid: observation.anomaly_by_pid,
                    load_avg: observation.load_avg,
                    username_by_pid,
                    num_cores: monitor.state().num_cores,
                })
            })();

            match snapshot {
                Ok(snapshot) => {
                    if events_tx.send(BackendEvent::Snapshot(snapshot)).is_err() {
                        break;
                    }
                }
                Err(err) => {
                    let _ =
                        events_tx.send(BackendEvent::Error(format!("sampling failed: {:?}", err)));
                }
            }

            match shutdown_rx.recv_timeout(sample_interval) {
                Ok(()) => {
                    info!(target: "app::gui_backend", "gui backend shutdown requested");
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }

        monitor.flush_storage_pipeline();

        // tear down continuous captures before announcing Stopped.
        {
            let mut workers = continuous_workers.lock().unwrap();
            workers.clear();
        }

        let _ = events_tx.send(BackendEvent::Stopped);
    });

    GuiBackendHandle {
        events_rx,
        command_tx,
        shutdown_tx,
        join_handle: Some(join_handle),
    }
}

/// one bpftrace child plus its reader thread. drop sigterms the child and joins
/// the reader, so both stop and shutdown share one path.
struct ContinuousScanWorker {
    child: Option<Child>,
    reader_handle: Option<JoinHandle<()>>,
    /// set by the reader just before it exits, so the backend can sweep workers
    /// whose target died on its own.
    finished: Arc<AtomicBool>,
}

impl ContinuousScanWorker {
    fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

impl Drop for ContinuousScanWorker {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let pid = child.id() as i32;
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            let _ = child.wait();
        }
        if let Some(h) = self.reader_handle.take() {
            let _ = h.join();
        }
    }
}

/// reads bpftrace stdout, parses each `--- END_SNAPSHOT ---` chunk into an
/// AllocScanReport, updates the shared cache, and emits a DeepScanUpdate per
/// snapshot. exits on stdout eof.
fn continuous_reader_loop(
    pid: Pid,
    name: String,
    stdout: std::process::ChildStdout,
    mut symbolizer: Symbolizer,
    cache: Arc<Mutex<HashMap<Pid, AllocScanReport>>>,
    events_tx: mpsc::Sender<BackendEvent>,
    command_tx: mpsc::Sender<BackendCommand>,
) {
    let reader = BufReader::new(stdout);
    let mut state = ContinuousScanState::new(pid, name.clone(), CONTINUOUS_INTERVAL_SECS);
    let mut chunk = String::new();
    let debug = std::env::var_os("LEAKPROBE_DEBUG").is_some();

    for line_res in reader.lines() {
        let Ok(line) = line_res else {
            break;
        };
        chunk.push_str(&line);
        chunk.push('\n');
        if line.trim() != "--- END_SNAPSHOT ---" {
            continue;
        }
        if let Some(report) = process_continuous_chunk(&chunk, &mut state, &mut symbolizer) {
            cache.lock().unwrap().insert(pid, report.clone());
            let snapshot_count = state.snapshot_count();
            if debug {
                eprintln!(
                    "[leakprobe] continuous pid={pid} snapshot={} outstanding={}B alloc_stacks={}",
                    snapshot_count,
                    report.total_outstanding_bytes,
                    report.alloc_stacks.len(),
                );
            }

            // persist one summary row per Nth snapshot (about 1/minute) for
            // replay's outstanding-bytes line.
            if snapshot_count > 0 && snapshot_count % PERSIST_EVERY_N_SNAPSHOTS == 0 {
                let collected_at_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                let _ = command_tx.send(BackendCommand::PersistAllocationSnapshot {
                    pid,
                    name: name.clone(),
                    collected_at_ms,
                    snapshot_count: snapshot_count as u64,
                    total_outstanding_bytes: report.total_outstanding_bytes,
                });
            }

            if events_tx
                .send(BackendEvent::DeepScanUpdate {
                    pid,
                    snapshot_count,
                    report,
                })
                .is_err()
            {
                break;
            }
        }
        chunk.clear();
    }
    // eof: bpftrace exited. the command loop sweeps the finished worker next tick.
    let _ = events_tx.send(BackendEvent::DeepScanStopped { pid });
}

fn persist_reachability_scan(
    db_path: &std::path::Path,
    report: &ReachabilityReport,
    alloc_report: Option<&AllocScanReport>,
) {
    let Ok(mut sink) = SqliteSink::new(db_path) else {
        return;
    };
    let started_at_ms = report
        .started_at
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let mut top_blocks: Vec<PersistedLeakedBlock> = Vec::new();
    let lookup_stack = |addr: u64| -> Option<String> {
        alloc_report
            .and_then(|a| a.alloc_stacks.get(&addr))
            .map(|frames| frames.join("\n"))
    };
    for group in [
        &report.definitely_lost,
        &report.possibly_lost,
        &report.indirectly_lost,
        &report.still_reachable,
    ] {
        for b in &group.top_blocks {
            top_blocks.push(PersistedLeakedBlock {
                addr: b.addr,
                size: b.size,
                class: b.class.as_str().to_string(),
                stack_text: lookup_stack(b.addr),
            });
        }
    }
    let persisted = PersistedReachabilityScan {
        pid: report.pid,
        name: report.name.clone(),
        started_at_ms,
        duration_s: report.duration.as_secs(),
        allocator: report.allocator.clone(),
        total_blocks: report.total_blocks as i64,
        total_bytes: report.total_bytes,
        definitely_lost_bytes: report.definitely_lost.total_bytes,
        possibly_lost_bytes: report.possibly_lost.total_bytes,
        indirectly_lost_bytes: report.indirectly_lost.total_bytes,
        still_reachable_bytes: report.still_reachable.total_bytes,
        top_blocks,
    };
    if let Err(err) = sink.persist_reachability_scan(&persisted) {
        tracing::warn!(target: "app::gui_backend", error = %err, "failed to persist reachability scan");
    }
}

fn build_username_map() -> HashMap<u32, String> {
    let Ok(contents) = std::fs::read_to_string("/etc/passwd") else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for line in contents.lines() {
        let mut parts = line.splitn(4, ':');
        let name = parts.next();
        let _ = parts.next(); // password field
        let uid_str = parts.next();
        if let (Some(name), Some(uid_str)) = (name, uid_str)
            && let Ok(uid) = uid_str.parse::<u32>()
        {
            map.insert(uid, name.to_string());
        }
    }
    map
}
