use std::collections::HashMap;

use crate::{
    dto::{ProcessCpuSampleDTO, ThreadCpuSampleDTO},
    parser::{parser::TraitProcessParser, thread_parser::TraitThreadParser, Parser, ProcessParser, ThreadParser},
    state::SystemState,
    util::{ParseError, Pid, Tid},
};

pub struct Monitor<ProcParser: TraitProcessParser, ThrParser: TraitThreadParser> {
    parser: Parser<ProcParser, ThrParser>,
    system_state: SystemState,
    previous_total_cpu: Option<u64>,
}

impl Monitor<ProcessParser, ThreadParser> {
    pub fn new() -> Self {
        Monitor {
            parser: Parser::new(ProcessParser::new(), ThreadParser::new()),
            system_state: SystemState::new(),
            previous_total_cpu: None,
        }
    }
}

impl<ProcParser: TraitProcessParser, ThrParser: TraitThreadParser> Monitor<ProcParser, ThrParser> {
    pub fn with_parsers(process_parser: ProcParser, thread_parser: ThrParser) -> Self {
        Monitor {
            parser: Parser::new(process_parser, thread_parser),
            system_state: SystemState::new(),
            previous_total_cpu: None,
        }
    }

    pub fn initialize_sampling(&mut self) -> Result<(), ParseError> {
        let total0 = self.parser.initialize_cpu_sampling(&mut self.system_state)?;
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

    pub fn sample_usage_maps(&mut self) -> Result<(HashMap<Pid, f64>, HashMap<Tid, f64>), ParseError> {
        let total0 = self.previous_total_cpu.ok_or_else(|| {
            ParseError::ParsingError("monitor sampling is not initialized".to_string())
        })?;

        self.parser.refresh_process_snapshot(&mut self.system_state);
        let new_jiffies = self.parser.get_process_jiffies(&self.system_state);
        let new_thread_jiffies = self.parser.get_thread_jiffies(&self.system_state);
        let total1 = self.parser.get_status_info()?.total_cpu;

        let process_cpu_usage = self
            .system_state
            .calculate_cpu_usage(new_jiffies.clone(), total0, total1);
        let thread_cpu_usage = self
            .system_state
            .calculate_thread_cpu_usage(new_thread_jiffies.clone(), total0, total1);

        self.system_state.update_jiffies(new_jiffies);
        self.system_state.update_thread_jiffies(new_thread_jiffies);
        self.previous_total_cpu = Some(total1);

        Ok((process_cpu_usage, thread_cpu_usage))
    }

    // adapter method that turns the HashMap into a more serializable Vec
    pub fn sample_cpu_usage(&mut self) -> Result<Vec<ProcessCpuSampleDTO>, ParseError> {
        let usage_map = self.sample_cpu_usage_map()?;
        let num_cores = self.system_state.num_cores as f64;

        let mut samples: Vec<ProcessCpuSampleDTO> = usage_map
            .into_iter()
            .filter_map(|(pid, cpu_norm)| {
                self.system_state.get_process(pid).map(|proc_| {
                    ProcessCpuSampleDTO::new(
                        pid,
                        proc_.name.clone(),
                        cpu_norm,
                        cpu_norm * num_cores,
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

        let mut samples: Vec<ThreadCpuSampleDTO> = usage_map
            .into_iter()
            .filter_map(|(tid, cpu_norm)| {
                self.system_state.threads_by_pid.iter().find_map(|(pid, tids)| {
                    if tids.contains(&tid) {
                        self.system_state.get_process(*pid).map(|proc_| {
                            ThreadCpuSampleDTO::new(
                                *pid,
                                tid,
                                proc_.name.clone(),
                                cpu_norm,
                                cpu_norm * num_cores,
                            )
                        })
                    } else {
                        None
                    }
                })
            })
            .collect();

        samples.sort_by(|a, b| b.cpu_top.total_cmp(&a.cpu_top));

        Ok(samples)
    }

    pub fn state(&self) -> &SystemState {
        &self.system_state
    }
}
