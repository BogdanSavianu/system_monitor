//! conservative mark-sweep: anything no pointer reaches is leaked. classifies
//! each block as definitely, indirectly, or possibly lost, or still reachable.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use super::capture::{MapKind, MemoryReader};
use super::pac::candidate_addresses;
use super::{
    BlockClass, ClassificationGroup, ClassifiedBlock, HeapBlock, ProcessSnapshot,
    ReachabilityReport,
};
use crate::util::Pid;

/// blocks per category kept for the ui and db
const TOP_BLOCKS_PER_CATEGORY: usize = 20;

const WORD: u64 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reach {
    Unmarked,
    Direct,
    InteriorOnly,
    Indirect,
}

pub struct MarkSweep {
    blocks: Vec<HeapBlock>,
    reach: Vec<Reach>,
    /// parallel to `blocks`, holds each addr for binary search
    block_starts: Vec<u64>,
}

impl MarkSweep {
    /// runs both passes and returns a populated classifier
    pub fn classify(snapshot: &ProcessSnapshot, blocks: &[HeapBlock]) -> Self {
        // sort by address so lookups can binary-search.
        let mut sorted: Vec<HeapBlock> = blocks.to_vec();
        sorted.sort_by_key(|b| b.addr);
        let starts: Vec<u64> = sorted.iter().map(|b| b.addr).collect();
        let mut ms = Self {
            reach: vec![Reach::Unmarked; sorted.len()],
            block_starts: starts,
            blocks: sorted,
        };

        if ms.blocks.is_empty() {
            return ms;
        }

        let heap_ranges = collect_heap_ranges(snapshot, &ms.blocks);
        ms.mark_from_roots(snapshot, &heap_ranges);
        ms.mark_indirect(snapshot);
        ms
    }

    /// pass 1: scan roots, mark reachable blocks, recurse into their contents
    fn mark_from_roots(&mut self, snapshot: &ProcessSnapshot, heap_ranges: &[(u64, u64)]) {
        let mut queue: Vec<usize> = Vec::new();

        // roots: every thread's register blob.
        for thread in &snapshot.threads {
            self.scan_bytes(&thread.regs, &mut queue);
        }

        // roots: writable mappings that aren't part of the heap
        for map in &snapshot.maps {
            if !map.readable || !map.writable {
                continue;
            }
            if in_heap_range(map.start, heap_ranges) {
                continue;
            }
            // vdso/vsyscall/vvar aren't program data
            if matches!(map.kind, MapKind::Vdso | MapKind::Vsyscall | MapKind::Vvar) {
                continue;
            }
            self.scan_mapping_bytes(snapshot.mem.as_ref(), map.start, map.end, &mut queue);
        }

        // scan the contents of every marked block for more pointers
        while let Some(idx) = queue.pop() {
            let block = self.blocks[idx];
            self.scan_mapping_bytes(
                snapshot.mem.as_ref(),
                block.addr,
                block.addr + block.size,
                &mut queue,
            );
        }
    }

    /// pass 2: among unmarked blocks, anything reachable from another unmarked
    /// block becomes indirectly lost, the rest stay definitely lost
    fn mark_indirect(&mut self, snapshot: &ProcessSnapshot) {
        // scan each unmarked block and mark the other unmarked blocks it
        // reaches as indirect. the originating block stays the leak head
        let mut to_indirect: Vec<usize> = Vec::new();
        for src_idx in 0..self.blocks.len() {
            if self.reach[src_idx] != Reach::Unmarked {
                continue;
            }
            let block = self.blocks[src_idx];
            let mut found = Vec::new();
            self.scan_mapping_for_targets(
                snapshot.mem.as_ref(),
                block.addr,
                block.addr + block.size,
                &mut found,
            );
            for (target_idx, _direct) in found {
                if target_idx != src_idx && self.reach[target_idx] == Reach::Unmarked {
                    to_indirect.push(target_idx);
                }
            }
        }
        for idx in to_indirect {
            self.reach[idx] = Reach::Indirect;
        }
    }

    /// scans a byte buffer such as a register blob for candidate pointers.
    fn scan_bytes(&mut self, buf: &[u8], queue: &mut Vec<usize>) {
        let words = buf.len() / WORD as usize;
        for i in 0..words {
            let off = i * WORD as usize;
            let word = u64::from_le_bytes(buf[off..off + WORD as usize].try_into().unwrap());
            self.try_pointer(word, queue);
        }
    }

    /// scans a virtual-address range through the memory reader
    fn scan_mapping_bytes(
        &mut self,
        mem: &dyn MemoryReader,
        start: u64,
        end: u64,
        queue: &mut Vec<usize>,
    ) {
        let mut cur = align_up(start, WORD);
        const CHUNK: u64 = 64 * 1024;
        while cur < end {
            let len = ((end - cur).min(CHUNK)) as usize;
            let Some(bytes) = mem.read_at(cur, len) else {
                cur += WORD; // skip an unreadable hole
                continue;
            };
            let words = bytes.len() / WORD as usize;
            for i in 0..words {
                let off = i * WORD as usize;
                let word = u64::from_le_bytes(bytes[off..off + WORD as usize].try_into().unwrap());
                self.try_pointer(word, queue);
            }
            cur += bytes.len() as u64;
        }
    }

    /// pass-2 variant: reports the blocks a range reaches
    fn scan_mapping_for_targets(
        &self,
        mem: &dyn MemoryReader,
        start: u64,
        end: u64,
        out: &mut Vec<(usize, bool)>,
    ) {
        let mut cur = align_up(start, WORD);
        const CHUNK: u64 = 64 * 1024;
        while cur < end {
            let len = ((end - cur).min(CHUNK)) as usize;
            let Some(bytes) = mem.read_at(cur, len) else {
                cur += WORD;
                continue;
            };
            let words = bytes.len() / WORD as usize;
            for i in 0..words {
                let off = i * WORD as usize;
                let word = u64::from_le_bytes(bytes[off..off + WORD as usize].try_into().unwrap());
                for cand in candidate_addresses(word) {
                    if let Some((idx, direct)) = self.lookup_block(cand) {
                        out.push((idx, direct));
                    }
                }
            }
            cur += bytes.len() as u64;
        }
    }

    /// tries each pac/tbi-masked candidate of `word` and marks any block it hits
    fn try_pointer(&mut self, word: u64, queue: &mut Vec<usize>) {
        for cand in candidate_addresses(word) {
            if let Some((idx, direct)) = self.lookup_block(cand) {
                let prev = self.reach[idx];
                let new = if direct {
                    Reach::Direct
                } else {
                    Reach::InteriorOnly
                };
                let merged = match (prev, new) {
                    (Reach::Direct, _) | (_, Reach::Direct) => Reach::Direct,
                    (Reach::InteriorOnly, _) | (_, Reach::InteriorOnly) => Reach::InteriorOnly,
                    _ => prev,
                };
                if prev == Reach::Unmarked {
                    queue.push(idx);
                }
                self.reach[idx] = merged;
                return;
            }
        }
    }

    /// binary-searches the block index for `addr`. `is_direct` means the address
    /// equals the block's start rather than its interior
    fn lookup_block(&self, addr: u64) -> Option<(usize, bool)> {
        if self.blocks.is_empty() {
            return None;
        }
        let pos = match self.block_starts.binary_search(&addr) {
            Ok(i) => return Some((i, true)),
            Err(i) => i,
        };
        if pos == 0 {
            return None;
        }
        let idx = pos - 1;
        let b = self.blocks[idx];
        if addr < b.addr + b.size {
            Some((idx, false))
        } else {
            None
        }
    }

    /// builds the user-facing report. the caller passes pid, name, and
    /// started_at because mark-sweep doesn't know them
    pub fn build_report(self, pid: Pid, name: &str, started_at: SystemTime) -> ReachabilityReport {
        let duration = SystemTime::now()
            .duration_since(started_at)
            .unwrap_or(Duration::ZERO);

        let mut def = ClassificationGroup::default();
        let mut pos = ClassificationGroup::default();
        let mut ind = ClassificationGroup::default();
        let mut reach = ClassificationGroup::default();

        let total_blocks = self.blocks.len();
        let mut total_bytes: u64 = 0;
        // full leaked set, so the allocation-scan join matches against every block, not
        // just the size-capped top_blocks display list
        let mut all_leaked: Vec<ClassifiedBlock> = Vec::new();
        for (i, block) in self.blocks.iter().enumerate() {
            total_bytes = total_bytes.saturating_add(block.size);
            let group = match self.reach[i] {
                Reach::Unmarked => &mut def,
                Reach::InteriorOnly => &mut pos,
                Reach::Indirect => &mut ind,
                Reach::Direct => &mut reach,
            };
            group.block_count += 1;
            group.total_bytes = group.total_bytes.saturating_add(block.size);
            let class = match self.reach[i] {
                Reach::Unmarked => BlockClass::DefinitelyLost,
                Reach::InteriorOnly => BlockClass::PossiblyLost,
                Reach::Indirect => BlockClass::IndirectlyLost,
                Reach::Direct => BlockClass::StillReachable,
            };
            let cb = ClassifiedBlock {
                addr: block.addr,
                size: block.size,
                class,
            };
            if class != BlockClass::StillReachable {
                all_leaked.push(cb);
            }
            group.top_blocks.push(cb);
        }
        for g in [&mut def, &mut pos, &mut ind, &mut reach] {
            g.top_blocks.sort_by(|a, b| b.size.cmp(&a.size));
            g.top_blocks.truncate(TOP_BLOCKS_PER_CATEGORY);
        }

        ReachabilityReport {
            pid,
            name: name.to_string(),
            started_at,
            duration,
            allocator: "glibc".to_string(),
            total_blocks,
            total_bytes,
            definitely_lost: def,
            possibly_lost: pos,
            indirectly_lost: ind,
            still_reachable: reach,
            all_leaked,
        }
    }
}

/// the [heap] mapping plus any mapping that holds enumerated blocks (mmap'd
/// chunks). excluded from the root sweep so a leaked block's contents aren't
/// treated as a root
fn collect_heap_ranges(snapshot: &ProcessSnapshot, blocks: &[HeapBlock]) -> Vec<(u64, u64)> {
    let mut ranges: Vec<(u64, u64)> = Vec::new();
    let mut by_start: HashMap<u64, u64> = HashMap::new();
    for m in &snapshot.maps {
        if matches!(m.kind, MapKind::Heap) {
            ranges.push((m.start, m.end));
        } else {
            by_start.insert(m.start, m.end);
        }
    }
    for b in blocks {
        // if a block's pointer falls in a non-heap mapping, treat that whole
        // mapping as a heap range so it isn't scanned as a root
        for (start, end) in by_start.iter() {
            if b.addr >= *start && b.addr < *end {
                ranges.push((*start, *end));
                break;
            }
        }
    }
    ranges.sort_by_key(|r| r.0);
    ranges.dedup();
    ranges
}

fn in_heap_range(addr: u64, ranges: &[(u64, u64)]) -> bool {
    ranges.iter().any(|(s, e)| addr >= *s && addr < *e)
}

fn align_up(v: u64, align: u64) -> u64 {
    (v + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leakprobe::reachability::capture::{MapKind, MemMap};
    use std::sync::Arc;

    /// memory reader stitched from a list of (vaddr, bytes) regions
    struct StubMem(Vec<(u64, Vec<u8>)>);
    impl MemoryReader for StubMem {
        fn read_at(&self, addr: u64, len: usize) -> Option<Vec<u8>> {
            for (a, bs) in &self.0 {
                let region_end = a + bs.len() as u64;
                if addr >= *a && addr + len as u64 <= region_end {
                    let off = (addr - *a) as usize;
                    return Some(bs[off..off + len].to_vec());
                }
            }
            None
        }
    }

    fn pointer_le(p: u64) -> [u8; 8] {
        p.to_le_bytes()
    }

    #[test]
    fn directly_referenced_block_is_still_reachable() {
        // one block at 0x1000, with a global at 0x9000 holding a pointer to it
        let block = HeapBlock {
            addr: 0x1000,
            size: 64,
        };
        let mut heap = vec![0u8; 64];
        let mut globals = pointer_le(0x1000).to_vec();
        globals.resize(64, 0);
        let mem = StubMem(vec![(0x1000, heap.clone()), (0x9000, globals)]);
        // heap mapping keeps the global out of the heap ranges
        let snap = ProcessSnapshot {
            pid: 1,
            maps: vec![
                MemMap {
                    start: 0x1000,
                    end: 0x1040,
                    readable: true,
                    writable: true,
                    kind: MapKind::Heap,
                },
                MemMap {
                    start: 0x9000,
                    end: 0x9040,
                    readable: true,
                    writable: true,
                    kind: MapKind::FileBacked("/some/lib.so".into()),
                },
            ],
            threads: vec![],
            mem: Arc::new(mem),
        };
        let ms = MarkSweep::classify(&snap, &[block]);
        let rep = ms.build_report(1, "x", SystemTime::now());
        assert_eq!(rep.still_reachable.block_count, 1);
        assert_eq!(rep.definitely_lost.block_count, 0);
        let _ = heap;
    }

    #[test]
    fn unreferenced_block_is_definitely_lost() {
        let block = HeapBlock {
            addr: 0x2000,
            size: 64,
        };
        let heap = vec![0u8; 64];
        let globals = vec![0u8; 64]; // no pointer to the block
        let mem = StubMem(vec![(0x2000, heap), (0x8000, globals)]);
        let snap = ProcessSnapshot {
            pid: 1,
            maps: vec![
                MemMap {
                    start: 0x2000,
                    end: 0x2040,
                    readable: true,
                    writable: true,
                    kind: MapKind::Heap,
                },
                MemMap {
                    start: 0x8000,
                    end: 0x8040,
                    readable: true,
                    writable: true,
                    kind: MapKind::FileBacked("/some/lib.so".into()),
                },
            ],
            threads: vec![],
            mem: Arc::new(mem),
        };
        let rep = MarkSweep::classify(&snap, &[block]).build_report(1, "x", SystemTime::now());
        assert_eq!(rep.definitely_lost.block_count, 1);
        assert_eq!(rep.still_reachable.block_count, 0);
    }

    #[test]
    fn interior_pointer_marks_possibly_lost() {
        let block = HeapBlock {
            addr: 0x3000,
            size: 128,
        };
        let heap = vec![0u8; 128];
        // global points 32 bytes into the block, not at its head
        let mut globals = pointer_le(0x3020).to_vec();
        globals.resize(64, 0);
        let mem = StubMem(vec![(0x3000, heap), (0xA000, globals)]);
        let snap = ProcessSnapshot {
            pid: 1,
            maps: vec![
                MemMap {
                    start: 0x3000,
                    end: 0x3080,
                    readable: true,
                    writable: true,
                    kind: MapKind::Heap,
                },
                MemMap {
                    start: 0xA000,
                    end: 0xA040,
                    readable: true,
                    writable: true,
                    kind: MapKind::FileBacked("/some/lib.so".into()),
                },
            ],
            threads: vec![],
            mem: Arc::new(mem),
        };
        let rep = MarkSweep::classify(&snap, &[block]).build_report(1, "x", SystemTime::now());
        assert_eq!(rep.possibly_lost.block_count, 1);
        assert_eq!(rep.still_reachable.block_count, 0);
    }

    #[test]
    fn indirectly_lost_chain() {
        // a points at b, no root points at a. so a is definitely lost and b,
        // reachable only from a, is indirectly lost
        let a = HeapBlock {
            addr: 0x4000,
            size: 64,
        };
        let b = HeapBlock {
            addr: 0x5000,
            size: 64,
        };
        let mut heap_a = pointer_le(0x5000).to_vec();
        heap_a.resize(64, 0);
        let heap_b = vec![0u8; 64];
        let globals = vec![0u8; 64]; // no root pointer
        let mem = StubMem(vec![(0x4000, heap_a), (0x5000, heap_b), (0xB000, globals)]);
        let snap = ProcessSnapshot {
            pid: 1,
            maps: vec![
                MemMap {
                    start: 0x4000,
                    end: 0x4040,
                    readable: true,
                    writable: true,
                    kind: MapKind::Heap,
                },
                MemMap {
                    start: 0x5000,
                    end: 0x5040,
                    readable: true,
                    writable: true,
                    kind: MapKind::Heap,
                },
                MemMap {
                    start: 0xB000,
                    end: 0xB040,
                    readable: true,
                    writable: true,
                    kind: MapKind::FileBacked("/some/lib.so".into()),
                },
            ],
            threads: vec![],
            mem: Arc::new(mem),
        };
        let rep = MarkSweep::classify(&snap, &[a, b]).build_report(1, "x", SystemTime::now());
        assert_eq!(rep.definitely_lost.block_count, 1);
        assert_eq!(rep.indirectly_lost.block_count, 1);
    }
}
