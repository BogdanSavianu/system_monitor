//! reachability-scan leak analysis: of the heap blocks alive right now, which are
//! unreachable from any pointer? capture the process, walk its heap, mark-sweep.

mod capture;
mod glibc;
mod mark;
mod pac;

pub use capture::{Capture, GcoreCapture, MemMap, MemoryReader, ThreadState};
pub use glibc::GlibcWalker;
pub use mark::MarkSweep;

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::util::Pid;

use super::ProbeError;

/// a single live heap block enumerated by a walker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeapBlock {
    pub addr: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockClass {
    DefinitelyLost,
    PossiblyLost,
    IndirectlyLost,
    StillReachable,
}

impl BlockClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DefinitelyLost => "definitely_lost",
            Self::PossiblyLost => "possibly_lost",
            Self::IndirectlyLost => "indirectly_lost",
            Self::StillReachable => "still_reachable",
        }
    }
}

/// `top_blocks` is the largest few, for the ui and db, same 20 as in mark.rs
#[derive(Debug, Clone, Default)]
pub struct ClassificationGroup {
    pub block_count: usize,
    pub total_bytes: u64,
    pub top_blocks: Vec<ClassifiedBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassifiedBlock {
    pub addr: u64,
    pub size: u64,
    pub class: BlockClass,
}

#[derive(Debug, Clone)]
pub struct ReachabilityReport {
    pub pid: Pid,
    pub name: String,
    pub started_at: SystemTime,
    pub duration: Duration,
    pub allocator: String,
    pub total_blocks: usize,
    pub total_bytes: u64,
    pub definitely_lost: ClassificationGroup,
    pub possibly_lost: ClassificationGroup,
    pub indirectly_lost: ClassificationGroup,
    pub still_reachable: ClassificationGroup,
    /// every block that wasn't still-reachable. the per-class `top_blocks` are
    /// truncated for display, but this keeps the full leaked set so the allocation-scan
    /// join has every block to match against.
    pub all_leaked: Vec<ClassifiedBlock>,
}

/// enumerates the live heap blocks in a snapshot, per allocator.
pub trait AllocatorWalker: Send + Sync {
    /// short name reported in the ui ("glibc", "jemalloc").
    fn name(&self) -> &'static str;

    /// true if this walker recognises the allocator in the snapshot.
    fn supports(&self, snapshot: &ProcessSnapshot) -> bool;

    fn walk(&self, snapshot: &ProcessSnapshot) -> Result<Vec<HeapBlock>, ProbeError>;
}

/// top-level reachability-scan entry point, held by the backend behind an arc.
pub trait ReachabilityProfiler: Send + Sync {
    fn reachability_scan(&self, pid: Pid, name: &str) -> Result<ReachabilityReport, ProbeError>;

    fn is_available(&self) -> bool;
}

/// a frozen view of a process, produced by a capture and read by the walker
/// and mark-sweep.
pub struct ProcessSnapshot {
    pub pid: Pid,
    pub maps: Vec<MemMap>,
    pub threads: Vec<ThreadState>,
    pub mem: Arc<dyn MemoryReader>,
}

impl fmt::Debug for ProcessSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessSnapshot")
            .field("pid", &self.pid)
            .field("maps", &self.maps.len())
            .field("threads", &self.threads.len())
            .finish()
    }
}

/// the default reachability-scan profiler: gcore capture, glibc walker, mark-sweep.
pub struct GcoreReachabilityScanner {
    capture: GcoreCapture,
    walker: GlibcWalker,
}

impl GcoreReachabilityScanner {
    pub fn new() -> Self {
        Self {
            capture: GcoreCapture::new(),
            walker: GlibcWalker::new(),
        }
    }
}

impl Default for GcoreReachabilityScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl ReachabilityProfiler for GcoreReachabilityScanner {
    fn is_available(&self) -> bool {
        self.capture.is_available() && super::has_bpf_or_ptrace_privilege()
    }

    fn reachability_scan(&self, pid: Pid, name: &str) -> Result<ReachabilityReport, ProbeError> {
        let started_at = SystemTime::now();
        let snapshot = self.capture.capture(pid)?;

        if !self.walker.supports(&snapshot) {
            return Err(ProbeError::Unavailable(format!(
                "unsupported allocator: {} only walks glibc heaps",
                self.walker.name()
            )));
        }

        let blocks = self.walker.walk(&snapshot)?;
        let report = MarkSweep::classify(&snapshot, &blocks).build_report(pid, name, started_at);
        Ok(report)
    }
}
