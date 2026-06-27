use std::{fmt::Display, time::SystemTime};
use uuid::Uuid;

use crate::util::{Pid, Pm, Vm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: Pid,
    pub start_time_ticks: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessFingerprint {
    pub executable_path: Option<String>,
    pub cmdline_hash: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistedSampleBatch {
    pub collected_at: SystemTime,
    pub session_id: Uuid,
    pub processes: Vec<PersistedProcessSample>,
    pub network: Vec<PersistedNetworkSample>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistedProcessSample {
    pub identity: ProcessIdentity,
    pub fingerprint: ProcessFingerprint,
    pub name: String,
    pub cmdline: String,
    pub cpu_top: f64,
    pub cpu_rel: f64,
    pub virtual_mem: Vm,
    pub physical_mem: Pm,
    pub thread_count: u32,
    /// ml anomaly verdict at sample time. `None` when the monitor didn't
    /// classify this sample (warming up, model not loaded)
    pub is_anomalous: Option<bool>,
    /// the tick this sample was collected at, distinct from the batch-level
    /// `collected_at` because the accumulator buffers several ticks
    pub collected_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistedNetworkSample {
    pub identity: ProcessIdentity,
    pub tcp_open: u32,
    pub tcp_established: u32,
    pub tcp_listen: u32,
    pub udp_open: u32,
    pub total_sockets: u32,
    pub collected_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredProcessMemoryPoint {
    pub collected_at_ms: i64,
    pub pid: Pid,
    pub name: String,
    pub physical_mem_kb: u64,
    pub cpu_top: f64,
    /// whether the ml model flagged this sample. `false` for pre-migration
    /// rows whose column is `NULL`
    pub is_anomalous: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredReachabilityScan {
    pub id: i64,
    pub pid: Pid,
    pub name: String,
    pub started_at_ms: i64,
    pub duration_s: u64,
    pub allocator: String,
    pub total_blocks: i64,
    pub total_bytes: u64,
    pub definitely_lost_bytes: u64,
    pub possibly_lost_bytes: u64,
    pub indirectly_lost_bytes: u64,
    pub still_reachable_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredLeakedBlock {
    pub addr: u64,
    pub size: u64,
    pub class: String,
    pub stack_text: Option<String>,
}

/// a summary row from continuous allocation-scan capture (about 1/minute),
/// for replay's outstanding-bytes line. per-site detail is not stored
#[derive(Debug, Clone, PartialEq)]
pub struct AllocationSnapshot {
    pub pid: Pid,
    pub name: String,
    pub collected_at_ms: i64,
    pub snapshot_count: u64,
    pub total_outstanding_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AllocationSnapshotRow {
    pub pid: Pid,
    pub name: String,
    pub collected_at_ms: i64,
    pub snapshot_count: u64,
    pub total_outstanding_bytes: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredSessionInfo {
    pub session_id: String,
    pub first_seen_ms: i64,
    pub last_seen_ms: i64,
}

/// a persisted allocation-scan allocation-site scan, independent of the sampling session
#[derive(Debug, Clone, PartialEq)]
pub struct PersistedAllocScan {
    pub pid: Pid,
    pub name: String,
    pub started_at_ms: i64,
    pub duration_s: u64,
    pub verdict: String,
    pub total_outstanding_bytes: u64,
    pub sites: Vec<PersistedAllocSite>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistedAllocSite {
    pub frame_text: String,
    pub outstanding_bytes: u64,
    pub outstanding_count: u64,
    pub growth_bytes_per_s: f64,
}

/// a persisted reachability-scan reachability scan, mirroring `ReachabilityReport` but
/// kept independent of the feature-gated leakprobe types
#[derive(Debug, Clone, PartialEq)]
pub struct PersistedReachabilityScan {
    pub pid: Pid,
    pub name: String,
    pub started_at_ms: i64,
    pub duration_s: u64,
    pub allocator: String,
    pub total_blocks: i64,
    pub total_bytes: u64,
    pub definitely_lost_bytes: u64,
    pub possibly_lost_bytes: u64,
    pub indirectly_lost_bytes: u64,
    pub still_reachable_bytes: u64,
    pub top_blocks: Vec<PersistedLeakedBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistedLeakedBlock {
    pub addr: u64,
    pub size: u64,
    pub class: String,
    /// allocation stack joined from a same-pid allocation-scan capture. `None` for blocks
    /// allocated before that capture started
    pub stack_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StorageError {
    Sqlite(String),
    Io(String),
    InvalidData(String),
}

pub type StorageResult<T> = Result<T, StorageError>;

impl Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(msg) => write!(f, "sqlite error: {}", msg),
            Self::Io(msg) => write!(f, "io error: {}", msg),
            Self::InvalidData(msg) => write!(f, "invalid data: {}", msg),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sqlite(err.to_string())
    }
}
