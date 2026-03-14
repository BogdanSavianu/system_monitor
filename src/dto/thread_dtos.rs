use crate::util::{Pid, Tid};

#[derive(Debug, Clone)]
pub struct ThreadCpuSampleDTO {
    pub pid: Pid,
    pub tid: Tid,
    pub process_name: String,
    pub thread_name: String,
    pub cpu_norm: f64,
    pub cpu_top: f64,
    pub state: Option<char>,
    pub last_cpu: Option<u32>,
    pub voluntary_ctxt_switches: Option<u64>,
    pub nonvoluntary_ctxt_switches: Option<u64>,
    pub io_read_bytes: Option<u64>,
    pub io_write_bytes: Option<u64>,
    pub io_rchar: Option<u64>,
    pub io_wchar: Option<u64>,
    pub io_syscr: Option<u64>,
    pub io_syscw: Option<u64>,
}

impl ThreadCpuSampleDTO {
    pub fn new(pid: Pid, tid: Tid, process_name: String, thread_name: String, cpu_norm: f64, cpu_top: f64) -> Self {
        ThreadCpuSampleDTO {
            pid,
            tid,
            process_name,
            thread_name,
            cpu_norm,
            cpu_top,
            state: None,
            last_cpu: None,
            voluntary_ctxt_switches: None,
            nonvoluntary_ctxt_switches: None,
            io_read_bytes: None,
            io_write_bytes: None,
            io_rchar: None,
            io_wchar: None,
            io_syscr: None,
            io_syscw: None,
        }
    }
}
