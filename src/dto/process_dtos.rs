use crate::util::Pid;

#[derive(Debug, Clone)]
pub struct ProcessCpuSampleDTO {
    pub pid: Pid,
    pub name: String,
    pub cpu_norm: f64,
    pub cpu_top: f64,
}

impl ProcessCpuSampleDTO {
    pub fn new(pid: Pid, name: String, cpu_norm: f64, cpu_top: f64) -> Self {
        ProcessCpuSampleDTO {
            pid, name, cpu_norm, cpu_top,
        }
    }
}
