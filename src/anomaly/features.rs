#[derive(Debug, Clone)]
pub struct TelemetrySample {
    pub elapsed_s: f64,
    pub leaked_kb_step: f64,
    pub leaked_kb_total: f64,
    pub workload_kb_this_step: f64,
    pub label: u8,
}

#[derive(Debug, Clone)]
pub struct FeatureRow {
    pub memory_slope: f64,
    pub memory_delta_mean: f64,
    pub memory_delta_max: f64,
    pub workload_mean: f64,
    pub leak_ratio: f64,
    pub label: u8,
}

impl FeatureRow {
    pub fn as_vec(&self) -> Vec<f64> {
        vec![
            self.memory_slope,
            self.memory_delta_mean,
            self.memory_delta_max,
            self.workload_mean,
            self.leak_ratio,
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
        let y0 = w.first().map(|s| s.leaked_kb_total).unwrap_or(0.0);
        let y1 = w.last().map(|s| s.leaked_kb_total).unwrap_or(0.0);

        let memory_slope = if (x1 - x0).abs() < f64::EPSILON {
            0.0
        } else {
            (y1 - y0) / (x1 - x0)
        };

        let mut delta_sum = 0.0;
        let mut delta_max = 0.0;
        let mut workload_sum = 0.0;
        let mut leak_step_sum = 0.0;

        for pair in w.windows(2) {
            let d = (pair[1].leaked_kb_total - pair[0].leaked_kb_total).max(0.0);
            delta_sum += d;
            if d > delta_max {
                delta_max = d;
            }
        }

        for s in w {
            workload_sum += s.workload_kb_this_step;
            leak_step_sum += s.leaked_kb_step;
        }

        let memory_delta_mean = delta_sum / (window - 1) as f64;
        let workload_mean = workload_sum / window as f64;
        let leak_ratio = if workload_sum <= 0.0 {
            0.0
        } else {
            leak_step_sum / workload_sum
        };

        rows.push(FeatureRow {
            memory_slope,
            memory_delta_mean,
            memory_delta_max: delta_max,
            workload_mean,
            leak_ratio,
            label: w.last().map(|s| s.label).unwrap_or(0),
        });
    }

    rows
}
