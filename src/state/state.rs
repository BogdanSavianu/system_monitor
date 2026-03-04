use std::collections::HashMap;

use crate::{hashmap, process::{Pid, Process}, thread::{Thread, Tid}};

#[derive(Debug)]
pub struct SystemState {
    pub processes: HashMap<Pid, Process>,
    pub threads: HashMap<Tid, Thread>,
    pub threads_by_pid: HashMap<Pid, Vec<Tid>>,
}

impl SystemState {
    pub fn new() -> Self {
        SystemState {
            processes: hashmap![],
            threads: hashmap![],
            threads_by_pid: hashmap![],
        }
    }

    pub fn insert_process(&mut self, process: Process) {
        self.processes.insert(process.pid, process);
    }

    pub fn insert_thread(&mut self, thread: Thread, ppid: Pid) {
        if let Some(vec) = self.threads_by_pid.get_mut(&ppid) {
            //dbg!(vec);
            vec.push(thread.tid);
        } else {
            self.threads_by_pid.insert(ppid, vec![thread.tid]);
        }

        self.threads.insert(thread.tid, thread);
    }

    pub fn get_process(&self, pid: Pid) -> Option<&Process> {
        self.processes.get(&pid)
    }
}
