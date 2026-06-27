#[derive(Debug, Clone, PartialEq)]
pub struct ReplaySample {
    pub collected_at_ms: i64,
    pub physical_mem_mb: f64,
    pub cpu_percent: f64,
    pub is_anomalous: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayScanMarker {
    pub id: i64,
    pub pid: u32,
    pub started_at_ms: i64,
    pub total_blocks: i64,
    pub total_lost_bytes: u64,
    pub definitely_lost_bytes: u64,
    pub possibly_lost_bytes: u64,
    pub indirectly_lost_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayLeakedBlock {
    pub addr: u64,
    pub size: u64,
    pub class: String,
    pub stack_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayAllocationPoint {
    pub collected_at_ms: i64,
    pub outstanding_mb: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaySource {
    None,
    SqliteByName,
}
