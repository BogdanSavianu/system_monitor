#[derive(Debug)]
pub struct SystemStatusFileModel {
    pub total_cpu: u64,
    pub cpus: Vec<u64>,
    pub num_cores: u8,
}

impl SystemStatusFileModel {
    pub fn new() -> Self {
        SystemStatusFileModel {
            total_cpu: 0,
            cpus: vec![],
            num_cores: 0,
        }
    }

    pub fn build(total_cpu: u64, cpus: Vec<u64>, num_cores: u8) -> Self {
        SystemStatusFileModel {
            total_cpu,
            cpus,
            num_cores,
        }
    }
}
