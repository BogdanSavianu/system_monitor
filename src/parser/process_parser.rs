use std::fs;
use std::num::ParseIntError;

use crate::process::{Pid, Process};
use crate::state::SystemState;
use crate::thread::Thread;
use crate::util::parser_utils::*;

const BASE_PROC_PATH: &str = "/proc";

pub struct ProcessParser;

impl From<ParseIntError> for ParseError {
    fn from(err: ParseIntError) -> ParseError {
        ParseError::ParsingError(err.to_string())
    }
}

impl ProcessParser {
    pub fn new() -> Self {
        ProcessParser {}
    }

    pub fn parse_process(&self, system_state: &mut SystemState, file_path: &String) -> Result<Process, ParseError> {
        let pid = extract_pid_from_path(file_path)?;
        let process = Process::new(pid);
        let threads = self.get_threads_for_pid(pid)?;

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
                threads.push(Thread::new(tid, ));
            }
        }

        Ok(threads)
    }

    pub fn get_base_thread_path(&self, pid: Pid) -> String {
        format!("{BASE_PROC_PATH}/{pid}/task")
    }
}
