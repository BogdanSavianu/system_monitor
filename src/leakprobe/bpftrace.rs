//! the allocation-scan profiler. a bpftrace child tracks outstanding (unfreed) bytes per
//! allocation stack and prints a snapshot every few seconds until it's stopped.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime};

use tracing::{debug, warn};

use crate::util::Pid;

use super::{AllocScanReport, AllocSite, AllocationProfiler, LeakVerdict, ProbeError};

const BPFTRACE_BINARIES: &[&str] = &["bpftrace"];
const EXTRA_BIN_DIRS: &[&str] = &["/usr/sbin", "/sbin", "/usr/local/sbin", "/usr/bin", "/bin"];

/// floor below which a growing site is "inconclusive" rather than "leaking"
/// set under the smallest leaker we test (~630 KB) and above engine churn
const MIN_LEAK_BYTES: u64 = 256 * 1024;

/// outstanding bytes must grow by at least this factor for a "leaking" verdict.
const GROWTH_RATIO: f64 = 1.25;

pub struct BpftraceProfiler {
    binary: Option<PathBuf>,
}

impl BpftraceProfiler {
    pub fn new() -> Self {
        Self {
            binary: resolve_in_path(BPFTRACE_BINARIES),
        }
    }
}

impl Default for BpftraceProfiler {
    fn default() -> Self {
        Self::new()
    }
}

/// default seconds between snapshot prints in continuous mode
pub const CONTINUOUS_INTERVAL_SECS: u64 = 5;

/// persist only every nth snapshot to sqlite. at the default interval this is
/// about one row per minute per pid, instead of twelve
pub const PERSIST_EVERY_N_SNAPSHOTS: usize = 12;

const MAX_ATTACH_ATTEMPTS: u32 = 4;

impl AllocationProfiler for BpftraceProfiler {
    fn is_available(&self) -> bool {
        self.binary.is_some() && has_bpf_privilege()
    }
}

impl BpftraceProfiler {
    /// spawns a long-running bpftrace child attached to `pid` with stdout piped
    /// the caller reads snapshots off it. returns the child and a symbolizer
    /// built from the target's /proc state at attach time
    pub fn start_continuous(
        &self,
        pid: Pid,
        interval_secs: u64,
    ) -> Result<(Child, super::Symbolizer), ProbeError> {
        let Some(binary) = self.binary.as_ref() else {
            return Err(ProbeError::Unavailable(
                "bpftrace not found on PATH".to_string(),
            ));
        };
        if !has_bpf_privilege() {
            return Err(ProbeError::Unavailable(
                "needs root or CAP_BPF/CAP_SYS_ADMIN".to_string(),
            ));
        }
        let Some(libc) = resolve_libc_path(pid) else {
            return Err(ProbeError::Failed(format!(
                "could not find libc mapping for pid {pid}"
            )));
        };

        let interval = interval_secs.max(1);
        let script = build_script_continuous(&libc.to_string_lossy(), interval);

        if std::env::var_os("LEAKPROBE_DEBUG").is_some() {
            eprintln!("[leakprobe] bpftrace -p {pid} (continuous, interval={interval}s)");
        }
        debug!(target: "leakprobe::bpftrace", pid, interval, "starting continuous capture");

        // uprobe attach is racy, so retry on the immediate-fail path
        let mut last_err = String::from("bpftrace failed to start");
        for attempt in 1..=MAX_ATTACH_ATTEMPTS {
            let child = Command::new(binary)
                .env("BPFTRACE_MAP_KEYS_MAX", "65536")
                .arg("-p")
                .arg(pid.to_string())
                .arg("-e")
                .arg(&script)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();
            let mut child = match child {
                Ok(c) => c,
                Err(e) => return Err(ProbeError::Spawn(e.to_string())),
            };
            // spawning succeeds even when the attach will fail, so wait briefly
            // if the child already exited, capture stderr and retry.
            std::thread::sleep(Duration::from_millis(400));
            match child.try_wait() {
                Ok(Some(status)) => {
                    let mut stderr = String::new();
                    if let Some(mut e) = child.stderr.take() {
                        use std::io::Read;
                        let _ = e.read_to_string(&mut stderr);
                    }
                    last_err = first_error_line(&stderr);
                    let retry = stderr.contains("attach") || stderr.contains("Invalid argument");
                    warn!(
                        target: "leakprobe::bpftrace",
                        pid, attempt, status = ?status, error = %last_err,
                        "bpftrace exited immediately"
                    );
                    if retry && attempt < MAX_ATTACH_ATTEMPTS {
                        std::thread::sleep(Duration::from_millis(300));
                        continue;
                    }
                    return Err(ProbeError::Failed(last_err));
                }
                Ok(None) => {
                    // still running, so bpftrace is attached and ticking.
                    let symbolizer = super::Symbolizer::from_pid(pid);
                    return Ok((child, symbolizer));
                }
                Err(e) => return Err(ProbeError::Spawn(e.to_string())),
            }
        }
        Err(ProbeError::Failed(last_err))
    }
}

/// bpftrace program that records the size and stack of each allocation (malloc,
/// calloc, realloc and direct mmap) keyed by the returned pointer, subtracts on
/// free/munmap, and prints every map each interval with no exit(), so it runs
/// until killed. realloc is treated as a free of the old pointer plus an
/// allocation of the new one. a per-thread `@inalloc` flag marks time spent
/// inside the heap allocator so that mmap/munmap calls glibc makes internally
/// (large mallocs go through mmap) are not double-counted; only direct mmap from
/// user code is recorded. the `--- END_SNAPSHOT ---` marker delimits one chunk
/// for the reader thread.
pub(crate) fn build_script_continuous(libc: &str, interval: u64) -> String {
    format!(
        "\
uprobe:{libc}:malloc {{ @sz[tid] = arg0; @inalloc[tid] = 1; }}
uretprobe:{libc}:malloc /@sz[tid]/ {{
    @stack[retval] = ustack(8);
    @bytes[retval] = @sz[tid];
    @out[ustack(8)] = sum((int64)@sz[tid]);
    @cnt[ustack(8)] = sum(1);
    delete(@sz[tid]);
    delete(@inalloc[tid]);
}}
uprobe:{libc}:calloc {{ @sz[tid] = arg0 * arg1; @inalloc[tid] = 1; }}
uretprobe:{libc}:calloc /@sz[tid]/ {{
    @stack[retval] = ustack(8);
    @bytes[retval] = @sz[tid];
    @out[ustack(8)] = sum((int64)@sz[tid]);
    @cnt[ustack(8)] = sum(1);
    delete(@sz[tid]);
    delete(@inalloc[tid]);
}}
uprobe:{libc}:realloc {{ @rptr[tid] = arg0; @rsz[tid] = arg1; @inalloc[tid] = 1; }}
uretprobe:{libc}:realloc {{
    if (@bytes[@rptr[tid]]) {{
        @out[@stack[@rptr[tid]]] = sum(0 - (int64)@bytes[@rptr[tid]]);
        @cnt[@stack[@rptr[tid]]] = sum(-1);
        delete(@stack[@rptr[tid]]);
        delete(@bytes[@rptr[tid]]);
    }}
    if (retval != 0 && @rsz[tid] > 0) {{
        @stack[retval] = ustack(8);
        @bytes[retval] = @rsz[tid];
        @out[ustack(8)] = sum((int64)@rsz[tid]);
        @cnt[ustack(8)] = sum(1);
    }}
    delete(@rptr[tid]);
    delete(@rsz[tid]);
    delete(@inalloc[tid]);
}}
uprobe:{libc}:free {{
    @inalloc[tid] = 1;
    if (@bytes[arg0]) {{
        @out[@stack[arg0]] = sum(0 - (int64)@bytes[arg0]);
        @cnt[@stack[arg0]] = sum(-1);
        delete(@stack[arg0]);
        delete(@bytes[arg0]);
    }}
}}
uretprobe:{libc}:free {{ delete(@inalloc[tid]); }}
uprobe:{libc}:mmap /!@inalloc[tid]/ {{ @msz[tid] = arg1; }}
uretprobe:{libc}:mmap /@msz[tid]/ {{
    if (retval != 0 && retval != (uint64)-1) {{
        @stack[retval] = ustack(8);
        @bytes[retval] = @msz[tid];
        @out[ustack(8)] = sum((int64)@msz[tid]);
        @cnt[ustack(8)] = sum(1);
    }}
    delete(@msz[tid]);
}}
uprobe:{libc}:munmap /!@inalloc[tid] && @bytes[arg0]/ {{
    @out[@stack[arg0]] = sum(0 - (int64)@bytes[arg0]);
    @cnt[@stack[arg0]] = sum(-1);
    delete(@stack[arg0]);
    delete(@bytes[arg0]);
}}
interval:s:{interval} {{
    @ticks = @ticks + 1;
    printf(\"=== SNAPSHOT %d ===\\n\", @ticks);
    printf(\"--- BYTES ---\\n\");
    print(@out);
    printf(\"--- COUNTS ---\\n\");
    print(@cnt);
    printf(\"--- ALLOC_STACKS ---\\n\");
    print(@stack);
    printf(\"--- END_SNAPSHOT ---\\n\");
}}
END {{ clear(@sz); clear(@msz); clear(@inalloc); clear(@rptr); clear(@rsz); clear(@stack); clear(@bytes); clear(@out); clear(@cnt); clear(@ticks); }}
"
    )
}

#[derive(Debug, Clone)]
pub(crate) struct RawSite {
    bytes: u64,
    count: u64,
    frames: Vec<String>,
}

pub(crate) type Snapshot = Vec<RawSite>;

/// running state for a continuous capture. keeps the per-snapshot totals so the
/// verdict reads growth across the whole capture
pub struct ContinuousScanState {
    pid: Pid,
    name: String,
    started_at: SystemTime,
    interval_secs: f64,
    snapshot_count: usize,
    previous_snapshot: Option<Snapshot>,
    totals_history: Vec<u64>,
}

impl ContinuousScanState {
    pub fn new(pid: Pid, name: String, interval_secs: u64) -> Self {
        Self {
            pid,
            name,
            started_at: SystemTime::now(),
            interval_secs: (interval_secs as f64).max(1.0),
            snapshot_count: 0,
            previous_snapshot: None,
            totals_history: Vec::new(),
        }
    }

    pub fn pid(&self) -> Pid {
        self.pid
    }
    pub fn snapshot_count(&self) -> usize {
        self.snapshot_count
    }
}

/// parses one `--- END_SNAPSHOT ---` chunk into the latest report and updates
/// `state` in place (snapshot counter, totals history, previous snapshot)
pub fn process_continuous_chunk(
    chunk: &str,
    state: &mut ContinuousScanState,
    symbolizer: &mut super::Symbolizer,
) -> Option<AllocScanReport> {
    let (snapshots, alloc_stacks) = parse_snapshots(chunk);
    let current = snapshots.into_iter().next()?;
    let current_total: u64 = current.iter().map(|s| s.bytes).sum();
    state.totals_history.push(current_total);
    state.snapshot_count = state.snapshot_count.saturating_add(1);

    let alloc_stacks = symbolizer.enrich_alloc_stacks(alloc_stacks);

    let snapshots_for_build = match state.previous_snapshot.as_ref() {
        Some(prev) => vec![prev.clone(), current.clone()],
        None => vec![current.clone()],
    };
    let duration = SystemTime::now()
        .duration_since(state.started_at)
        .unwrap_or(Duration::ZERO);

    let mut report = build_report(
        state.pid,
        state.name.clone(),
        state.started_at,
        duration,
        state.interval_secs,
        snapshots_for_build,
        alloc_stacks,
    );

    report.verdict = classify_series(&state.totals_history);

    for site in &mut report.sites {
        site.stack = symbolizer.enrich_stack(&site.stack);
    }

    state.previous_snapshot = Some(current);
    Some(report)
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    None,
    Bytes,
    Counts,
    AllocStacks,
}

/// parses bpftrace output into one snapshot per `=== SNAPSHOT n ===` block plus
/// an addr-to-stack map from the trailing `--- ALLOC_STACKS ---` section. each
/// snapshot's bytes and counts sections are correlated by stack text
pub(crate) fn parse_snapshots(stdout: &str) -> (Vec<Snapshot>, HashMap<u64, Vec<String>>) {
    let mut snapshots: Vec<Snapshot> = Vec::new();
    let mut bytes_entries: Vec<(Vec<String>, i64)> = Vec::new();
    let mut counts_by_key: HashMap<String, i64> = HashMap::new();
    let mut alloc_stacks: HashMap<u64, Vec<String>> = HashMap::new();
    let mut section = Section::None;
    let mut open_frames: Option<Vec<String>> = None;
    // in alloc_stacks the address sits on the `@stack[0xADDR]:` opener line,
    // held here while frames are collected
    let mut open_addr: Option<u64> = None;
    let mut started = false;

    let finish_snapshot = |bytes_entries: &mut Vec<(Vec<String>, i64)>,
                           counts: &mut HashMap<String, i64>|
     -> Snapshot {
        let snap = bytes_entries
            .drain(..)
            .filter(|(_, b)| *b > 0)
            .map(|(frames, b)| {
                let count = counts.get(&frames.join("\n")).copied().unwrap_or(0);
                RawSite {
                    bytes: b as u64,
                    count: count.max(0) as u64,
                    frames,
                }
            })
            .collect();
        counts.clear();
        snap
    };

    let flush_alloc_entry =
        |open_addr: &mut Option<u64>,
         open_frames: &mut Option<Vec<String>>,
         alloc_stacks: &mut HashMap<u64, Vec<String>>| {
            if let (Some(addr), Some(frames)) = (open_addr.take(), open_frames.take()) {
                if !frames.is_empty() {
                    alloc_stacks.insert(addr, frames);
                }
            } else {
                *open_addr = None;
                *open_frames = None;
            }
        };

    for line in stdout.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("=== SNAPSHOT") {
            flush_alloc_entry(&mut open_addr, &mut open_frames, &mut alloc_stacks);
            if started {
                snapshots.push(finish_snapshot(&mut bytes_entries, &mut counts_by_key));
            }
            started = true;
            section = Section::None;
            open_frames = None;
            open_addr = None;
            continue;
        }
        if !started {
            continue;
        }
        if trimmed.starts_with("--- BYTES") {
            flush_alloc_entry(&mut open_addr, &mut open_frames, &mut alloc_stacks);
            section = Section::Bytes;
            open_frames = None;
            open_addr = None;
            continue;
        }
        if trimmed.starts_with("--- COUNTS") {
            flush_alloc_entry(&mut open_addr, &mut open_frames, &mut alloc_stacks);
            section = Section::Counts;
            open_frames = None;
            open_addr = None;
            continue;
        }
        if trimmed.starts_with("--- ALLOC_STACKS") {
            section = Section::AllocStacks;
            open_frames = None;
            open_addr = None;
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }

        if section == Section::AllocStacks {
            // entry opener: `@stack[0xADDR]:`, then frame lines follow
            if let Some(rest) = trimmed.strip_prefix('@')
                && let Some(bracket_open) = rest.find('[')
                && let Some(bracket_close) = rest.find("]:")
                && bracket_close > bracket_open
            {
                let key = &rest[bracket_open + 1..bracket_close];
                let parsed = parse_address_key(key);
                if let Some(addr) = parsed {
                    // flush the previous entry before starting a new one.
                    flush_alloc_entry(&mut open_addr, &mut open_frames, &mut alloc_stacks);
                    open_addr = Some(addr);
                    open_frames = Some(Vec::new());
                }
                continue;
            }
            // otherwise a frame line for the open entry.
            if let Some(frames) = open_frames.as_mut() {
                frames.push(trimmed.to_string());
            }
            continue;
        }

        // bytes/counts: multi-line bracketed key, value after `]:`
        if trimmed.starts_with('@') && trimmed.ends_with('[') {
            open_frames = Some(Vec::new());
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("]:") {
            if let Some(frames) = open_frames.take()
                && let Ok(value) = rest.trim().parse::<i64>()
            {
                match section {
                    Section::Bytes => bytes_entries.push((frames, value)),
                    Section::Counts => {
                        counts_by_key.insert(frames.join("\n"), value);
                    }
                    _ => {}
                }
            }
            continue;
        }
        if let Some(frames) = open_frames.as_mut() {
            frames.push(trimmed.to_string());
        }
    }

    flush_alloc_entry(&mut open_addr, &mut open_frames, &mut alloc_stacks);
    if started {
        snapshots.push(finish_snapshot(&mut bytes_entries, &mut counts_by_key));
    }
    (snapshots, alloc_stacks)
}

pub(crate) fn build_report(
    pid: Pid,
    name: String,
    started_at: SystemTime,
    duration: Duration,
    interval_secs: f64,
    mut snapshots: Vec<Snapshot>,
    alloc_stacks: HashMap<u64, Vec<String>>,
) -> AllocScanReport {
    // total outstanding bytes per snapshot, the series the verdict reads
    let totals: Vec<u64> = snapshots
        .iter()
        .map(|snap| snap.iter().map(|s| s.bytes).sum())
        .collect();

    let last = snapshots.pop().unwrap_or_default();
    let first = snapshots.first().cloned().unwrap_or_default();
    let total_last = totals.last().copied().unwrap_or(0);

    // per-site growth measured across the full window (first to last snapshot)
    let span = (interval_secs.max(1.0)) * (totals.len().saturating_sub(1).max(1) as f64);
    let mut sites: Vec<AllocSite> = last
        .into_iter()
        .map(|s| {
            let key = s.frames.join("\n");
            let prev = first
                .iter()
                .find(|p| p.frames.join("\n") == key)
                .map(|p| p.bytes)
                .unwrap_or(0);
            AllocSite {
                growth_bytes_per_s: (s.bytes as f64 - prev as f64) / span,
                outstanding_bytes: s.bytes,
                outstanding_count: s.count,
                stack: s.frames,
            }
        })
        .collect();
    sites.sort_by(|a, b| b.outstanding_bytes.cmp(&a.outstanding_bytes));

    AllocScanReport {
        pid,
        name,
        started_at,
        duration,
        total_outstanding_bytes: total_last,
        verdict: classify_series(&totals),
        sites,
        alloc_stacks,
    }
}

/// verdict from the series of total-outstanding-bytes snapshots. nothing left,
/// or a peak then release, counts as reclaimed. anything below the floor is
/// inconclusive, sustained growth above it is leaking, and a flat series is
/// inconclusive
fn classify_series(totals: &[u64]) -> LeakVerdict {
    let last = totals.last().copied().unwrap_or(0);
    if last == 0 {
        return LeakVerdict::Reclaimed;
    }
    let first = totals.first().copied().unwrap_or(0);
    let max = totals.iter().copied().max().unwrap_or(last);

    // grew then gave most of it back before the window ended: transient
    if (last as f64) < (max as f64) * 0.7 {
        return LeakVerdict::Reclaimed;
    }
    // too small to call a leak
    if last < MIN_LEAK_BYTES {
        return LeakVerdict::Inconclusive;
    }
    if (last as f64) > (first.max(1) as f64) * GROWTH_RATIO {
        LeakVerdict::Leaking
    } else if (last as f64) < (first as f64) * 0.9 {
        LeakVerdict::Reclaimed
    } else {
        LeakVerdict::Inconclusive
    }
}

/// parses the address inside `@stack[KEY]:`. bpftrace prints integer keys in
/// several formats by version, so accept 0xhex, bare hex, and signed or
/// unsigned decimal, tolerating commas and whitespace
fn parse_address_key(key: &str) -> Option<u64> {
    let key = key.trim().trim_matches(',').replace(',', "");
    if key.is_empty() {
        return None;
    }
    if let Some(hex) = key.strip_prefix("0x").or_else(|| key.strip_prefix("0X"))
        && let Ok(n) = u64::from_str_radix(hex, 16)
    {
        return Some(n);
    }
    if let Ok(n) = key.parse::<u64>() {
        return Some(n);
    }
    if let Ok(n) = key.parse::<i64>() {
        return Some(n as u64);
    }
    if key.chars().all(|c| c.is_ascii_hexdigit())
        && let Ok(n) = u64::from_str_radix(&key, 16)
    {
        return Some(n);
    }
    None
}

fn first_error_line(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("Attaching"))
        .unwrap_or("unknown bpftrace error")
        .to_string()
}

fn resolve_in_path(names: &[&str]) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for extra in EXTRA_BIN_DIRS {
        dirs.push(PathBuf::from(extra));
    }
    for dir in dirs {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn resolve_libc_path(pid: Pid) -> Option<PathBuf> {
    let maps = std::fs::read_to_string(format!("/proc/{pid}/maps")).ok()?;
    for line in maps.lines() {
        let Some(path) = line.split_whitespace().last() else {
            continue;
        };
        if !path.starts_with('/') {
            continue;
        }
        let file = path.rsplit('/').next().unwrap_or("");
        if file.starts_with("libc.so") || file.starts_with("libc-") {
            return Some(PathBuf::from(path));
        }
    }
    None
}

fn has_bpf_privilege() -> bool {
    if unsafe { libc::geteuid() } == 0 {
        return true;
    }
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return false;
    };
    for line in status.lines() {
        if let Some(hex) = line.strip_prefix("CapEff:") {
            if let Ok(caps) = u64::from_str_radix(hex.trim(), 16) {
                const CAP_SYS_ADMIN: u64 = 1 << 21;
                const CAP_PERFMON: u64 = 1 << 38;
                const CAP_BPF: u64 = 1 << 39;
                return caps & (CAP_BPF | CAP_PERFMON | CAP_SYS_ADMIN) != 0;
            }
            warn!(target: "leakprobe::bpftrace", "could not parse CapEff");
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Attaching 5 probes...
=== SNAPSHOT 1 ===
--- BYTES ---
@out[
    0x50ae44
    PyByteArray_Resize+124
    main+16
]: 151552
--- COUNTS ---
@cnt[
    0x50ae44
    PyByteArray_Resize+124
    main+16
]: 37
=== SNAPSHOT 2 ===
--- BYTES ---
@out[
    0x50ae44
    PyByteArray_Resize+124
    main+16
]: 303104
--- COUNTS ---
@cnt[
    0x50ae44
    PyByteArray_Resize+124
    main+16
]: 74
--- ALLOC_STACKS ---
@stack[0xaaaabbbb1000]:
    main+0x10
    __libc_start_call_main+0x74
    _start+0x30
@stack[0xaaaabbbb2000]:
    helper+0x40
    main+0x80
";

    #[test]
    fn parses_bytes_and_counts() {
        let (snaps, _alloc_stacks) = parse_snapshots(SAMPLE);
        assert_eq!(snaps.len(), 2);
        assert_eq!(snaps[0].len(), 1);
        assert_eq!(snaps[0][0].bytes, 151552);
        assert_eq!(snaps[0][0].count, 37);
        assert_eq!(snaps[0][0].frames.len(), 3);
        assert_eq!(snaps[1][0].bytes, 303104);
        assert_eq!(snaps[1][0].count, 74);
    }

    #[test]
    fn parses_alloc_stacks_section() {
        let (_snaps, alloc_stacks) = parse_snapshots(SAMPLE);
        assert_eq!(alloc_stacks.len(), 2);
        let frames1 = alloc_stacks.get(&0xaaaabbbb1000).unwrap();
        assert_eq!(frames1.len(), 3);
        assert_eq!(frames1[0], "main+0x10");
        let frames2 = alloc_stacks.get(&0xaaaabbbb2000).unwrap();
        assert_eq!(frames2.len(), 2);
        assert_eq!(frames2[0], "helper+0x40");
    }

    #[test]
    fn growth_and_verdict() {
        let (snaps, alloc_stacks) = parse_snapshots(SAMPLE);
        let report = build_report(
            1,
            "x".into(),
            SystemTime::now(),
            Duration::from_secs(6),
            3.0,
            snaps,
            alloc_stacks,
        );
        // 151552 to 303104: doubled and well above the 256 kb floor
        assert_eq!(report.verdict, LeakVerdict::Leaking);
        assert_eq!(report.total_outstanding_bytes, 303104);
        // single interval between the two sample snapshots
        assert!((report.sites[0].growth_bytes_per_s - (151552.0 / 3.0)).abs() < 1.0);
        // alloc_stacks survives through build_report.
        assert_eq!(report.alloc_stacks.len(), 2);
    }

    #[test]
    fn verdicts() {
        // nothing outstanding at the end
        assert_eq!(classify_series(&[40960, 0]), LeakVerdict::Reclaimed);
        // sustained growth, above the floor.
        assert_eq!(
            classify_series(&[300_000, 500_000, 700_000, 900_000]),
            LeakVerdict::Leaking
        );
        // grew a lot but tiny in absolute terms, so not alarmed on
        assert_eq!(
            classify_series(&[40_960, 81_920]),
            LeakVerdict::Inconclusive
        );
        // peaked then released most of it: transient
        assert_eq!(
            classify_series(&[1_000_000, 5_000_000, 4_000_000, 1_200_000]),
            LeakVerdict::Reclaimed
        );
        // large but flat (a steady working set such as a cache): inconclusive
        assert_eq!(
            classify_series(&[210_000_000, 212_000_000, 213_000_000, 214_000_000]),
            LeakVerdict::Inconclusive
        );
    }

    #[test]
    fn empty_output_yields_no_snapshots() {
        let (snaps, allocs) = parse_snapshots("Attaching 5 probes...\n");
        assert!(snaps.is_empty());
        assert!(allocs.is_empty());
    }
}
