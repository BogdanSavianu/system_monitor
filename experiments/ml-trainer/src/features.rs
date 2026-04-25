#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureSet {
    Full,
    Realistic,
}

impl FeatureSet {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "full" => Some(Self::Full),
            "realistic" => Some(Self::Realistic),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Realistic => "realistic",
        }
    }
}

#[derive(Debug, Clone)]
pub struct TelemetrySample {
    pub elapsed_s: f64,
    pub leaked_kb_step: f64,
    pub leaked_kb_total: f64,
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
    pub leak_ratio: f64,
    pub label: u8,
}

impl FeatureRow {
    pub fn as_vec(&self, feature_set: FeatureSet) -> Vec<f64> {
        match feature_set {
            FeatureSet::Full => vec![
                self.memory_slope,
                self.memory_delta_mean,
                self.memory_delta_max,
                self.workload_mean,
                self.leak_ratio,
            ],
            FeatureSet::Realistic => vec![
                self.memory_slope,
                self.memory_delta_mean,
                self.memory_delta_max,
                self.workload_mean,
            ],
        }
    }
}

pub fn build_feature_rows(
    samples: &[TelemetrySample],
    window: usize,
    feature_set: FeatureSet,
) -> Vec<FeatureRow> {
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
        let m0 = w.first().map(|s| s.observed_memory_kb).unwrap_or(0.0);
        let m1 = w.last().map(|s| s.observed_memory_kb).unwrap_or(0.0);

        let leak_memory_slope = if (x1 - x0).abs() < f64::EPSILON {
            0.0
        } else {
            (y1 - y0) / (x1 - x0)
        };
        let observed_memory_slope = if (x1 - x0).abs() < f64::EPSILON {
            0.0
        } else {
            (m1 - m0) / (x1 - x0)
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
            leak_step_sum += s.leaked_kb_step;
        }

        let leak_delta_mean = delta_sum / (window - 1) as f64;
        let observed_delta_mean = observed_delta_sum / (window - 1) as f64;
        let workload_mean = workload_sum / window as f64;
        let leak_ratio = if workload_sum <= 0.0 {
            0.0
        } else {
            leak_step_sum / workload_sum
        };

        let (memory_slope, memory_delta_mean, memory_delta_max) = match feature_set {
            FeatureSet::Full => (leak_memory_slope, leak_delta_mean, delta_max),
            FeatureSet::Realistic => {
                (observed_memory_slope, observed_delta_mean, observed_delta_max)
            }
        };

        rows.push(FeatureRow {
            memory_slope,
            memory_delta_mean,
            memory_delta_max,
            workload_mean,
            leak_ratio,
            label: w.last().map(|s| s.label).unwrap_or(0),
        });
    }

    rows
}
