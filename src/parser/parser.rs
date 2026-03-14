use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::io::{BufRead, BufReader};

use crate::hashmap;
use crate::process::Process;
use crate::state::SystemState;
use crate::util::{Pid, Tid, parser_utils::*};
use crate::model::SystemStatusFileModel;

pub use super::process_parser::*;
pub use super::thread_parser::*;

pub struct Parser<ProcParser: TraitProcessParser, ThrParser: TraitThreadParser> {
    pub process_parser: ProcParser,
    pub thread_parser: ThrParser,
}


impl<ProcParser: TraitProcessParser, ThrParser: TraitThreadParser> Parser<ProcParser, ThrParser> {
    pub fn new(process_parser: ProcParser, thread_parser: ThrParser) -> Self {
        Parser { 
            process_parser,
            thread_parser,
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
                    let parsed_thread = self.thread_parser.parse_thread(pid, thread);
                    system_state.insert_thread(parsed_thread, pid);
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

        let prev_thread_jiffies = self.get_thread_jiffies(system_state);
        system_state.update_thread_jiffies(prev_thread_jiffies);

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

    pub fn get_thread_jiffies(&self, system_state: &SystemState) -> HashMap<Tid, u64> {
        let mut jiffies: HashMap<Tid, u64> = hashmap![];

        for (pid, tids) in system_state.threads_by_pid.iter() {
            for tid in tids {
                if let Ok((utime, stime)) = self.thread_parser.get_thread_stat_info(*pid, *tid) {
                    jiffies.insert(*tid, utime + stime);
                }
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{
        model::ProcessStatusFileModel,
        parser::{process_parser::TraitProcessParser, thread_parser::TraitThreadParser},
        process::Process,
        thread::Thread,
        util::{parser_utils::ParseError, types::Pid},
    };

    use super::Parser;

    struct DummyProcessParser;
    struct DummyThreadParser;

    impl TraitProcessParser for DummyProcessParser {
        fn parse_process(&self, _file_path: &String) -> Result<Process, ParseError> {
            Ok(Process::new(0))
        }

        fn get_threads_for_pid(&self, _pid: Pid) -> Result<Vec<Thread>, ParseError> {
            Ok(vec![])
        }

        fn get_status_info(&self, _pid: Pid) -> Result<ProcessStatusFileModel, ParseError> {
            Err(ParseError::ParsingError("not used in this test".to_string()))
        }

        fn get_stat_info(&self, _pid: Pid) -> Result<(u64, u64), ParseError> {
            Err(ParseError::ParsingError("not used in this test".to_string()))
        }

        fn get_process_name(&self, _pid: Pid) -> Result<String, ParseError> {
            Err(ParseError::ParsingError("not used in this test".to_string()))
        }

        fn get_process_cmdline(&self, _pid: Pid) -> Result<String, ParseError> {
            Err(ParseError::ParsingError("not used in this test".to_string()))
        }
    }

    impl TraitThreadParser for DummyThreadParser {
        fn get_thread_stat_info(&self, _pid: Pid, _tid: crate::util::Tid) -> Result<(u64, u64), ParseError> {
            Err(ParseError::ParsingError("not used in this test".to_string()))
        }

        fn parse_thread(&self, _pid: Pid, thread: Thread) -> Thread {
            thread
        }
    }

    #[test]
    fn parse_status_info_extracts_total_and_per_core_jiffies() {
        let parser = Parser::new(DummyProcessParser, DummyThreadParser);
        let input = "cpu  10 20 30 40 50 60 70 80 90 100\n\
cpu0 1 2 3 4 5 6 7 8 9 10\n\
cpu1 2 3 4 5 6 7 8 9 10 11\n\
intr 123\n";

        let status = parser
            .parse_status_info(Cursor::new(input))
            .expect("status should parse");

        assert_eq!(status.total_cpu, 360);
        assert_eq!(status.cpus, vec![36, 44]);
        assert_eq!(status.num_cores, 2);
    }

    #[test]
    fn parse_status_info_fails_without_aggregate_cpu_line() {
        let parser = Parser::new(DummyProcessParser, DummyThreadParser);
        let input = "cpu0 1 2 3 4 5 6 7 8\n";

        let result = parser.parse_status_info(Cursor::new(input));
        assert!(result.is_err());
    }

    #[test]
    fn sum_cpu_jiffies_requires_minimum_fields() {
        let parser = Parser::new(DummyProcessParser, DummyThreadParser);
        let result = parser.sum_cpu_jiffies(&["1", "2", "3"]);

        assert!(result.is_err());
    }
}
