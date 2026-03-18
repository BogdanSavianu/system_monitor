use std::collections::HashMap;

use crate::{
    hashmap, model::CpuUsageResultModel, process::Process, thread::Thread, util::types::*,
};

#[derive(Debug)]
pub struct SystemState {
    pub num_cores: u8,
    pub processes: HashMap<Pid, Process>,
    pub threads: HashMap<Tid, Thread>,
    pub threads_by_pid: HashMap<Pid, Vec<Tid>>,
    pub process_jiffies: HashMap<Pid, u64>,
    pub total_proc_cpu_percentage: f64,
    pub thread_jiffies: HashMap<Tid, u64>,
}

impl SystemState {
    pub fn new() -> Self {
        SystemState {
            num_cores: 0,
            processes: hashmap![],
            threads: hashmap![],
            threads_by_pid: hashmap![],
            process_jiffies: hashmap![],
            total_proc_cpu_percentage: 0_f64,
            thread_jiffies: hashmap![],
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

    pub fn update_thread_jiffies(&mut self, new_jiffies: HashMap<Tid, u64>) {
        self.thread_jiffies = new_jiffies;
    }

    pub fn clear_process_snapshot(&mut self) {
        self.processes.clear();
        self.threads.clear();
        self.threads_by_pid.clear();
    }

    pub fn add_jiffies_for_pid(&mut self, pid: Pid, jiffies: u64) {
        self.process_jiffies.insert(pid, jiffies);
    }

    pub fn add_jiffies_for_tid(&mut self, tid: Tid, jiffies: u64) {
        self.thread_jiffies.insert(tid, jiffies);
    }

    pub fn calculate_cpu_usage(
        &self,
        new_jiffies: &HashMap<Pid, u64>,
        total0: u64,
        total1: u64,
    ) -> CpuUsageResultModel {
        let d_total = total1.saturating_sub(total0);
        if d_total == 0 {
            return CpuUsageResultModel::new();
        }

        let mut usages: HashMap<Pid, f64> = hashmap![];
        let mut total_proc_cpu_usage = 0_f64;
        for (pid, jiffie) in new_jiffies.iter() {
            if let Some(prev_jiffie) = self.process_jiffies.get(pid) {
                let d_proc = jiffie.saturating_sub(*prev_jiffie);
                let pct_norm = 100.0 * (d_proc as f64) / (d_total as f64);
                usages.insert(*pid, pct_norm);
                total_proc_cpu_usage += pct_norm;
            } else {
                usages.insert(*pid, 0 as f64);
            }
        }

        CpuUsageResultModel::with_values(usages, total_proc_cpu_usage)
    }

    pub fn calculate_relative_cpu_usage(
        &self,
        usages_norm: &HashMap<Pid, f64>,
        total_proc_cpu_percentage: f64,
    ) -> HashMap<Pid, f64> {
        let mut relative: HashMap<Pid, f64> = hashmap![];
        // f64::EPSILON would have been impractically small
        let epsilon = 1e-9;

        if total_proc_cpu_percentage <= epsilon {
            for pid in usages_norm.keys() {
                relative.insert(*pid, 0.0);
            }
            return relative;
        }

        let scale = 100.0 / total_proc_cpu_percentage;
        for (pid, cpu_norm) in usages_norm.iter() {
            relative.insert(*pid, cpu_norm.max(0.0) * scale);
        }

        relative
    }

    pub fn calculate_thread_cpu_usage(
        &self,
        new_jiffies: &HashMap<Tid, u64>,
        total0: u64,
        total1: u64,
    ) -> HashMap<Tid, f64> {
        let d_total = total1.saturating_sub(total0);
        if d_total == 0 {
            return hashmap![];
        }

        let mut usages: HashMap<Tid, f64> = hashmap![];
        for (tid, jiffie) in new_jiffies.iter() {
            if let Some(prev_jiffie) = self.thread_jiffies.get(tid) {
                let d_thr = jiffie.saturating_sub(*prev_jiffie);
                let pct_norm = 100.0 * (d_thr as f64) / (d_total as f64);
                usages.insert(*tid, pct_norm);
            } else {
                usages.insert(*tid, 0 as f64);
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

        let usage = state.calculate_cpu_usage(&new_jiffies, 1000, 1000);
        assert!(usage.usages_norm.is_empty());
    }

    #[test]
    fn calculate_cpu_usage_computes_pct_for_existing_pid() {
        let mut state = SystemState::new();
        let mut prev_jiffies = HashMap::new();
        prev_jiffies.insert(42_u32, 100_u64);
        state.update_jiffies(prev_jiffies);

        let mut new_jiffies = HashMap::new();
        new_jiffies.insert(42_u32, 150_u64);
        let usage = state.calculate_cpu_usage(&new_jiffies, 1_000, 1_200);

        let val = usage.usages_norm.get(&42_u32).copied().unwrap_or(-1.0);
        // %CPU for this pid should be 25 so subtracting 25 leaves us with 0
        assert!((val - 25.0).abs() < f64::EPSILON);
        assert!((usage.total_proc_cpu_usage - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn calculate_cpu_usage_sets_zero_for_new_pid() {
        let mut state = SystemState::new();
        let mut prev_jiffies = HashMap::new();
        prev_jiffies.insert(1_u32, 10_u64);
        state.update_jiffies(prev_jiffies);

        let mut new_jiffies = HashMap::new();
        new_jiffies.insert(2_u32, 77_u64);
        let usage = state.calculate_cpu_usage(&new_jiffies, 100, 200);

        assert_eq!(usage.usages_norm.get(&2_u32).copied(), Some(0.0));
    }

    #[test]
    fn calculate_cpu_usage_uses_saturating_sub_for_pid_delta() {
        let mut state = SystemState::new();
        let mut prev_jiffies = HashMap::new();
        prev_jiffies.insert(9_u32, 100_u64);
        state.update_jiffies(prev_jiffies);

        let mut new_jiffies = HashMap::new();
        new_jiffies.insert(9_u32, 90_u64);
        let usage = state.calculate_cpu_usage(&new_jiffies, 100, 200);

        assert_eq!(usage.usages_norm.get(&9_u32).copied(), Some(0.0));
    }

    #[test]
    fn calculate_relative_cpu_usage_scales_to_hundred() {
        let state = SystemState::new();
        let mut usages = HashMap::new();
        usages.insert(1_u32, 20.0);
        usages.insert(2_u32, 30.0);

        let relative = state.calculate_relative_cpu_usage(&usages, 50.0);
        assert_eq!(relative.get(&1_u32).copied(), Some(40.0));
        assert_eq!(relative.get(&2_u32).copied(), Some(60.0));
    }
}
