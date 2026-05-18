#[derive(Debug, Clone, Copy, Default)]
pub struct BinaryMetrics {
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
}

pub fn binary_metrics(y_true: &[u8], y_pred: &[u8]) -> BinaryMetrics {
    if y_true.is_empty() || y_true.len() != y_pred.len() {
        return BinaryMetrics::default();
    }

    let mut tp: f64 = 0.0;
    let mut tn: f64 = 0.0;
    let mut fp: f64 = 0.0;
    let mut fn_: f64 = 0.0;

    for (t, p) in y_true.iter().zip(y_pred.iter()) {
        match (*t, *p) {
            (1, 1) => tp += 1.0,
            (0, 0) => tn += 1.0,
            (0, 1) => fp += 1.0,
            (1, 0) => fn_ += 1.0,
            _ => {}
        }
    }

    let accuracy = (tp + tn) / (tp + tn + fp + fn_).max(1.0_f64);
    let precision = tp / (tp + fp).max(1.0_f64);
    let recall = tp / (tp + fn_).max(1.0_f64);
    let f1 = if (precision + recall).abs() < f64::EPSILON {
        0.0
    } else {
        2.0 * precision * recall / (precision + recall)
    };

    BinaryMetrics {
        accuracy,
        precision,
        recall,
        f1,
    }
}
