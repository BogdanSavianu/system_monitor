#[derive(Debug)]
pub struct SystemStatusFileModel {
    pub total_cpu: u64,
    pub cpus: Vec<u64>,
}

impl SystemStatusFileModel {
    pub fn new() -> Self {
        SystemStatusFileModel { 
            total_cpu: 0,
            cpus: vec![],
        }
    }

    pub fn build(total_cpu: u64, cpus: Vec<u64>) -> Self {
        SystemStatusFileModel { total_cpu, cpus }
    }
}
