use super::types::{ReplayAllocationPoint, ReplaySample};

pub fn visible_samples_until_cursor(
    samples: &[ReplaySample],
    cursor_ms: Option<i64>,
) -> &[ReplaySample] {
    let Some(cursor_ms) = cursor_ms else {
        return &[];
    };
    let end = samples.partition_point(|s| s.collected_at_ms <= cursor_ms);
    &samples[..end]
}

pub fn visible_memory_points_until_cursor(
    samples: &[ReplaySample],
    cursor_ms: Option<i64>,
) -> Vec<(i64, f64, bool)> {
    let Some(cursor_ms) = cursor_ms else {
        return Vec::new();
    };
    samples
        .iter()
        .take_while(|s| s.collected_at_ms <= cursor_ms)
        .map(|s| (s.collected_at_ms, s.physical_mem_mb, s.is_anomalous))
        .collect()
}

pub fn visible_cpu_points_until_cursor(
    samples: &[ReplaySample],
    cursor_ms: Option<i64>,
) -> Vec<(i64, f64, bool)> {
    let Some(cursor_ms) = cursor_ms else {
        return Vec::new();
    };
    samples
        .iter()
        .take_while(|s| s.collected_at_ms <= cursor_ms)
        .map(|s| (s.collected_at_ms, s.cpu_percent, false))
        .collect()
}

pub fn visible_allocation_points_until_cursor(
    series: &[ReplayAllocationPoint],
    cursor_ms: Option<i64>,
) -> Vec<(i64, f64, bool)> {
    let Some(cursor_ms) = cursor_ms else {
        return Vec::new();
    };
    series
        .iter()
        .take_while(|p| p.collected_at_ms <= cursor_ms)
        .map(|p| (p.collected_at_ms, p.outstanding_mb, false))
        .collect()
}
