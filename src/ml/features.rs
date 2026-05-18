#[derive(Debug, Clone)]
pub struct FeatureRow {
    pub memory_slope: f64,
    pub memory_delta_mean: f64,
    pub memory_delta_max: f64,
    pub workload_mean: f64,
}

impl FeatureRow {
    pub fn as_vec(&self) -> Vec<f64> {
        vec![
            self.memory_slope,
            self.memory_delta_mean,
            self.memory_delta_max,
            self.workload_mean,
        ]
    }
}
