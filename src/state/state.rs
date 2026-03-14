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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::SystemState;

    #[test]
    fn calculate_cpu_usage_returns_empty_when_total_delta_is_zero() {
        let state = SystemState::new();
        let mut new_jiffies = HashMap::new();
        new_jiffies.insert(1_u32, 120_u64);

        let usage = state.calculate_cpu_usage(new_jiffies, 1000, 1000);
        assert!(usage.is_empty());
    }

    #[test]
    fn calculate_cpu_usage_computes_pct_for_existing_pid() {
        let mut state = SystemState::new();
        let mut prev_jiffies = HashMap::new();
        prev_jiffies.insert(42_u32, 100_u64);
        state.update_jiffies(prev_jiffies);

        let mut new_jiffies = HashMap::new();
        new_jiffies.insert(42_u32, 150_u64);
        let usage = state.calculate_cpu_usage(new_jiffies, 1_000, 1_200);

        let val = usage.get(&42_u32).copied().unwrap_or(-1.0);
        // %CPU for this pid should be 25 so subtracting 25 leaves us with 0
        assert!((val - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn calculate_cpu_usage_sets_zero_for_new_pid() {
        let mut state = SystemState::new();
        let mut prev_jiffies = HashMap::new();
        prev_jiffies.insert(1_u32, 10_u64);
        state.update_jiffies(prev_jiffies);

        let mut new_jiffies = HashMap::new();
        new_jiffies.insert(2_u32, 77_u64);
        let usage = state.calculate_cpu_usage(new_jiffies, 100, 200);

        assert_eq!(usage.get(&2_u32).copied(), Some(0.0));
    }

    #[test]
    fn calculate_cpu_usage_uses_saturating_sub_for_pid_delta() {
        let mut state = SystemState::new();
        let mut prev_jiffies = HashMap::new();
        prev_jiffies.insert(9_u32, 100_u64);
        state.update_jiffies(prev_jiffies);

        let mut new_jiffies = HashMap::new();
        new_jiffies.insert(9_u32, 90_u64);
        let usage = state.calculate_cpu_usage(new_jiffies, 100, 200);

        assert_eq!(usage.get(&9_u32).copied(), Some(0.0));
    }
}
