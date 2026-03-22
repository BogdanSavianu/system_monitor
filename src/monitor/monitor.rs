use std::collections::HashMap;

use crate::{
    dto::{ProcessCpuSampleDTO, ThreadCpuSampleDTO},
    parser::{
        NetworkParser, Parser, ProcessParser, ThreadParser, network_parser::TraitNetworkParser,
        parser::TraitProcessParser, thread_parser::TraitThreadParser,
    },
    state::SystemState,
    util::{ParseError, Pid, Tid},
};

pub struct Monitor<
    ProcParser: TraitProcessParser,
    ThrParser: TraitThreadParser,
    NetParser: TraitNetworkParser,
> {
    parser: Parser<ProcParser, ThrParser, NetParser>,
    system_state: SystemState,
    previous_total_cpu: Option<u64>,
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
        }
    }
}

impl<ProcParser: TraitProcessParser, ThrParser: TraitThreadParser, NetParser: TraitNetworkParser>
    Monitor<ProcParser, ThrParser, NetParser>
{
    pub fn with_parsers(
        process_parser: ProcParser,
        thread_parser: ThrParser,
        network_parser: NetParser,
    ) -> Self {
        Monitor {
            parser: Parser::new(process_parser, thread_parser, network_parser),
            system_state: SystemState::new(),
            previous_total_cpu: None,
        }
    }

    pub fn initialize_sampling(&mut self) -> Result<(), ParseError> {
        let total0 = self
            .parser
            .initialize_cpu_sampling(&mut self.system_state)?;
        self.previous_total_cpu = Some(total0);

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
        self.parser
            .refresh_network_snapshot(&mut self.system_state)?;
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

        Ok((process_cpu_usage.usages_norm, thread_cpu_usage))
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
