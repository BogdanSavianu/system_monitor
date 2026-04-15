use std::{collections::HashMap, time::SystemTime};
use tracing::{debug, info, warn};

use crate::{
    dto::{
        ProcessCpuSampleDTO, ProcessHierarchyIndexDTO, ProcessHierarchyNodeDTO,
        ProcessNetworkSampleDTO, ThreadCpuSampleDTO,
    },
    model::{ProcessHierarchyModel, ProcessNetworkStatsModel},
    parser::{
        NetworkParser, Parser, ProcessParser, ThreadParser, network_parser::TraitNetworkParser,
        parser::TraitProcessParser, thread_parser::TraitThreadParser,
    },
    state::SystemState,
    storage::{StorageSink, TraitSampleAccumulator},
    util::{ParseError, Pid, Tid},
};

pub struct MonitorObservation {
    pub collected_at: SystemTime,
    pub cpu: Vec<ProcessCpuSampleDTO>,
    pub network: Vec<ProcessNetworkSampleDTO>,
    pub total_cpu_top: f64,
    pub system_mem_used_kb: u64,
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
        Monitor {
            parser: Parser::new(process_parser, thread_parser, network_parser),
            system_state: SystemState::new(),
            previous_total_cpu: None,
            accumulator,
            storage_sink,
        }
    }

    fn persist_observation(
        &mut self,
        collected_at: SystemTime,
        cpu_samples: &[ProcessCpuSampleDTO],
        network_samples: &[ProcessNetworkSampleDTO],
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
        let (process_usage, _) = self.sample_usage_maps()?;
        Ok(process_usage)
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
        self.parser.refresh_process_snapshot(&mut self.system_state);
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
        let network = self.sample_process_network_stats()?;
        let total_cpu_top = cpu.iter().map(|sample| sample.cpu_top).sum::<f64>();
        let system_status = self.parser.get_status_info()?;
        let system_mem_used_kb = system_status
            .mem_total_kb
            .saturating_sub(system_status.mem_available_kb);

        self.persist_observation(collected_at, &cpu, &network);

        Ok(MonitorObservation {
            collected_at,
            cpu,
            network,
            total_cpu_top,
            system_mem_used_kb,
        })
    }

    pub fn sample_process_hierarchy_indexes(
        &mut self,
    ) -> Result<ProcessHierarchyIndexDTO, ParseError> {
        self.parser.refresh_process_snapshot(&mut self.system_state);
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
        self.parser.refresh_process_snapshot(&mut self.system_state);

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
        let num_cores = self.system_state.num_cores as f64;
        let usage_relative = self.system_state.calculate_relative_cpu_usage(
            &usage_map,
            self.system_state.get_total_proc_cpu_percentage(),
        );

        let mut samples: Vec<ProcessCpuSampleDTO> = usage_map
            .into_iter()
            .filter_map(|(pid, cpu_norm)| {
                self.system_state.get_process(pid).map(|proc_| {
                    let cpu_rel = usage_relative.get(&pid).copied().unwrap_or(0.0);
                    ProcessCpuSampleDTO::with_values(
                        pid,
                        proc_.name.clone(),
                        cpu_norm,
                        cpu_norm * num_cores,
                        cpu_rel,
                        proc_.virtual_mem,
                        proc_.physical_mem,
                    )
                })
            })
            .collect();

        samples.sort_by(|a, b| b.cpu_top.total_cmp(&a.cpu_top));

        Ok(samples)
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
