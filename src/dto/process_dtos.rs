use crate::util::Pid;

#[derive(Debug, Clone)]
pub struct ProcessCpuSampleDTO {
    pub pid: Pid,
    pub name: String,
    pub cpu_norm: f64,
    pub cpu_top: f64,
    pub cpu_rel: f64,
}

impl ProcessCpuSampleDTO {
    pub fn new() -> Self {
        ProcessCpuSampleDTO { 
            pid: 0,
            name: "".into(),
            cpu_norm: 0_f64,
            cpu_top: 0_f64,
            cpu_rel: 0_f64,
        }
    }

    pub fn with_values(pid: Pid, name: String, cpu_norm: f64, cpu_top: f64, cpu_rel: f64) -> Self {
        ProcessCpuSampleDTO {
            pid, name, cpu_norm, cpu_top, cpu_rel,
        }
    }
}
