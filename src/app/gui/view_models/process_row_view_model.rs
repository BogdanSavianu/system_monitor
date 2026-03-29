use system_monitor::{
    dto::ProcessCpuSampleDTO,
    util::{Pid, Pm, Vm},
};

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessRowViewModel {
    pub pid: Pid,
    pub name: String,
    pub cpu_top: f64,
    pub cpu_rel: f64,
    pub virtual_mem: Vm,
    pub physical_mem: Pm,
}

pub fn cpu_rows_from_dtos(samples: &[ProcessCpuSampleDTO]) -> Vec<ProcessRowViewModel> {
    samples
        .iter()
        .map(|sample| ProcessRowViewModel {
            pid: sample.pid,
            name: sample.name.clone(),
            cpu_top: sample.cpu_top,
            cpu_rel: sample.cpu_rel,
            virtual_mem: sample.virtual_mem,
            physical_mem: sample.physical_mem,
        })
        .collect()
}
