//! walks glibc's heap to enumerate live malloc chunks, following the in-band
//! chunk-size headers through `[heap]` and mmap'd regions.

use super::capture::{MapKind, MemoryReader};
use super::{AllocatorWalker, HeapBlock, ProcessSnapshot};
use crate::leakprobe::ProbeError;

const SIZE_T: u64 = 8;
const CHUNK_HEADER: u64 = 2 * SIZE_T;
const MIN_CHUNK_SIZE: u64 = 32;
const FLAG_IS_MMAPPED: u64 = 0x2;
const FLAG_MASK: u64 = 0x7;
#[cfg(test)]
const FLAG_PREV_INUSE: u64 = 0x1;

pub struct GlibcWalker;

impl GlibcWalker {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GlibcWalker {
    fn default() -> Self {
        Self::new()
    }
}

impl AllocatorWalker for GlibcWalker {
    fn name(&self) -> &'static str {
        "glibc"
    }

    fn supports(&self, snapshot: &ProcessSnapshot) -> bool {
        // a glibc target maps libc.so.
        snapshot.maps.iter().any(|m| m.is_libc())
    }

    fn walk(&self, snapshot: &ProcessSnapshot) -> Result<Vec<HeapBlock>, ProbeError> {
        let debug = std::env::var_os("LEAKPROBE_DEBUG").is_some();
        let mut blocks = Vec::new();
        for map in &snapshot.maps {
            let before = blocks.len();
            match &map.kind {
                MapKind::Heap => {
                    walk_arena(map.start, map.end, snapshot.mem.as_ref(), &mut blocks);
                    if debug {
                        let found = blocks.len() - before;
                        eprintln!(
                            "[leakprobe] heap walk {:#x}-{:#x} ({} bytes): {found} blocks",
                            map.start,
                            map.end,
                            map.end - map.start
                        );
                    }
                }
                MapKind::Anonymous if map.writable && map.readable => {
                    // the kernel merges adjacent mmap'd allocations into one
                    // vma, each keeping its own chunk header, so walk linearly
                    // using IS_MMAPPED as the continue predicate.
                    walk_mmap_chunks(map.start, map.end, snapshot.mem.as_ref(), &mut blocks);
                    let found_mmap = blocks.len() - before;
                    if found_mmap == 0 {
                        // no mmap chunks here, so try it as a secondary arena.
                        walk_arena(map.start, map.end, snapshot.mem.as_ref(), &mut blocks);
                        if debug {
                            let found = blocks.len() - before;
                            eprintln!(
                                "[leakprobe] anon walk {:#x}-{:#x} ({} bytes): {found} blocks",
                                map.start,
                                map.end,
                                map.end - map.start
                            );
                        }
                    } else if debug {
                        eprintln!(
                            "[leakprobe] mmap chunks {:#x}-{:#x} ({} bytes): {found_mmap} blocks",
                            map.start,
                            map.end,
                            map.end - map.start
                        );
                    }
                }
                _ => {
                    if debug && map.writable && map.readable && map.size() > 16 * 1024 * 1024 {
                        // a big rw mapping we skipped, where memory could hide.
                        eprintln!(
                            "[leakprobe] SKIPPED {:?} {:#x}-{:#x} ({} bytes, rw)",
                            map.kind,
                            map.start,
                            map.end,
                            map.end - map.start
                        );
                    }
                }
            }
        }
        if debug {
            eprintln!("[leakprobe] glibc walker total: {} blocks", blocks.len());
        }
        Ok(blocks)
    }
}

/// walks a region as a chain of malloc_chunks. stops at the first implausible
/// size (zero, unaligned, or past the region), which is the top chunk or a
/// region we shouldn't have walked.
fn walk_arena(start: u64, end: u64, mem: &dyn MemoryReader, out: &mut Vec<HeapBlock>) {
    let mut cursor = start;
    let mut steps = 0u64;
    while cursor + CHUNK_HEADER <= end {
        let Some(size_w) = mem.read_u64(cursor + SIZE_T) else {
            break;
        };
        let size = size_w & !FLAG_MASK;
        if size < MIN_CHUNK_SIZE || size & 0x7 != 0 {
            break;
        }
        let chunk_end = match cursor.checked_add(size) {
            Some(e) => e,
            None => break,
        };
        if chunk_end >= end {
            // top chunk runs to the end of the region.
            break;
        }
        // record the user pointer, what malloc() would have returned.
        out.push(HeapBlock {
            addr: cursor + CHUNK_HEADER,
            size: size - CHUNK_HEADER,
        });
        cursor = chunk_end;
        steps += 1;
        if steps > 5_000_000 {
            // brake so a corrupted snapshot can't hang us.
            break;
        }
    }
}

/// walks consecutive mmap'd chunks in `[start, end)`. stops when a header lacks
/// the `IS_MMAPPED` flag, has an implausible size, or would run past `end`.
/// recovers the back-to-back chunks the kernel leaves in one coalesced vma.
fn walk_mmap_chunks(start: u64, end: u64, mem: &dyn MemoryReader, out: &mut Vec<HeapBlock>) {
    let mut cursor = start;
    let mut steps = 0u64;
    while cursor + CHUNK_HEADER <= end {
        let Some(size_w) = mem.read_u64(cursor + SIZE_T) else {
            break;
        };
        if size_w & FLAG_IS_MMAPPED == 0 {
            break;
        }
        let size = size_w & !FLAG_MASK;
        if size < MIN_CHUNK_SIZE {
            break;
        }
        let chunk_end = match cursor.checked_add(size) {
            Some(e) if e <= end => e,
            _ => break,
        };
        out.push(HeapBlock {
            addr: cursor + CHUNK_HEADER,
            size: size - CHUNK_HEADER,
        });
        cursor = chunk_end;
        steps += 1;
        if steps > 5_000_000 {
            break;
        }
    }
}

/// single-chunk variant kept for tests.
#[cfg(test)]
fn detect_mmap_chunk(start: u64, end: u64, mem: &dyn MemoryReader) -> Option<HeapBlock> {
    let mut out = Vec::new();
    walk_mmap_chunks(start, end, mem, &mut out);
    out.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// tiny in-memory MemoryReader for the unit tests.
    struct FakeMem(Vec<(u64, Vec<u8>)>);

    impl FakeMem {
        fn put(&mut self, addr: u64, bytes: &[u8]) {
            self.0.push((addr, bytes.to_vec()));
        }
    }

    impl MemoryReader for FakeMem {
        fn read_at(&self, addr: u64, len: usize) -> Option<Vec<u8>> {
            for (a, bs) in &self.0 {
                if addr >= *a && addr + len as u64 <= *a + bs.len() as u64 {
                    let off = (addr - *a) as usize;
                    return Some(bs[off..off + len].to_vec());
                }
            }
            None
        }
    }

    fn chunk_bytes(prev_size: u64, size: u64, flags: u64, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&prev_size.to_le_bytes());
        out.extend_from_slice(&(size | flags).to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn walks_three_chunks_then_stops_at_top() {
        let base = 0x10_0000u64;
        let mut bytes = Vec::new();
        // three 64-byte chunks then a top chunk.
        bytes.extend(chunk_bytes(0, 64, FLAG_PREV_INUSE, &vec![0u8; 48]));
        bytes.extend(chunk_bytes(0, 64, FLAG_PREV_INUSE, &vec![0u8; 48]));
        bytes.extend(chunk_bytes(0, 64, FLAG_PREV_INUSE, &vec![0u8; 48]));
        // end the region at the top chunk so the walker stops there.
        let region_end = base + bytes.len() as u64 + 64;
        bytes.extend(chunk_bytes(0, 64, FLAG_PREV_INUSE, &vec![0u8; 48]));

        let mut mem = FakeMem(Vec::new());
        mem.put(base, &bytes);
        let mut out = Vec::new();
        walk_arena(base, region_end, &mem, &mut out);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].addr, base + 16);
        assert_eq!(out[0].size, 48);
    }

    #[test]
    fn detects_mmap_chunk() {
        let base = 0x20_0000u64;
        let size = 256 * 1024u64;
        let mut bytes = chunk_bytes(0, size, FLAG_IS_MMAPPED, &[]);
        bytes.resize(size as usize, 0);
        let mut mem = FakeMem(Vec::new());
        mem.put(base, &bytes);
        let block = detect_mmap_chunk(base, base + size, &mem).unwrap();
        assert_eq!(block.addr, base + 16);
        assert_eq!(block.size, size - 16);
    }

    #[test]
    fn walks_back_to_back_mmap_chunks_in_coalesced_vma() {
        // three back-to-back mmap chunks, the coalesced anon vma case.
        let base = 0x40_0000u64;
        let mut bytes = Vec::new();
        for size in [256 * 1024u64, 512 * 1024u64, 1024 * 1024u64] {
            let mut chunk = chunk_bytes(0, size, FLAG_IS_MMAPPED, &[]);
            chunk.resize(size as usize, 0);
            bytes.extend(chunk);
        }
        let total = bytes.len() as u64;
        let mut mem = FakeMem(Vec::new());
        mem.put(base, &bytes);
        let mut out = Vec::new();
        walk_mmap_chunks(base, base + total, &mem, &mut out);
        assert_eq!(
            out.len(),
            3,
            "must walk every mmap chunk, not just the first"
        );
        assert_eq!(out[0].size, 256 * 1024 - 16);
        assert_eq!(out[1].size, 512 * 1024 - 16);
        assert_eq!(out[2].size, 1024 * 1024 - 16);
    }

    #[test]
    fn rejects_non_mmap_anon() {
        let base = 0x30_0000u64;
        let bytes = chunk_bytes(0, 4096, FLAG_PREV_INUSE, &[]); // no is_mmapped flag
        let mut mem = FakeMem(Vec::new());
        mem.put(base, &bytes);
        assert!(detect_mmap_chunk(base, base + 4096, &mem).is_none());
    }

    #[test]
    fn supports_uses_libc_presence() {
        use crate::leakprobe::reachability::capture::{MapKind, MemMap};
        let snap = ProcessSnapshot {
            pid: 1,
            maps: vec![MemMap {
                start: 0,
                end: 0x1000,
                readable: true,
                writable: false,
                kind: MapKind::FileBacked("/usr/lib/aarch64-linux-gnu/libc.so.6".into()),
            }],
            threads: vec![],
            mem: Arc::new(FakeMem(vec![])),
        };
        let w = GlibcWalker::new();
        assert!(w.supports(&snap));
    }
}
