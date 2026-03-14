use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::io::{BufRead, BufReader};

use crate::hashmap;
use crate::process::Process;
use crate::state::SystemState;
use crate::util::{Pid, parser_utils::*};
use crate::model::SystemStatusFileModel;

pub use super::process_parser::*;

pub struct Parser<ProcParser: TraitProcessParser> {
    pub process_parser: ProcParser
}


impl<ProcParser: TraitProcessParser> Parser<ProcParser> {
    pub fn new(process_parser: ProcParser) -> Self {
        Parser { 
            process_parser,
        }
    }
    
    // this DOES NOT include jiffies
    pub fn parse_all_processes(&self) -> Vec<Process> {
        let mut processes: Vec<Process> = vec![];
        if let Ok(entries) = read_dir("/proc") {
            for entry in entries {
                let Ok(entry) = entry else {
                    continue;
                };

                let process_path = entry.path();
                if process_path.is_dir() {
                    let process_path_string = process_path.display().to_string();
                    if let Ok(process) = self.parse_process(&process_path_string) {
                        processes.push(process);
                    }
                }
            }
        }

        processes
    }

    // obviously this ONLY includes jiffies
    pub fn parse_all_process_jiffies(&self) -> HashMap<Pid, u64> {
        let mut jiffies: HashMap<Pid, u64> = hashmap![];
        if let Ok(entries) = read_dir("/proc") {
            for entry in entries {
                let Ok(entry) = entry else {
                    continue;
                };

                let process_path = entry.path();
                if process_path.is_dir() {
                    let process_path_string = process_path.display().to_string();
                    let Some(pid_str) = process_path_string.split('/').last() else {
                        continue;
                    };

                    let Ok(pid) = pid_str.parse::<u32>() else {
                        continue;
                    };

                    if let Ok((utime, stime)) = self.process_parser.get_stat_info(pid) {
                        jiffies.insert(pid, utime + stime);
                    }
                }
            }
        }

        jiffies
    }

    pub fn parse_process(&self, file_path: &String) -> Result<Process, ParseError> {
        self.process_parser.parse_process(file_path)
    }

    pub fn refresh_process_snapshot(&self, system_state: &mut SystemState) {
        system_state.clear_process_snapshot();
        let processes = self.parse_all_processes();

        for process in processes {
            let pid = process.pid;
            system_state.insert_process(process);

            if let Ok(threads) = self.process_parser.get_threads_for_pid(pid) {
                for thread in threads {
                    system_state.insert_thread(thread, pid);
                }
            }
        }
    }

    pub fn initialize_cpu_sampling(&self, system_state: &mut SystemState) -> Result<u64, ParseError> {
        self.refresh_process_snapshot(system_state);

        let sys0 = self.get_status_info()?;
        system_state.num_cores = sys0.num_cores;

        let prev_jiffies = self.get_process_jiffies(system_state);
        system_state.update_jiffies(prev_jiffies);

        Ok(sys0.total_cpu)
    }

    pub fn get_status_info(&self) -> Result<SystemStatusFileModel, ParseError> {
        let file_path = format!("/proc/stat");
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let buf_reader = BufReader::new(file);
        self.parse_status_info(buf_reader)
    }

    pub fn get_process_jiffies(&self, system_state: &SystemState) -> HashMap<Pid, u64> {
        let mut jiffies: HashMap<Pid, u64> = hashmap![];
        for p in system_state.processes.values() {
            if let Ok((utime, stime)) = self.process_parser.get_stat_info(p.pid) {
                jiffies.insert(p.pid, utime + stime);
            }
        }

        jiffies
    }

    fn parse_status_info<R>(&self, reader: R) -> Result<SystemStatusFileModel, ParseError>
        where R: BufRead,
    {
        let mut total_cpu: Option<u64> = None;
        let mut cpus: Vec<u64> = Vec::new();
        let mut cpu_section_started = false;

        for line in reader.lines() {
            let line = line.map_err(|err| ParseError::ParsingError(err.to_string()))?;
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let key = parts[0];
            if key == "cpu" {
                total_cpu = Some(self.sum_cpu_jiffies(&parts[1..])?);
                cpu_section_started = true;
                continue;
            }

            if let Some(index) = key.strip_prefix("cpu") {
                if !index.is_empty() && index.chars().all(|ch| ch.is_ascii_digit()) {
                    cpus.push(self.sum_cpu_jiffies(&parts[1..])?);
                    cpu_section_started = true;
                    continue;
                }
            }

            if cpu_section_started {
                break;
            }
        }

        let total_cpu = total_cpu.ok_or_else(|| {
            ParseError::ParsingError("missing aggregate cpu line in /proc/stat".to_string())
        })?;

        let num_cores = cpus.len() as u8;

        Ok(SystemStatusFileModel::build(total_cpu, cpus, num_cores))
    }


    fn sum_cpu_jiffies(&self, fields: &[&str]) -> Result<u64, ParseError> {
        if fields.len() < 8 {
            return Err(ParseError::ParsingError(
                "invalid /proc/stat cpu line: expected at least 8 fields".to_string(),
            ));
        }

        fields
            .iter()
            .take(8)
            .map(|x| x.parse::<u64>())
            .sum::<Result<u64, _>>()
            .map_err(ParseError::from)
    }

}
