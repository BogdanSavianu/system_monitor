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
        }
    }

    pub fn with_values(
        pid: Pid,
        name: String,
        cpu_norm: f64,
        cpu_top: f64,
        cpu_rel: f64,
        virtual_mem: Vm,
        physical_mem: Pm,
    ) -> Self {
        ProcessCpuSampleDTO {
            pid,
            name,
            cpu_norm,
            cpu_top,
            cpu_rel,
            virtual_mem,
            physical_mem,
        }
    }
}
