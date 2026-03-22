use std::collections::HashMap;

use crate::util::{Pid, Tid};

#[derive(Debug, Clone)]
pub struct JiffyUsageModel {
    pub process_jiffies: HashMap<Pid, u64>,
    pub thread_jiffies: HashMap<Tid, u64>,
    pub total_proc_cpu_percentage: f64,
}

impl JiffyUsageModel {
    pub fn new() -> Self {
        JiffyUsageModel {
            process_jiffies: HashMap::new(),
            thread_jiffies: HashMap::new(),
            total_proc_cpu_percentage: 0.0,
        }
    }

    pub fn update_process_jiffies(&mut self, new_jiffies: HashMap<Pid, u64>) {
        self.process_jiffies = new_jiffies;
    }

    pub fn update_thread_jiffies(&mut self, new_jiffies: HashMap<Tid, u64>) {
        self.thread_jiffies = new_jiffies;
    }

    pub fn set_total_proc_cpu_percentage(&mut self, total_proc_cpu_percentage: f64) {
        self.total_proc_cpu_percentage = total_proc_cpu_percentage;
    }
}
