use crate::process::ProcessState;
use crate::util::{Pid, Pm, Vm};

#[derive(Debug, Clone)]
pub struct ProcessCpuSampleDTO {
    pub pid: Pid,
    pub name: String,
    pub cpu_norm: f64,
    pub cpu_top: f64,
    pub cpu_rel: f64,
    pub virtual_mem: Vm,
    pub physical_mem: Pm,
    pub state: ProcessState,
    pub swap_mem: u32,
    pub fd_count: u32,
    pub disk_read_kb_s: f64,
    pub disk_write_kb_s: f64,
}

impl ProcessCpuSampleDTO {
    pub fn new() -> Self {
        ProcessCpuSampleDTO {
            pid: 0,
            name: "".into(),
            cpu_norm: 0_f64,
            cpu_top: 0_f64,
            cpu_rel: 0_f64,
            virtual_mem: 0,
            physical_mem: 0,
            state: ProcessState::Sleeping,
            swap_mem: 0,
            fd_count: 0,
            disk_read_kb_s: 0.0,
            disk_write_kb_s: 0.0,
        }
    }
}
