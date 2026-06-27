use std::{collections::HashMap, time::SystemTime};
use tracing::{debug, info, warn};

use crate::{
    dto::{
        ProcessCpuSampleDTO, ProcessHierarchyIndexDTO, ProcessHierarchyNodeDTO,
        ProcessNetworkSampleDTO, ThreadCpuSampleDTO,
    },
    ml::MemoryLeakDetector,
    model::{ProcessHierarchyModel, ProcessNetworkStatsModel},
    parser::{
        NetworkParser, Parser, ProcessParser, ThreadParser, network_parser::TraitNetworkParser,
        parser::TraitProcessParser, thread_parser::TraitThreadParser,
    },
    process::ProcessState,
    state::SystemState,
    storage::{StorageSink, TraitSampleAccumulator},
    util::{ParseError, Pid, Pm, Tid, Vm},
};

pub struct MonitorObservation {
    pub collected_at: SystemTime,
    pub cpu: Vec<ProcessCpuSampleDTO>,
    pub network: Vec<ProcessNetworkSampleDTO>,
    pub total_cpu_top: f64,
    pub system_mem_used_kb: u64,
    pub anomaly_by_pid: HashMap<Pid, bool>,
    pub anomaly_transitions: Vec<MonitorAnomalyTransition>,
    pub load_avg: (f64, f64, f64),
}

#[derive(Debug, Clone)]
pub struct MonitorAnomalyTransition {
    pub pid: Pid,
    pub process_name: String,
    pub is_anomalous: bool,
}

pub struct Monitor<
    ProcParser: TraitProcessParser,
    ThrParser: TraitThreadParser,
    NetParser: TraitNetworkParser,
> {
    parser: Parser<ProcParser, ThrParser, NetParser>,
    system_state: SystemState,
    previous_total_cpu: Option<u64>,
    accumulator: Option<Box<dyn TraitSampleAccumulator + Send>>,
    storage_sink: Option<Box<dyn StorageSink + Send>>,
    leak_detector: Option<MemoryLeakDetector>,
    prev_process_io: HashMap<Pid, (u64, u64, SystemTime)>,
    /// pids whose /proc/<pid>/io was unreadable last tick, skipped next tick to
    /// avoid spamming open() on processes we cannot read
    unreadable_io_pids: std::collections::HashSet<Pid>,
}

impl Monitor<ProcessParser, ThreadParser, NetworkParser> {
    pub fn new() -> Self {
        Monitor {
            parser: Parser::new(
                ProcessParser::new(),
                ThreadParser::new(),
                NetworkParser::new(),
            ),
            system_state: SystemState::new(),
            previous_total_cpu: None,
            accumulator: None,
            storage_sink: None,
            leak_detector: None,
            prev_process_io: HashMap::new(),
            unreadable_io_pids: std::collections::HashSet::new(),
        }
    }
}

impl<
    ProcParser: TraitProcessParser + Sync,
    ThrParser: TraitThreadParser + Sync,
    NetParser: TraitNetworkParser,
> Monitor<ProcParser, ThrParser, NetParser>
{
    pub fn with_parsers(
        process_parser: ProcParser,
        thread_parser: ThrParser,
        network_parser: NetParser,
    ) -> Self {
        Self::with_parsers_and_pipeline(process_parser, thread_parser, network_parser, None, None)
    }

    pub fn with_parsers_and_pipeline(
        process_parser: ProcParser,
        thread_parser: ThrParser,
        network_parser: NetParser,
        accumulator: Option<Box<dyn TraitSampleAccumulator + Send>>,
        storage_sink: Option<Box<dyn StorageSink + Send>>,
    ) -> Self {
        Self::with_parsers_pipeline_and_detector(
            process_parser,
            thread_parser,
            network_parser,
            accumulator,
            storage_sink,
            None,
        )
    }

    pub fn with_parsers_pipeline_and_detector(
        process_parser: ProcParser,
        thread_parser: ThrParser,
        network_parser: NetParser,
        accumulator: Option<Box<dyn TraitSampleAccumulator + Send>>,
        storage_sink: Option<Box<dyn StorageSink + Send>>,
        leak_detector: Option<MemoryLeakDetector>,
    ) -> Self {
        Monitor {
            parser: Parser::new(process_parser, thread_parser, network_parser),
            system_state: SystemState::new(),
            previous_total_cpu: None,
            accumulator,
            storage_sink,
            leak_detector,
            prev_process_io: HashMap::new(),
            unreadable_io_pids: std::collections::HashSet::new(),
        }
    }

    pub fn set_memory_leak_detector(&mut self, leak_detector: Option<MemoryLeakDetector>) {
        self.leak_detector = leak_detector;
    }

    fn persist_observation(
        &mut self,
        collected_at: SystemTime,
        cpu_samples: &[ProcessCpuSampleDTO],
        network_samples: &[ProcessNetworkSampleDTO],
        anomaly_by_pid: &HashMap<Pid, bool>,
    ) {
        let Some(accumulator) = self.accumulator.as_mut() else {
            return;
        };
        let Some(sink) = self.storage_sink.as_mut() else {
            return;
        };

        let maybe_batch = accumulator.accumulate(
            collected_at,
            cpu_samples,
            network_samples,
            &self.system_state,
            anomaly_by_pid,
        );

        let Some(batch) = maybe_batch else {
            return;
        };

        if let Err(err) = sink.persist_sample_batch(&batch) {
            warn!(target: "monitor::storage", error = ?err, "failed to persist sample batch");
        }
    }

    pub fn flush_storage_pipeline(&mut self) {
        let collected_at = SystemTime::now();

        if let (Some(accumulator), Some(sink)) =
            (self.accumulator.as_mut(), self.storage_sink.as_mut())
        {
            if let Some(batch) = accumulator.drain_pending(collected_at)
                && let Err(err) = sink.persist_sample_batch(&batch)
            {
                warn!(target: "monitor::storage", error = ?err, "failed to persist drained sample batch");
            }

            if let Err(err) = sink.flush() {
                warn!(target: "monitor::storage", error = ?err, "failed to flush sink");
            }
        }
    }

    pub fn initialize_sampling(&mut self) -> Result<(), ParseError> {
        let total0 = self
            .parser
            .initialize_cpu_sampling(&mut self.system_state)?;
        self.previous_total_cpu = Some(total0);

        info!(
            target: "monitor::sampling",
            processes = self.system_state.processes.len(),
            threads = self.system_state.threads.len(),
            cores = self.system_state.num_cores,
            "monitor sampling initialized"
        );

        Ok(())
    }

    pub fn sample_cpu_usage_map(&mut self) -> Result<HashMap<Pid, f64>, ParseError> {
        let total0 = self.previous_total_cpu.ok_or_else(|| {
            ParseError::ParsingError("monitor sampling is not initialized".to_string())
        })?;

        let t0 = std::time::Instant::now();
        self.parser
            .refresh_process_snapshot_no_threads(&mut self.system_state);
        info!(target: "monitor::timing", ms = t0.elapsed().as_millis(), "refresh_process_snapshot_no_threads");

        let t1 = std::time::Instant::now();
        let new_jiffies = self.parser.get_process_jiffies(&self.system_state);
        info!(target: "monitor::timing", ms = t1.elapsed().as_millis(), "get_process_jiffies");

        let t2 = std::time::Instant::now();
        let total1 = self.parser.get_status_info()?.total_cpu;
        info!(target: "monitor::timing", ms = t2.elapsed().as_millis(), "get_status_info");

        let process_cpu_usage = self
            .system_state
            .calculate_cpu_usage(&new_jiffies, total0, total1);

        self.system_state.update_jiffies(new_jiffies);
        self.system_state
            .set_total_proc_cpu_percentage(process_cpu_usage.total_proc_cpu_usage);
        self.previous_total_cpu = Some(total1);

        Ok(process_cpu_usage.usages_norm)
    }

    pub fn sample_thread_cpu_usage_map(&mut self) -> Result<HashMap<Tid, f64>, ParseError> {
        let (_, thread_usage) = self.sample_usage_maps()?;
        Ok(thread_usage)
    }

    pub fn sample_usage_maps(
        &mut self,
    ) -> Result<(HashMap<Pid, f64>, HashMap<Tid, f64>), ParseError> {
        let total0 = self.previous_total_cpu.ok_or_else(|| {
            ParseError::ParsingError("monitor sampling is not initialized".to_string())
        })?;

        self.parser.refresh_process_snapshot(&mut self.system_state);
        let new_jiffies = self.parser.get_process_jiffies(&self.system_state);
        let new_thread_jiffies = self.parser.get_thread_jiffies(&self.system_state);
        let total1 = self.parser.get_status_info()?.total_cpu;
        let process_cpu_usage = self
            .system_state
            .calculate_cpu_usage(&new_jiffies, total0, total1);
        let thread_cpu_usage =
            self.system_state
                .calculate_thread_cpu_usage(&new_thread_jiffies, total0, total1);

        self.system_state.update_jiffies(new_jiffies);
        self.system_state.update_thread_jiffies(new_thread_jiffies);
        self.system_state
            .set_total_proc_cpu_percentage(process_cpu_usage.total_proc_cpu_usage);
        self.previous_total_cpu = Some(total1);

        debug!(
            target: "monitor::sampling",
            process_count = process_cpu_usage.usages_norm.len(),
            thread_count = thread_cpu_usage.len(),
            total_proc_cpu = process_cpu_usage.total_proc_cpu_usage,
            "sampled cpu usage maps"
        );

        Ok((process_cpu_usage.usages_norm, thread_cpu_usage))
    }

    pub fn sample_process_network_stats_map(
        &mut self,
    ) -> Result<HashMap<Pid, ProcessNetworkStatsModel>, ParseError> {
        self.parser.network_parser.get_process_network_stats()
    }

    pub fn sample_process_network_stats(
        &mut self,
    ) -> Result<Vec<ProcessNetworkSampleDTO>, ParseError> {
        let stats_by_pid = self.sample_process_network_stats_map()?;

        let mut samples: Vec<ProcessNetworkSampleDTO> = stats_by_pid
            .into_values()
            .map(|stats| {
                let process_name = self
                    .system_state
                    .get_process(stats.pid)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();

                ProcessNetworkSampleDTO::from_model(process_name, &stats)
            })
            .collect();

        samples.sort_by(|a, b| {
            b.total_sockets
                .cmp(&a.total_sockets)
                .then_with(|| b.tcp_open.cmp(&a.tcp_open))
                .then_with(|| b.udp_open.cmp(&a.udp_open))
        });

        debug!(target: "monitor::sampling", sample_count = samples.len(), "sampled process network stats");

        Ok(samples)
    }

    pub fn sample_observation_cycle(&mut self) -> Result<MonitorObservation, ParseError> {
        let collected_at = SystemTime::now();
        let cpu = self.sample_cpu_usage()?;
        info!(target: "monitor::sampling", cpu_count = cpu.len(), "cpu samples ready");
        let network = self.sample_process_network_stats()?;
        let total_cpu_top = cpu.iter().map(|sample| sample.cpu_top).sum::<f64>();
        let system_status = self.parser.get_status_info()?;
        let system_mem_used_kb = system_status
            .mem_total_kb
            .saturating_sub(system_status.mem_available_kb);

        // detector runs before persistence so the batch carries the per-sample
        // anomaly verdict that replay reads back.
        let (anomaly_by_pid, anomaly_transitions) =
            if let Some(detector) = self.leak_detector.as_mut() {
                let result = detector.evaluate(collected_at, &cpu);
                let transitions = result
                    .transitions
                    .into_iter()
                    .map(|transition| MonitorAnomalyTransition {
                        pid: transition.pid,
                        process_name: transition.process_name,
                        is_anomalous: transition.is_anomalous,
                    })
                    .collect::<Vec<_>>();

                (result.anomaly_by_pid, transitions)
            } else {
                (HashMap::new(), Vec::new())
            };

        self.persist_observation(collected_at, &cpu, &network, &anomaly_by_pid);

        for transition in &anomaly_transitions {
            if transition.is_anomalous {
                info!(
                    target: "monitor::anomaly",
                    pid = transition.pid,
                    process = transition.process_name.as_str(),
                    "memory leak anomaly detected"
                );
            } else {
                info!(
                    target: "monitor::anomaly",
                    pid = transition.pid,
                    process = transition.process_name.as_str(),
                    "memory leak anomaly cleared"
                );
            }
        }

        let load_avg = read_load_avg();

        Ok(MonitorObservation {
            collected_at,
            cpu,
            network,
            total_cpu_top,
            system_mem_used_kb,
            anomaly_by_pid,
            anomaly_transitions,
            load_avg,
        })
    }

    pub fn sample_process_hierarchy_indexes(
        &mut self,
    ) -> Result<ProcessHierarchyIndexDTO, ParseError> {
        let hierarchy = &self.system_state.process_hierarchy;

        Ok(ProcessHierarchyIndexDTO::with_values(
            hierarchy.pid_to_ppid.clone(),
            hierarchy.children_by_pid.clone(),
            hierarchy.roots.clone(),
        ))
    }

    pub fn sample_process_hierarchy_tree(
        &mut self,
    ) -> Result<Vec<ProcessHierarchyNodeDTO>, ParseError> {
        let mut roots = Vec::new();
        for root_pid in &self.system_state.process_hierarchy.roots {
            roots.push(self.build_hierarchy_node(*root_pid));
        }

        debug!(target: "monitor::sampling", root_count = roots.len(), "sampled process hierarchy tree");

        Ok(roots)
    }

    fn build_hierarchy_node(&self, pid: Pid) -> ProcessHierarchyNodeDTO {
        let hierarchy: &ProcessHierarchyModel = &self.system_state.process_hierarchy;
        let children_pids = hierarchy
            .children_by_pid
            .get(&pid)
            .cloned()
            .unwrap_or_default();

        let children = children_pids
            .into_iter()
            .map(|child_pid| self.build_hierarchy_node(child_pid))
            .collect();

        let ppid = hierarchy.pid_to_ppid.get(&pid).copied().unwrap_or(0);
        let name = self
            .system_state
            .get_process(pid)
            .map(|process| process.name.clone())
            .unwrap_or_default();

        ProcessHierarchyNodeDTO::with_values(pid, ppid, name, children)
    }

    // adapter method that turns the HashMap into a more serializable Vec
    pub fn sample_cpu_usage(&mut self) -> Result<Vec<ProcessCpuSampleDTO>, ParseError> {
        let usage_map = self.sample_cpu_usage_map()?;
        info!(target: "monitor::sampling", usage_count = usage_map.len(), "usage map built");
        let num_cores = self.system_state.num_cores as f64;
        let usage_relative = self.system_state.calculate_relative_cpu_usage(
            &usage_map,
            self.system_state.get_total_proc_cpu_percentage(),
        );

        let now = SystemTime::now();

        struct ProcSnapshot {
            pid: Pid,
            cpu_norm: f64,
            name: String,
            virtual_mem: Vm,
            physical_mem: Pm,
            state: ProcessState,
            swap_mem: u32,
            fd_size: u32,
        }
        let snapshots: Vec<ProcSnapshot> = usage_map
            .into_iter()
            .filter_map(|(pid, cpu_norm)| {
                self.system_state.get_process(pid).map(|p| ProcSnapshot {
                    pid,
                    cpu_norm,
                    name: p.name.clone(),
                    virtual_mem: p.virtual_mem,
                    physical_mem: p.physical_mem,
                    state: p.state,
                    swap_mem: p.swap_mem,
                    fd_size: p.fd_size,
                })
            })
            .collect();

        let pids_in_cycle: Vec<Pid> = snapshots.iter().map(|s| s.pid).collect();

        let mut samples: Vec<ProcessCpuSampleDTO> = snapshots
            .into_iter()
            .map(|s| {
                let cpu_rel = usage_relative.get(&s.pid).copied().unwrap_or(0.0);
                let (disk_read_kb_s, disk_write_kb_s) = self.sample_process_io_rate(s.pid, now);
                ProcessCpuSampleDTO {
                    pid: s.pid,
                    name: s.name,
                    cpu_norm: s.cpu_norm,
                    cpu_top: s.cpu_norm * num_cores,
                    cpu_rel,
                    virtual_mem: s.virtual_mem,
                    physical_mem: s.physical_mem,
                    state: s.state,
                    swap_mem: s.swap_mem,
                    fd_count: s.fd_size,
                    disk_read_kb_s,
                    disk_write_kb_s,
                }
            })
            .collect();

        // evict counters for pids that disappeared so a recycled pid starts fresh
        let alive: std::collections::HashSet<Pid> = pids_in_cycle.into_iter().collect();
        self.prev_process_io.retain(|pid, _| alive.contains(pid));
        self.unreadable_io_pids.retain(|pid| alive.contains(pid));

        samples.sort_by(|a, b| b.cpu_top.total_cmp(&a.cpu_top));

        Ok(samples)
    }

    /// diffs /proc/<pid>/io against last tick to get (read, write) KB/s
    fn sample_process_io_rate(&mut self, pid: Pid, now: SystemTime) -> (f64, f64) {
        if self.unreadable_io_pids.contains(&pid) {
            return (0.0, 0.0);
        }
        let Some((read_bytes, write_bytes)) = self.parser.process_parser.get_io_bytes(pid) else {
            self.unreadable_io_pids.insert(pid);
            self.prev_process_io.remove(&pid);
            return (0.0, 0.0);
        };

        let (read_kb_s, write_kb_s) = if let Some((prev_read, prev_write, prev_at)) =
            self.prev_process_io.get(&pid).copied()
        {
            let dt_s = now
                .duration_since(prev_at)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            if dt_s > 0.0 {
                let dr = read_bytes.saturating_sub(prev_read) as f64;
                let dw = write_bytes.saturating_sub(prev_write) as f64;
                (dr / dt_s / 1024.0, dw / dt_s / 1024.0)
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        self.prev_process_io
            .insert(pid, (read_bytes, write_bytes, now));
        (read_kb_s, write_kb_s)
    }

    pub fn sample_thread_cpu_usage(&mut self) -> Result<Vec<ThreadCpuSampleDTO>, ParseError> {
        let usage_map = self.sample_thread_cpu_usage_map()?;
        let num_cores = self.system_state.num_cores as f64;

        let mut tid_to_pid: HashMap<Tid, Pid> = HashMap::new();
        for (pid, tids) in self.system_state.threads_by_pid.iter() {
            for tid in tids {
                tid_to_pid.insert(*tid, *pid);
            }
        }

        let mut samples: Vec<ThreadCpuSampleDTO> = usage_map
            .into_iter()
            .filter_map(|(tid, cpu_norm)| {
                let pid = tid_to_pid.get(&tid)?;
                let process = self.system_state.get_process(*pid)?;
                let thread = self.system_state.threads.get(&tid)?;

                let mut dto = ThreadCpuSampleDTO::new(
                    *pid,
                    tid,
                    process.name.clone(),
                    thread.name.clone(),
                    cpu_norm,
                    cpu_norm * num_cores,
                );

                dto.state = thread.state;
                dto.last_cpu = thread.last_cpu;
                dto.voluntary_ctxt_switches = thread.voluntary_ctxt_switches;
                dto.nonvoluntary_ctxt_switches = thread.nonvoluntary_ctxt_switches;
                dto.io_read_bytes = thread.io_read_bytes;
                dto.io_write_bytes = thread.io_write_bytes;
                dto.io_rchar = thread.io_rchar;
                dto.io_wchar = thread.io_wchar;
                dto.io_syscr = thread.io_syscr;
                dto.io_syscw = thread.io_syscw;

                Some(dto)
            })
            .collect();

        samples.sort_by(|a, b| b.cpu_top.total_cmp(&a.cpu_top));

        Ok(samples)
    }

    pub fn state(&self) -> &SystemState {
        &self.system_state
    }
}

fn read_load_avg() -> (f64, f64, f64) {
    let Ok(content) = std::fs::read_to_string("/proc/loadavg") else {
        return (0.0, 0.0, 0.0);
    };
    let mut parts = content.split_whitespace();
    let la1 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let la5 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    let la15 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    (la1, la5, la15)
}
