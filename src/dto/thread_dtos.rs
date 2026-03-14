use crate::util::{Pid, Tid};

#[derive(Debug, Clone)]
pub struct ThreadCpuSampleDTO {
    pub pid: Pid,
    pub tid: Tid,
    pub process_name: String,
    pub cpu_norm: f64,
    pub cpu_top: f64,
}

impl ThreadCpuSampleDTO {
    pub fn new(pid: Pid, tid: Tid, process_name: String, cpu_norm: f64, cpu_top: f64) -> Self {
        ThreadCpuSampleDTO {
            pid,
            tid,
            process_name,
            cpu_norm,
            cpu_top,
        }
    }
}
