use std::collections::HashMap;

use system_monitor::{
    dto::ProcessCpuSampleDTO,
    process::ProcessState,
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
    pub is_anomalous: bool,
    pub state: ProcessState,
    pub swap_mem: u32,
    pub fd_count: u32,
    pub disk_read_kb_s: f64,
    pub disk_write_kb_s: f64,
    pub username: String,
}

pub fn cpu_rows_from_dtos(
    samples: &[ProcessCpuSampleDTO],
    anomaly_by_pid: &HashMap<Pid, bool>,
    username_by_pid: &HashMap<Pid, String>,
) -> Vec<ProcessRowViewModel> {
    samples
        .iter()
        .map(|sample| ProcessRowViewModel {
            pid: sample.pid,
            name: sample.name.clone(),
            cpu_top: sample.cpu_top,
            cpu_rel: sample.cpu_rel,
            virtual_mem: sample.virtual_mem,
            physical_mem: sample.physical_mem,
            is_anomalous: anomaly_by_pid.get(&sample.pid).copied().unwrap_or(false),
            state: sample.state,
            swap_mem: sample.swap_mem,
            fd_count: sample.fd_count,
            disk_read_kb_s: sample.disk_read_kb_s,
            disk_write_kb_s: sample.disk_write_kb_s,
            username: username_by_pid
                .get(&sample.pid)
                .cloned()
                .unwrap_or_default(),
        })
        .collect()
}
