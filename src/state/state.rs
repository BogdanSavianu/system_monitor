use std::collections::HashMap;

use crate::{hashmap, process::Process, thread::Thread, util::types::*};

#[derive(Debug)]
pub struct SystemState {
    pub num_cores: u8,
    pub processes: HashMap<Pid, Process>,
    pub threads: HashMap<Tid, Thread>,
    pub threads_by_pid: HashMap<Pid, Vec<Tid>>,
    pub process_jiffies: HashMap<Pid, u64>,
}

impl SystemState {
    pub fn new() -> Self {
        SystemState {
            num_cores: 0,
            processes: hashmap![],
            threads: hashmap![],
            threads_by_pid: hashmap![],
            process_jiffies: hashmap![],
        }
    }

    pub fn insert_process(&mut self, process: Process) {
        self.processes.insert(process.pid, process);
    }

    pub fn insert_thread(&mut self, thread: Thread, ppid: Pid) {
        if let Some(vec) = self.threads_by_pid.get_mut(&ppid) {
            vec.push(thread.tid);
        } else {
            self.threads_by_pid.insert(ppid, vec![thread.tid]);
        }

        self.threads.insert(thread.tid, thread);
    }

    pub fn get_process(&self, pid: Pid) -> Option<&Process> {
        self.processes.get(&pid)
    }

    pub fn update_jiffies(&mut self, new_jiffies: HashMap<Pid, u64>) {
        self.process_jiffies = new_jiffies;
    }

    pub fn clear_process_snapshot(&mut self) {
        self.processes.clear();
        self.threads.clear();
        self.threads_by_pid.clear();
    }

    pub fn add_jiffies_for_pid(&mut self, pid: Pid, jiffies: u64) {
        self.process_jiffies.insert(pid, jiffies);
    }

    pub fn calculate_cpu_usage(&self, new_jiffies: HashMap<Pid, u64>, total0: u64, total1: u64) -> HashMap<Pid, f64> {
        let d_total = total1.saturating_sub(total0);
        if d_total == 0 {
            return hashmap![];
        }

        let mut usages: HashMap<Pid, f64> = hashmap![];
        for (pid, jiffie) in new_jiffies.iter() {
            if let Some(prev_jiffie) = self.process_jiffies.get(pid) {
                let d_proc = jiffie.saturating_sub(*prev_jiffie);
                let pct_norm = 100.0 * (d_proc as f64) / (d_total as f64);
                usages.insert(*pid, pct_norm);
            } else {
                usages.insert(*pid, 0 as f64);
            }
        }

        usages
    }
}
