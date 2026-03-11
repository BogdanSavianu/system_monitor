use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};

use crate::model::ProcessStatusFileModel;
use crate::process::Process;
use crate::state::SystemState;
use crate::thread::Thread;
use crate::util::{types::*, parser_utils::*};

const BASE_PROC_PATH: &str = "/proc";

pub trait TraitProcessParser {
    fn parse_process(&self, system_state: &mut SystemState, file_path: &String) -> Result<Process, ParseError>;
    fn get_threads_for_pid(&self, pid: Pid) -> Result<Vec<Thread>, ParseError>;
    fn get_status_info(&self, pid: Pid) -> Result<ProcessStatusFileModel, ParseError>;
    // for now it return utime and stime used for jiffies
    fn get_stat_info(&self, pid: Pid) -> Result<(u64, u64), ParseError>;
    fn get_process_name(&self, pid: Pid) -> Result<String, ParseError>;
    fn get_process_cmdline(&self, pid: Pid) -> Result<String, ParseError>;
}

pub struct ProcessParser;

impl ProcessParser {
    pub fn new() -> Self {
        ProcessParser {}
    }

    fn parse_status_info<R>(&self, reader: R) -> Result<ProcessStatusFileModel, ParseError>
        where R: BufRead,
    {
        let mut vm_size: Option<Vm> = None;
        let mut pm_size: Option<Pm> = None;
        let mut swap_size: Option<Pm> = None;
        let mut thread_count: Option<u32> = None;

        for line in reader.lines() {
            let line = line.map_err(|err| ParseError::ParsingError(err.to_string()))?;
            let mut parts = line.split_whitespace();
            let key = parts.next();
            let value = parts.next();

            match (key, value) {
                (Some("VmSize:"), Some(val)) => {
                    vm_size = Some(
                        val.parse::<Vm>()
                            .map_err(|err| ParseError::ParsingError(err.to_string()))? as Vm
                    );
                }

                (Some("VmRSS:"), Some(val)) => {
                    pm_size = Some(
                        val.parse::<Pm>()
                            .map_err(|err| ParseError::ParsingError(err.to_string()))? as Pm
                    );
                }

                (Some("VmSwap:"), Some(val)) => {
                    swap_size = Some(
                        val.parse::<Swap>()
                            .map_err(|err| ParseError::ParsingError(err.to_string()))? as Swap
                    );
                }

                (Some("Threads:"), Some(val)) => {
                    thread_count = Some(
                        val.parse::<u32>()
                            .map_err(|err| ParseError::ParsingError(err.to_string()))?
                    );
                }

                _ => {}
            }

            if vm_size.is_some() && pm_size.is_some() && swap_size.is_some() && thread_count.is_some() {
                break;
            }

        }

        match (vm_size, pm_size, swap_size, thread_count) {
            (Some(vm), Some(pm), Some(swap), Some(th_count)) 
                => Ok(ProcessStatusFileModel::new(vm, pm, swap, th_count)),
            _ => Err(ParseError::ParsingError(
                "VmSize or VmRSS not found in status".into(),
            )),
        }
    }

    fn read_entire_file(&self, file_path: &String) -> Result<String, ParseError> {
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let mut buf_reader = BufReader::new(file);
        let mut buf = String::new();
        let _ = buf_reader.read_to_string(&mut buf);

        Ok(buf)
    }

    fn parse_stat_info<R>(&self, mut buf_reader: R) -> Result<(u64, u64), ParseError> 
        where R: BufRead,
    {
        let mut content = String::new();
        let size_read = buf_reader
            .read_to_string(&mut content)
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;
        if size_read == 0 {
            return Err(ParseError::ParsingError("Stat file has 0 bytes".to_string()));
        }

        let content = content.trim();

        // /proc/<pid>/stat format starts with: "pid (comm) state ..."
        let comm_start = content
            .find('(')
            .ok_or_else(|| ParseError::ParsingError("Stat file has wrong format".to_string()))?;

        // comm is unpredictable since it is the command line and can contain ')' itself
        // that is why I use rfind to find its last appearance
        let comm_end = content
            .rfind(") ")
            .ok_or_else(|| ParseError::ParsingError("Stat file has wrong format".to_string()))?;

        if comm_end <= comm_start {
            return Err(ParseError::ParsingError("Stat file has wrong format".to_string()));
        }

        // skip state and ppid
        let after_comm = &content[(comm_end + 2)..];
        let fields: Vec<&str> = after_comm.split_whitespace().collect();

        // relative to field 3: field14(utime) => idx 11, field15(stime) => idx 12
        if fields.len() <= 12 {
            return Err(ParseError::ParsingError(
                "Stat file has too few fields".to_string(),
            ));
        }

        let utime = fields[11]
            .parse::<u64>()
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let stime = fields[12]
            .parse::<u64>()
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;

        Ok((utime, stime))
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

    fn get_base_thread_path(&self, pid: Pid) -> String {
        format!("{BASE_PROC_PATH}/{pid}/task")
    }
}

impl TraitProcessParser for ProcessParser {
    fn parse_process(&self, system_state: &mut SystemState, file_path: &String) -> Result<Process, ParseError> {
        let pid = extract_pid_from_path(file_path)?;
        let mut process = Process::new(pid);
        let name = self.get_process_name(pid)?;
        let cmdline = self.get_process_cmdline(pid)?;
        let threads = self.get_threads_for_pid(pid)?;
        let statuf_file_model = self.get_status_info(pid)?;

        process.name = name;
        process.cmdline = cmdline;
        process.virtual_mem = statuf_file_model.virtual_mem;
        process.physical_mem = statuf_file_model.physical_mem;
        process.swap_mem = statuf_file_model.swap_mem;
        process.thread_count = statuf_file_model.thread_count;

        system_state.insert_process(process.clone());
        threads
            .into_iter()
            .for_each(|thread| system_state.insert_thread(thread, pid));

        Ok (process)
    }

    fn get_threads_for_pid(&self, pid: Pid) -> Result<Vec<Thread>, ParseError> {
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

    fn get_status_info(&self, pid: Pid) -> Result<ProcessStatusFileModel, ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/status");
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let buf_reader = BufReader::new(file);
        self.parse_status_info(buf_reader)
    }

    fn get_stat_info(&self, pid: Pid) -> Result<(u64, u64), ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/stat");
        let file = File::open(file_path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let buf_reader = BufReader::new(file);
        self.parse_stat_info(buf_reader)
    }

    fn get_process_name(&self, pid: Pid) -> Result<String, ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/comm");
        let buf = self.read_entire_file(&file_path)?;
        let normalized = self.normalize_name(&buf);

        Ok(normalized)
    }

    fn get_process_cmdline(&self, pid: Pid) -> Result<String, ParseError> {
        let file_path = format!("{BASE_PROC_PATH}/{pid}/cmdline");
        let buf = self.read_entire_file(&file_path)?;
        let normalized = self.normalize_cmdline(&buf);

        Ok(normalized)
    }
}
