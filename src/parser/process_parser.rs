use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};

use crate::process::Process;
use crate::state::SystemState;
use crate::thread::Thread;
use crate::util::{types::*, parser_utils::*};

const BASE_PROC_PATH: &str = "/proc";

pub struct ProcessParser;

impl ProcessParser {
    pub fn new() -> Self {
        ProcessParser {}
    }

    pub fn parse_process(&self, system_state: &mut SystemState, file_path: &String) -> Result<Process, ParseError> {
        let pid = extract_pid_from_path(file_path)?;
        let mut process = Process::new(pid);
        let name = self.get_process_name(pid)?;
        let cmdline = self.get_process_cmdline(pid)?;
        let threads = self.get_threads_for_pid(pid)?;
        let (vm, pm) = self.get_mem_info(pid)?;

        process.name = name;
        process.cmdline = cmdline;
        process.virtual_mem = vm;
        process.physical_mem = pm;

        system_state.insert_process(process.clone());
        threads
            .into_iter()
            .for_each(|thread| system_state.insert_thread(thread, pid));

        Ok (process)
    }

    pub fn get_threads_for_pid(&self, pid: Pid) -> Result<Vec<Thread>, ParseError> {
        let mut threads = vec![];
        for entry in fs::read_dir(self.get_base_thread_path(pid)).unwrap() {
            let thread_path = entry.unwrap().path();
            if thread_path.is_dir() {
                let thr_path_str = thread_path.display().to_string();
                let tid = extract_tid_from_path(&thr_path_str)?;
                threads.push(Thread::new(tid));
            }
        }

        Ok(threads)
    }

    pub fn get_mem_info(&self, pid: Pid) -> Result<(Vm, Pm), ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/status");
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let buf_reader = BufReader::new(file);
        self.parse_mem_info(buf_reader)
    }

    fn parse_mem_info<R>(&self, reader: R) -> Result<(Vm, Pm), ParseError>
        where R: BufRead,
    {
        let mut vm_size: Option<Vm> = None;
        let mut vm_rss: Option<Pm> = None;

        for line in reader.lines() {
            let line = line.map_err(|err| ParseError::ParsingError(err.to_string()))?;

            if line.starts_with("VmSize:") {
                let n = line
                    .split_whitespace()
                    .nth(1)
                    .ok_or_else(|| ParseError::ParsingError("Could not parse VmSize".into()))?
                    .parse::<u32>()
                    .map_err(|err| ParseError::ParsingError(err.to_string()))?;
                vm_size = Some(n as Vm);
            } else if line.starts_with("VmRSS:") {
                let n = line
                    .split_whitespace()
                    .nth(1)
                    .ok_or_else(|| ParseError::ParsingError("Could not parse VmRSS".into()))?
                    .parse::<u32>()
                    .map_err(|err| ParseError::ParsingError(err.to_string()))?;
                vm_rss = Some(n as Pm);
            }

            if vm_size.is_some() && vm_rss.is_some() {
                break;
            }
        }

        match (vm_size, vm_rss) {
            (Some(vm), Some(pm)) => Ok((vm, pm)),
            _ => Err(ParseError::ParsingError(
                "VmSize or VmRSS not found in status".into(),
            )),
        }
    }

    pub fn get_process_name(&self, pid: Pid) -> Result<String, ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/comm");
        let buf = self.read_entire_file(&file_path)?;
        let normalized = self.normalize_name(&buf);

        Ok(normalized)
    }

    pub fn get_process_cmdline(&self, pid: Pid) -> Result<String, ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/cmdline");
        let buf = self.read_entire_file(&file_path)?;
        let normalized = self.normalize_cmdline(&buf);

        Ok(normalized)
    }

    fn read_entire_file(&self, file_path: &String) -> Result<String, ParseError> {
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let mut buf_reader = BufReader::new(file);
        let mut buf = String::new();
        let _ = buf_reader.read_to_string(&mut buf);

        Ok(buf)
    }

    fn normalize_cmdline(&self, s: &String) -> String {
        s.split("\0")
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn normalize_name(&self, s: &String) -> String {
        s.trim_end_matches("\n").into()
    }

    pub fn get_base_thread_path(&self, pid: Pid) -> String {
        format!("{BASE_PROC_PATH}/{pid}/task")
    }
}
