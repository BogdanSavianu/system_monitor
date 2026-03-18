use std::collections::HashMap;

use crate::{hashmap, util::Pid};

pub struct CpuUsageResultModel {
    pub usages_norm: HashMap<Pid, f64>,
    pub total_proc_cpu_usage: f64,
}

impl CpuUsageResultModel {
    pub fn new() -> CpuUsageResultModel {
        CpuUsageResultModel {
            usages_norm: hashmap![],
            total_proc_cpu_usage: 0_f64,
        }
    }

    pub fn with_values(
        usages_norm: HashMap<Pid, f64>,
        total_proc_cpu_usage: f64,
    ) -> CpuUsageResultModel {
        CpuUsageResultModel {
            usages_norm,
            total_proc_cpu_usage,
        }
    }
}
