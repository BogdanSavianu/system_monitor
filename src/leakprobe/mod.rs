//! allocation-scan leak analysis: which call sites keep allocating bytes that never get
//! freed. anomaly-detection only sees rss climb; this points at the code responsible.

mod bpftrace;
mod correlate;
mod reachability;
mod symbolize;

pub use bpftrace::{
    BpftraceProfiler, CONTINUOUS_INTERVAL_SECS, ContinuousScanState, PERSIST_EVERY_N_SNAPSHOTS,
    process_continuous_chunk,
};
pub use correlate::{AttributedBlock, attribute_leaks};
pub use reachability::{
    BlockClass, ClassificationGroup, ClassifiedBlock, GcoreReachabilityScanner, HeapBlock,
    ReachabilityProfiler, ReachabilityReport,
};
pub use symbolize::Symbolizer;

use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, SystemTime};

use crate::util::Pid;

/// whether outstanding memory is growing, being reclaimed, or holding flat
/// flat reads as inconclusive since a steady working set looks the same
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeakVerdict {
    Leaking,
    Reclaimed,
    Inconclusive,
}

impl LeakVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Leaking => "leaking",
            Self::Reclaimed => "reclaimed",
            Self::Inconclusive => "inconclusive",
        }
    }
}

/// one allocation call stack with its outstanding bytes and count at scan end
#[derive(Debug, Clone, PartialEq)]
pub struct AllocSite {
    pub stack: Vec<String>,
    pub outstanding_bytes: u64,
    pub outstanding_count: u64,
    pub growth_bytes_per_s: f64,
}

/// result of one allocation-scan scan of a single pid
#[derive(Debug, Clone)]
pub struct AllocScanReport {
    pub pid: Pid,
    pub name: String,
    pub started_at: SystemTime,
    pub duration: Duration,
    pub total_outstanding_bytes: u64,
    pub verdict: LeakVerdict,
    pub sites: Vec<AllocSite>,
    /// addr to stack frames for every allocation allocation-scan saw during its window,
    /// joined against a reachability-scan report to attach backtraces
    /// blocks allocated before the scan started are absent.
    pub alloc_stacks: HashMap<u64, Vec<String>>,
}

#[derive(Debug, Clone)]
pub enum ProbeError {
    /// tool missing or insufficient privileges.
    Unavailable(String),
    /// failed to spawn or wait on the external tool.
    Spawn(String),
    /// the tool ran but produced output we couldn't parse.
    Parse(String),
    /// the tool ran but exited with an error or no usable data.
    Failed(String),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(m) => write!(f, "deep scan unavailable: {m}"),
            Self::Spawn(m) => write!(f, "failed to run profiler: {m}"),
            Self::Parse(m) => write!(f, "failed to parse profiler output: {m}"),
            Self::Failed(m) => write!(f, "profiler failed: {m}"),
        }
    }
}

impl std::error::Error for ProbeError {}

/// shared privilege check: root, or any of CAP_BPF, CAP_PERFMON, CAP_SYS_ADMIN,
/// CAP_SYS_PTRACE. both allocation-scan and reachability-scan need elevated rights, so this is the
/// source of truth
pub(crate) fn has_bpf_or_ptrace_privilege() -> bool {
    if unsafe { libc::geteuid() } == 0 {
        return true;
    }
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return false;
    };
    for line in status.lines() {
        if let Some(hex) = line.strip_prefix("CapEff:") {
            if let Ok(caps) = u64::from_str_radix(hex.trim(), 16) {
                const CAP_SYS_PTRACE: u64 = 1 << 19;
                const CAP_SYS_ADMIN: u64 = 1 << 21;
                const CAP_PERFMON: u64 = 1 << 38;
                const CAP_BPF: u64 = 1 << 39;
                return caps & (CAP_BPF | CAP_PERFMON | CAP_SYS_ADMIN | CAP_SYS_PTRACE) != 0;
            }
            return false;
        }
    }
    false
}

pub trait AllocationProfiler: Send + Sync {
    fn is_available(&self) -> bool;
}
