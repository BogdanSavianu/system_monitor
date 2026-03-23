use std::collections::HashMap;
use std::fs::{File, read_dir};
use std::io::{BufRead, BufReader};
use std::thread;

use crate::hashmap;
use crate::model::SystemStatusFileModel;
use crate::process::Process;
use crate::state::SystemState;
use crate::util::{Pid, Tid, parser_utils::*};

pub use super::network_parser::*;
pub use super::process_parser::*;
pub use super::thread_parser::*;

pub struct Parser<
    ProcParser: TraitProcessParser,
    ThrParser: TraitThreadParser,
    NetParser: TraitNetworkParser,
> {
    pub process_parser: ProcParser,
    pub thread_parser: ThrParser,
    pub network_parser: NetParser,
}

impl<
    ProcParser: TraitProcessParser + Sync,
    ThrParser: TraitThreadParser + Sync,
    NetParser: TraitNetworkParser,
> Parser<ProcParser, ThrParser, NetParser>
{
    pub fn new(
        process_parser: ProcParser,
        thread_parser: ThrParser,
        network_parser: NetParser,
    ) -> Self {
        Parser {
            process_parser,
            thread_parser,
            network_parser,
        }
    }

    // this DOES NOT include jiffies
    pub fn parse_all_processes(&self) -> Vec<Process> {
        let mut process_paths: Vec<String> = vec![];
        if let Ok(entries) = read_dir("/proc") {
            for entry in entries {
                let Ok(entry) = entry else {
                    continue;
                };

                let process_path = entry.path();
                if process_path.is_dir() {
                    process_paths.push(process_path.display().to_string());
                }
            }
        }

        if process_paths.is_empty() {
            return vec![];
        }

        let workers = worker_count(process_paths.len());
        let chunk_size = process_paths.len().div_ceil(workers);
        let process_parser = &self.process_parser;

        let mut processes: Vec<Process> = Vec::new();
        thread::scope(|scope| {
            let mut handles = Vec::new();
            for chunk in process_paths.chunks(chunk_size) {
                handles.push(scope.spawn(move || {
                    let mut parsed: Vec<Process> = Vec::new();
                    for path in chunk {
                        if let Ok(process) = process_parser.parse_process(path) {
                            parsed.push(process);
                        }
                    }

                    parsed
                }));
            }

            for handle in handles {
                if let Ok(mut partial) = handle.join() {
                    processes.append(&mut partial);
                }
            }
        });

        processes.sort_by_key(|process| process.pid);

        processes
    }

    // obviously this ONLY includes jiffies
    pub fn parse_all_process_jiffies(&self) -> HashMap<Pid, u64> {
        let mut pids: Vec<Pid> = Vec::new();
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

                    pids.push(pid);
                }
            }
        }

        self.collect_process_jiffies(&pids)
    }

    pub fn parse_process(&self, file_path: &String) -> Result<Process, ParseError> {
        self.process_parser.parse_process(file_path)
    }

    pub fn refresh_process_snapshot(&self, system_state: &mut SystemState) {
        system_state.clear_process_snapshot();
        let processes = self.parse_all_processes();

        if processes.is_empty() {
            system_state.rebuild_process_hierarchy();
            return;
        }

        let workers = worker_count(processes.len());
        let chunk_size = processes.len().div_ceil(workers);
        let process_parser = &self.process_parser;
        let thread_parser = &self.thread_parser;

        let mut process_snapshots: Vec<(Process, Vec<crate::thread::Thread>)> = Vec::new();
        // normally thread::spawn does not allow use of variables outside its scope
        // but thread::scope guarantees the new thread does not outlive its creator
        // thus allowing the use of its local variables by passing references (process and thread parsers in this case)
        thread::scope(|scope| {
            let mut handles = Vec::new();
            for chunk in processes.chunks(chunk_size) {
                handles.push(scope.spawn(move || {
                    let mut partial: Vec<(Process, Vec<crate::thread::Thread>)> = Vec::new();
                    for process in chunk {
                        let pid = process.pid;
                        let threads = process_parser
                            .get_threads_for_pid(pid)
                            .unwrap_or_default()
                            .into_iter()
                            .map(|thread| thread_parser.parse_thread(pid, thread))
                            .collect::<Vec<_>>();

                        partial.push((process.clone(), threads));
                    }

                    partial
                }));
            }

            for handle in handles {
                if let Ok(mut partial) = handle.join() {
                    process_snapshots.append(&mut partial);
                }
            }
        });

        process_snapshots.sort_by_key(|(process, _)| process.pid);

        for (process, threads) in process_snapshots {
            let pid = process.pid;
            system_state.insert_process(process);
            for thread in threads {
                system_state.insert_thread(thread, pid);
            }
        }

        system_state.rebuild_process_hierarchy();
    }

    pub fn initialize_cpu_sampling(
        &self,
        system_state: &mut SystemState,
    ) -> Result<u64, ParseError> {
        self.refresh_process_snapshot(system_state);
        self.refresh_network_snapshot(system_state)?;

        let sys0 = self.get_status_info()?;
        system_state.num_cores = sys0.num_cores;

        let prev_jiffies = self.get_process_jiffies(system_state);
        system_state.update_jiffies(prev_jiffies);

        let prev_thread_jiffies = self.get_thread_jiffies(system_state);
        system_state.update_thread_jiffies(prev_thread_jiffies);

        Ok(sys0.total_cpu)
    }

    pub fn refresh_network_snapshot(
        &self,
        system_state: &mut SystemState,
    ) -> Result<(), ParseError> {
        let network_snapshot = self.network_parser.get_network_snapshot()?;
        system_state.update_network_snapshot(network_snapshot);

        Ok(())
    }

    pub fn get_status_info(&self) -> Result<SystemStatusFileModel, ParseError> {
        let file_path = format!("/proc/stat");
        let file =
            File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let buf_reader = BufReader::new(file);
        self.parse_status_info(buf_reader)
    }

    pub fn get_process_jiffies(&self, system_state: &SystemState) -> HashMap<Pid, u64> {
        let mut pids: Vec<Pid> = Vec::new();
        for process in system_state.processes.values() {
            pids.push(process.pid);
        }

        self.collect_process_jiffies(&pids)
    }

    fn collect_process_jiffies(&self, pids: &[Pid]) -> HashMap<Pid, u64> {
        if pids.is_empty() {
            return hashmap![];
        }

        let workers = worker_count(pids.len());
        let chunk_size = pids.len().div_ceil(workers);
        let process_parser = &self.process_parser;

        let mut jiffies: HashMap<Pid, u64> = hashmap![];
        thread::scope(|scope| {
            let mut handles = Vec::new();
            for chunk in pids.chunks(chunk_size) {
                handles.push(scope.spawn(move || {
                    let mut partial: Vec<(Pid, u64)> = Vec::new();
                    for pid in chunk {
                        if let Ok((utime, stime)) = process_parser.get_stat_info(*pid) {
                            partial.push((*pid, utime + stime));
                        }
                    }

                    partial
                }));
            }

            for handle in handles {
                if let Ok(partial) = handle.join() {
                    for (pid, jiffy) in partial {
                        jiffies.insert(pid, jiffy);
                    }
                }
            }
        });

        jiffies
    }

    pub fn get_thread_jiffies(&self, system_state: &SystemState) -> HashMap<Tid, u64> {
        let mut thread_pairs: Vec<(Pid, Tid)> = Vec::new();
        for (pid, tids) in system_state.threads_by_pid.iter() {
            for tid in tids {
                thread_pairs.push((*pid, *tid));
            }
        }

        if thread_pairs.is_empty() {
            return hashmap![];
        }

        let workers = worker_count(thread_pairs.len());
        let chunk_size = thread_pairs.len().div_ceil(workers);
        let thread_parser = &self.thread_parser;

        let mut jiffies: HashMap<Tid, u64> = hashmap![];
        thread::scope(|scope| {
            let mut handles = Vec::new();
            for chunk in thread_pairs.chunks(chunk_size) {
                handles.push(scope.spawn(move || {
                    let mut partial: Vec<(Tid, u64)> = Vec::new();
                    for (pid, tid) in chunk {
                        if let Ok((utime, stime)) = thread_parser.get_thread_stat_info(*pid, *tid) {
                            partial.push((*tid, utime + stime));
                        }
                    }

                    partial
                }));
            }

            for handle in handles {
                if let Ok(partial) = handle.join() {
                    for (tid, jiffy) in partial {
                        jiffies.insert(tid, jiffy);
                    }
                }
            }
        });

        jiffies
    }

    fn parse_status_info<R>(&self, reader: R) -> Result<SystemStatusFileModel, ParseError>
    where
        R: BufRead,
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
        model::{NetworkSnapshotModel, ProcessStatusFileModel},
        parser::{
            network_parser::TraitNetworkParser, process_parser::TraitProcessParser,
            thread_parser::TraitThreadParser,
        },
        process::Process,
        thread::Thread,
        util::{parser_utils::ParseError, types::Pid},
    };

    use super::Parser;

    struct DummyProcessParser;
    struct DummyThreadParser;
    struct DummyNetworkParser;

    impl TraitProcessParser for DummyProcessParser {
        fn parse_process(&self, _file_path: &String) -> Result<Process, ParseError> {
            Ok(Process::new(0))
        }

        fn get_threads_for_pid(&self, _pid: Pid) -> Result<Vec<Thread>, ParseError> {
            Ok(vec![])
        }

        fn get_status_info(&self, _pid: Pid) -> Result<ProcessStatusFileModel, ParseError> {
            Err(ParseError::ParsingError(
                "not used in this test".to_string(),
            ))
        }

        fn get_stat_info(&self, _pid: Pid) -> Result<(u64, u64), ParseError> {
            Err(ParseError::ParsingError(
                "not used in this test".to_string(),
            ))
        }

        fn get_process_name(&self, _pid: Pid) -> Result<String, ParseError> {
            Err(ParseError::ParsingError(
                "not used in this test".to_string(),
            ))
        }

        fn get_parent_pid(&self, _pid: Pid) -> Result<Pid, ParseError> {
            Err(ParseError::ParsingError(
                "not used in this test".to_string(),
            ))
        }

        fn get_process_cmdline(&self, _pid: Pid) -> Result<String, ParseError> {
            Err(ParseError::ParsingError(
                "not used in this test".to_string(),
            ))
        }
    }

    impl TraitThreadParser for DummyThreadParser {
        fn get_thread_stat_info(
            &self,
            _pid: Pid,
            _tid: crate::util::Tid,
        ) -> Result<(u64, u64), ParseError> {
            Err(ParseError::ParsingError(
                "not used in this test".to_string(),
            ))
        }

        fn parse_thread(&self, _pid: Pid, thread: Thread) -> Thread {
            thread
        }
    }

    impl TraitNetworkParser for DummyNetworkParser {
        fn get_net_tcp_info(&self) -> Result<Vec<crate::model::SocketInfoModel>, ParseError> {
            Ok(vec![])
        }

        fn get_net_tcp6_info(&self) -> Result<Vec<crate::model::SocketInfoModel>, ParseError> {
            Ok(vec![])
        }

        fn get_net_udp_info(&self) -> Result<Vec<crate::model::SocketInfoModel>, ParseError> {
            Ok(vec![])
        }

        fn get_net_udp6_info(&self) -> Result<Vec<crate::model::SocketInfoModel>, ParseError> {
            Ok(vec![])
        }

        fn get_all_net_socket_info(
            &self,
        ) -> Result<Vec<crate::model::SocketInfoModel>, ParseError> {
            Ok(vec![])
        }

        fn get_pid_socket_ownership(
            &self,
            _pid: Pid,
        ) -> Result<crate::model::PidSocketOwnershipModel, ParseError> {
            Ok(crate::model::PidSocketOwnershipModel::new())
        }

        fn get_all_pid_socket_ownership(
            &self,
        ) -> Result<Vec<crate::model::PidSocketOwnershipModel>, ParseError> {
            Ok(vec![])
        }

        fn get_process_network_stats(
            &self,
        ) -> Result<
            std::collections::HashMap<Pid, crate::model::ProcessNetworkStatsModel>,
            ParseError,
        > {
            Ok(std::collections::HashMap::new())
        }

        fn get_network_snapshot(&self) -> Result<NetworkSnapshotModel, ParseError> {
            Ok(NetworkSnapshotModel::new())
        }
    }

    #[test]
    fn parse_status_info_extracts_total_and_per_core_jiffies() {
        let parser = Parser::new(DummyProcessParser, DummyThreadParser, DummyNetworkParser);
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
        let parser = Parser::new(DummyProcessParser, DummyThreadParser, DummyNetworkParser);
        let input = "cpu0 1 2 3 4 5 6 7 8\n";

        let result = parser.parse_status_info(Cursor::new(input));
        assert!(result.is_err());
    }

    #[test]
    fn sum_cpu_jiffies_requires_minimum_fields() {
        let parser = Parser::new(DummyProcessParser, DummyThreadParser, DummyNetworkParser);
        let result = parser.sum_cpu_jiffies(&["1", "2", "3"]);

        assert!(result.is_err());
    }
}
