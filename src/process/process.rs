use std::fmt::Display;
use std::fs;

pub use crate::thread::Thread;
use crate::util::{ParseError, extract_tid_from_path};

const BASE_PROC_PATH: &str = "/proc";

#[derive(Debug)]
pub struct Process {
    pub pid: u32,
    pub threads_list: Vec<Thread>
}

impl Process {
    pub fn new(pid: u32) -> Self {
        Process { 
            pid,
            threads_list: vec![]
        }
    }

    pub fn get_base_thread_path(&self) -> String {
        let pid = self.pid;
        format!("{BASE_PROC_PATH}/{pid}/task")
    }

    pub fn collect_threads(&mut self) -> Result<(), ParseError> {
        for entry in fs::read_dir(self.get_base_thread_path()).unwrap() {
            let thread_path = entry.unwrap().path();
            if thread_path.is_dir() {
                let thr_path_str = thread_path.display().to_string();
                let tid = extract_tid_from_path(&thr_path_str)?;
                self.threads_list.push(Thread::new(tid));
            }
        }

        Ok(())
    }

}

impl Display for Process {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, 
"Process id: {:?},
    Thread list: {:?}", 
    self.pid, self.threads_list)
    }
}
