//! snapshots a process with gcore into an elf core we mmap and parse. the
//! target is only paused for the dump; the analysis runs against the core.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use memmap2::Mmap;
use object::Endianness;
use object::elf::{NT_PRSTATUS, PT_LOAD, PT_NOTE};
use object::read::elf::{ElfFile64, FileHeader, ProgramHeader};
use tracing::warn;

use super::ProcessSnapshot;
use crate::leakprobe::ProbeError;
use crate::util::Pid;

const GCORE_BINARIES: &[&str] = &["gcore"];
const GCORE_EXTRA_DIRS: &[&str] = &["/usr/bin", "/bin", "/usr/local/bin"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapKind {
    Heap,
    Stack,
    FileBacked(String),
    Anonymous,
    Vdso,
    Vsyscall,
    Vvar,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct MemMap {
    pub start: u64,
    pub end: u64,
    pub readable: bool,
    pub writable: bool,
    pub kind: MapKind,
}

impl MemMap {
    pub fn size(&self) -> u64 {
        self.end - self.start
    }

    pub fn is_libc(&self) -> bool {
        match &self.kind {
            MapKind::FileBacked(path) => {
                let file = path.rsplit('/').next().unwrap_or("");
                file.starts_with("libc.so") || file.starts_with("libc-")
            }
            _ => false,
        }
    }
}

/// one thread's saved register blob from `NT_PRSTATUS`. we don't decode it
/// mark-sweep scans it as u64 words looking for pointers, which covers every register
#[derive(Debug, Clone)]
pub struct ThreadState {
    pub regs: Vec<u8>,
}

pub trait MemoryReader: Send + Sync {
    /// up to `len` bytes from `addr`, or `None` if it isn't mapped or the read
    /// would cross an unmapped region
    fn read_at(&self, addr: u64, len: usize) -> Option<Vec<u8>>;

    /// little-endian u64 at `addr`, or `None` if not mapped
    fn read_u64(&self, addr: u64) -> Option<u64> {
        let bytes = self.read_at(addr, 8)?;
        if bytes.len() < 8 {
            return None;
        }
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&bytes[..8]);
        Some(u64::from_le_bytes(buf))
    }
}

pub trait Capture: Send + Sync {
    fn capture(&self, pid: Pid) -> Result<ProcessSnapshot, ProbeError>;
    fn is_available(&self) -> bool;
}

pub struct GcoreCapture {
    binary: Option<PathBuf>,
}

impl GcoreCapture {
    pub fn new() -> Self {
        Self {
            binary: resolve_in_path(GCORE_BINARIES, GCORE_EXTRA_DIRS),
        }
    }
}

impl Default for GcoreCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl Capture for GcoreCapture {
    fn is_available(&self) -> bool {
        self.binary.is_some()
    }

    fn capture(&self, pid: Pid) -> Result<ProcessSnapshot, ProbeError> {
        let Some(binary) = self.binary.as_ref() else {
            return Err(ProbeError::Unavailable("gcore not found on PATH".into()));
        };

        // classify mappings before gcore, since reading /proc is
        // more reliable than guessing each pt_load's role from the core
        let live_maps = read_proc_maps(pid)?;

        let tmp = tempfile::tempdir().map_err(|e| ProbeError::Spawn(e.to_string()))?;
        let prefix = tmp.path().join("core");
        let output = Command::new(binary)
            .arg("-o")
            .arg(&prefix)
            .arg(pid.to_string())
            .output()
            .map_err(|e| ProbeError::Spawn(format!("gcore: {e}")))?;

        if !output.status.success() {
            return Err(ProbeError::Failed(format!(
                "gcore exit {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        // gcore names the file "<prefix>.<pid>"
        let core_path = format!("{}.{}", prefix.display(), pid);
        let core_path = Path::new(&core_path);
        if !core_path.is_file() {
            return Err(ProbeError::Failed(format!(
                "gcore produced no core at {}",
                core_path.display()
            )));
        }

        let file = std::fs::File::open(core_path)
            .map_err(|e| ProbeError::Failed(format!("open core: {e}")))?;
        let mmap = unsafe { Mmap::map(&file) }
            .map_err(|e| ProbeError::Failed(format!("mmap core: {e}")))?;
        let mmap = Arc::new(mmap);
        let _tmpdir = Arc::new(tmp);

        let elf = ElfFile64::<Endianness>::parse(&**mmap)
            .map_err(|e| ProbeError::Parse(format!("ELF core: {e}")))?;

        let (loads, threads) = parse_core(&elf, &mmap)?;
        let maps = merge_loads_with_live_maps(&loads, live_maps);

        let reader = CoreMemoryReader {
            loads,
            mmap: Arc::clone(&mmap),
            _tmpdir,
        };

        Ok(ProcessSnapshot {
            pid,
            maps,
            threads,
            mem: Arc::new(reader),
        })
    }
}

/// one pt_load segment: its virtual range and where the bytes live in the core
#[derive(Debug, Clone, Copy)]
struct CoreLoad {
    vaddr: u64,
    memsz: u64,
    filesz: u64,
    offset: u64,
    flags: u32,
}

fn parse_core(
    elf: &ElfFile64<Endianness>,
    mmap: &Mmap,
) -> Result<(Vec<CoreLoad>, Vec<ThreadState>), ProbeError> {
    let endian = elf.endian();
    let header = elf.elf_header();
    let phs = header
        .program_headers(endian, &**mmap)
        .map_err(|e| ProbeError::Parse(format!("program headers: {e}")))?;

    let mut loads = Vec::new();
    let mut threads = Vec::new();

    for ph in phs {
        match ph.p_type(endian) {
            PT_LOAD => loads.push(CoreLoad {
                vaddr: ph.p_vaddr(endian),
                memsz: ph.p_memsz(endian),
                filesz: ph.p_filesz(endian),
                offset: ph.p_offset(endian),
                flags: ph.p_flags(endian),
            }),
            PT_NOTE => {
                let data = ph
                    .data(endian, &**mmap)
                    .map_err(|_| ProbeError::Parse("note data unreadable".to_string()))?;
                parse_notes(data, &mut threads);
            }
            _ => {}
        }
    }

    Ok((loads, threads))
}

/// walks the pt_note blob and pulls out each `NT_PRSTATUS`, one per thread
/// each note is a header, a padded name, then a padded desc. for prstatus the
/// desc is a `struct elf_prstatus` whose `pr_reg` tail holds the registers
fn parse_notes(mut data: &[u8], threads: &mut Vec<ThreadState>) {
    while data.len() >= 12 {
        let namesz = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        let descsz = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        let ntype = u32::from_le_bytes(data[8..12].try_into().unwrap());
        let header_len = 12;
        let name_padded = align4(namesz);
        let desc_padded = align4(descsz);
        let total = header_len + name_padded + desc_padded;
        if data.len() < total {
            break;
        }
        let desc_start = header_len + name_padded;
        let desc = &data[desc_start..desc_start + descsz];
        if ntype == NT_PRSTATUS
            && let Some(state) = parse_prstatus(desc)
        {
            threads.push(state);
        }
        data = &data[total..];
    }
}

fn align4(n: usize) -> usize {
    (n + 3) & !3
}

/// pulls the `pr_reg` register blob out of `struct elf_prstatus`. on x86_64 and
/// aarch64 linux it's the last field bar the trailing pr_fpvalid
fn parse_prstatus(desc: &[u8]) -> Option<ThreadState> {
    // offsets that hold for both x86_64 and aarch64 linux: pr_reg at 112, with
    // pr_fpvalid (i32) at the very end
    const PR_REG_OFFSET: usize = 112;
    const PR_FPVALID_LEN: usize = 4;

    if desc.len() < PR_REG_OFFSET + PR_FPVALID_LEN {
        return None;
    }
    let regs_end = desc.len() - PR_FPVALID_LEN;
    let regs = desc[PR_REG_OFFSET..regs_end].to_vec();
    Some(ThreadState { regs })
}

/// reads and parses `/proc/<pid>/maps`
fn read_proc_maps(pid: Pid) -> Result<Vec<MemMap>, ProbeError> {
    let path = format!("/proc/{pid}/maps");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| ProbeError::Failed(format!("read {path}: {e}")))?;
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(m) = parse_maps_line(line) {
            out.push(m);
        }
    }
    Ok(out)
}

fn parse_maps_line(line: &str) -> Option<MemMap> {
    // format: addr_start-addr_end perms offset dev inode pathname
    let mut parts = line.splitn(6, char::is_whitespace);
    let range = parts.next()?;
    let perms = parts.next()?;
    let _offset = parts.next()?;
    let _dev = parts.next()?;
    let _inode = parts.next()?;
    let path = parts.next().map(str::trim).unwrap_or("");
    let (start_s, end_s) = range.split_once('-')?;
    let start = u64::from_str_radix(start_s, 16).ok()?;
    let end = u64::from_str_radix(end_s, 16).ok()?;
    let perms_bytes = perms.as_bytes();
    let r = perms_bytes.first() == Some(&b'r');
    let w = perms_bytes.get(1) == Some(&b'w');
    let kind = match path {
        "" => MapKind::Anonymous,
        "[heap]" => MapKind::Heap,
        p if p.starts_with("[stack") => MapKind::Stack,
        "[vdso]" => MapKind::Vdso,
        "[vsyscall]" => MapKind::Vsyscall,
        "[vvar]" => MapKind::Vvar,
        p if p.starts_with("[anon:") || p.starts_with("[anon_shmem:") => MapKind::Anonymous,
        p if p.starts_with('/') => MapKind::FileBacked(p.to_string()),
        p => MapKind::Other(p.to_string()),
    };
    Some(MemMap {
        start,
        end,
        readable: r,
        writable: w,
        kind,
    })
}

/// matches each pt_load to its live `/proc/maps` entry for kind and perms
/// loads with no matching entry fall back to anonymous and the elf flag bits
fn merge_loads_with_live_maps(loads: &[CoreLoad], live: Vec<MemMap>) -> Vec<MemMap> {
    let mut out = Vec::with_capacity(loads.len());
    for load in loads {
        let end = load.vaddr + load.memsz;
        let matched = live.iter().find(|m| m.start == load.vaddr && m.end == end);
        let map = matched.cloned().unwrap_or(MemMap {
            start: load.vaddr,
            end,
            readable: load.flags & 0x4 != 0,
            writable: load.flags & 0x2 != 0,
            kind: MapKind::Anonymous,
        });
        out.push(map);
    }
    out
}

struct CoreMemoryReader {
    loads: Vec<CoreLoad>,
    mmap: Arc<Mmap>,
    _tmpdir: Arc<tempfile::TempDir>,
}

impl MemoryReader for CoreMemoryReader {
    fn read_at(&self, addr: u64, len: usize) -> Option<Vec<u8>> {
        let want_end = addr.checked_add(len as u64)?;
        let load = self
            .loads
            .iter()
            .find(|l| addr >= l.vaddr && want_end <= l.vaddr + l.filesz)?;
        let off = (addr - load.vaddr + load.offset) as usize;
        let end = off.checked_add(len)?;
        if end > self.mmap.len() {
            warn!(target: "leakprobe::reachability", "core read past mmap end");
            return None;
        }
        Some(self.mmap[off..end].to_vec())
    }
}

fn resolve_in_path(names: &[&str], extras: &[&str]) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for extra in extras {
        dirs.push(PathBuf::from(extra));
    }
    for dir in dirs {
        for n in names {
            let p = dir.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_maps_line_heap() {
        let line =
            "55b6e8b3c000-55b6e8b5d000 rw-p 00000000 00:00 0                          [heap]";
        let m = parse_maps_line(line).unwrap();
        assert_eq!(m.start, 0x55b6e8b3c000);
        assert_eq!(m.end, 0x55b6e8b5d000);
        assert!(m.readable && m.writable);
        assert_eq!(m.kind, MapKind::Heap);
    }

    #[test]
    fn parses_maps_line_libc() {
        let line = "fdf965ab0000-fdf965c00000 r-xp 00000000 fd:01 1                          /usr/lib/aarch64-linux-gnu/libc.so.6";
        let m = parse_maps_line(line).unwrap();
        assert!(matches!(&m.kind, MapKind::FileBacked(p) if p.ends_with("libc.so.6")));
        assert!(m.is_libc());
    }

    #[test]
    fn parses_maps_line_anon_stack() {
        let line =
            "7ffd1a000000-7ffd1a021000 rw-p 00000000 00:00 0                          [stack]";
        let m = parse_maps_line(line).unwrap();
        assert_eq!(m.kind, MapKind::Stack);
    }

    #[test]
    fn glibc_tagged_mmap_malloc_is_anonymous() {
        // modern glibc tags each mmap'd malloc allocation.
        let line = "bbba22e00000-bbba22e80000 rw-p 00000000 00:00 0                          [anon:glibc: malloc]";
        let m = parse_maps_line(line).unwrap();
        assert_eq!(
            m.kind,
            MapKind::Anonymous,
            "tagged anon must be anonymous so the walker examines it"
        );
    }

    #[test]
    fn anon_shmem_tag_is_anonymous() {
        let line = "7f1234000000-7f1234001000 rw-p 00000000 00:00 0                          [anon_shmem:something]";
        let m = parse_maps_line(line).unwrap();
        assert_eq!(m.kind, MapKind::Anonymous);
    }
}
