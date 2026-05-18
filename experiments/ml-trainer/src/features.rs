#[derive(Debug, Clone)]
pub struct TelemetrySample {
    pub elapsed_s: f64,
    pub workload_kb_this_step: f64,
    pub observed_memory_kb: f64,
    pub label: u8,
}

#[derive(Debug, Clone)]
pub struct FeatureRow {
    pub memory_slope: f64,
    pub memory_delta_mean: f64,
    pub memory_delta_max: f64,
    pub workload_mean: f64,
    pub label: u8,
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

pub fn build_feature_rows(samples: &[TelemetrySample], window: usize) -> Vec<FeatureRow> {
    if window < 2 || samples.len() < window {
        return Vec::new();
    }

    let mut rows = Vec::new();
    for i in (window - 1)..samples.len() {
        let w = &samples[(i + 1 - window)..=i];

        let x0 = w.first().map(|s| s.elapsed_s).unwrap_or(0.0);
        let x1 = w.last().map(|s| s.elapsed_s).unwrap_or(0.0);
        let m0 = w.first().map(|s| s.observed_memory_kb).unwrap_or(0.0);
        let m1 = w.last().map(|s| s.observed_memory_kb).unwrap_or(0.0);

        let observed_memory_slope = if (x1 - x0).abs() < f64::EPSILON {
            0.0
        } else {
            (m1 - m0) / (x1 - x0)
        };

        let mut workload_sum = 0.0;

        let mut observed_delta_sum = 0.0;
        let mut observed_delta_max = 0.0;
        for pair in w.windows(2) {
            let d = (pair[1].observed_memory_kb - pair[0].observed_memory_kb).max(0.0);
            observed_delta_sum += d;
            if d > observed_delta_max {
                observed_delta_max = d;
            }
        }

        for s in w {
            workload_sum += s.workload_kb_this_step;
        }

        let observed_delta_mean = observed_delta_sum / (window - 1) as f64;
        let workload_mean = workload_sum / window as f64;

        rows.push(FeatureRow {
            memory_slope: observed_memory_slope,
            memory_delta_mean: observed_delta_mean,
            memory_delta_max: observed_delta_max,
            workload_mean,
            label: w.last().map(|s| s.label).unwrap_or(0),
        });
    }

    rows
}
