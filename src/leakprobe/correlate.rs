//! joins reachability-scan leaked blocks to allocation-scan allocation stacks by address, so a
//! leaked block can show where it was allocated. blocks allocation-scan never saw stay
//! unattributed

use super::{AllocScanReport, BlockClass, ReachabilityReport};

#[derive(Debug, Clone, PartialEq)]
pub struct AttributedBlock {
    pub addr: u64,
    pub size: u64,
    pub class: BlockClass,
    /// `None` if allocation-scan never saw this allocation, because the block
    /// existed before the allocation-scan scan started
    pub stack: Option<Vec<String>>,
}

/// attaches each lost block's allocation stack when allocation-scan captured one.
/// iterates the full `all_leaked` set
pub fn attribute_leaks(
    reach: &ReachabilityReport,
    alloc: Option<&AllocScanReport>,
) -> Vec<AttributedBlock> {
    let lookup = |addr: u64| -> Option<Vec<String>> {
        alloc.and_then(|a| a.alloc_stacks.get(&addr).cloned())
    };

    let mut out: Vec<AttributedBlock> = reach
        .all_leaked
        .iter()
        .map(|b| AttributedBlock {
            addr: b.addr,
            size: b.size,
            class: b.class,
            stack: lookup(b.addr),
        })
        .collect();

    out.sort_by(|a, b| {
        class_severity(b.class)
            .cmp(&class_severity(a.class))
            .then(b.size.cmp(&a.size))
    });
    out
}

fn class_severity(c: BlockClass) -> u8 {
    match c {
        BlockClass::DefinitelyLost => 3,
        BlockClass::IndirectlyLost => 2,
        BlockClass::PossiblyLost => 1,
        BlockClass::StillReachable => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leakprobe::{
        AllocScanReport, AllocSite, ClassificationGroup, ClassifiedBlock, LeakVerdict,
        ReachabilityReport,
    };
    use std::collections::HashMap;
    use std::time::{Duration, SystemTime};

    fn report_with_lost_blocks(blocks: &[(u64, u64)]) -> ReachabilityReport {
        let total_bytes = blocks.iter().map(|(_, s)| *s).sum();
        let cbs: Vec<ClassifiedBlock> = blocks
            .iter()
            .map(|(a, s)| ClassifiedBlock {
                addr: *a,
                size: *s,
                class: BlockClass::DefinitelyLost,
            })
            .collect();
        ReachabilityReport {
            pid: 1,
            name: "x".into(),
            started_at: SystemTime::now(),
            duration: Duration::from_secs(1),
            allocator: "glibc".into(),
            total_blocks: blocks.len(),
            total_bytes,
            definitely_lost: ClassificationGroup {
                block_count: blocks.len(),
                total_bytes,
                top_blocks: cbs.clone(),
            },
            possibly_lost: ClassificationGroup::default(),
            indirectly_lost: ClassificationGroup::default(),
            still_reachable: ClassificationGroup::default(),
            all_leaked: cbs,
        }
    }

    fn alloc_report_with_stacks(stacks: &[(u64, &[&str])]) -> AllocScanReport {
        let mut map = HashMap::new();
        for (addr, frames) in stacks {
            map.insert(
                *addr,
                frames.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            );
        }
        AllocScanReport {
            pid: 1,
            name: "x".into(),
            started_at: SystemTime::now(),
            duration: Duration::from_secs(1),
            total_outstanding_bytes: 0,
            verdict: LeakVerdict::Inconclusive,
            sites: Vec::<AllocSite>::new(),
            alloc_stacks: map,
        }
    }

    #[test]
    fn attaches_stack_when_addr_matches() {
        let reach = report_with_lost_blocks(&[(0x1000, 4096)]);
        let alloc = alloc_report_with_stacks(&[(0x1000, &["main+0x10", "_start"])]);
        let out = attribute_leaks(&reach, Some(&alloc));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].addr, 0x1000);
        assert_eq!(out[0].stack.as_deref().map(|s| s.len()), Some(2));
        assert_eq!(out[0].stack.as_ref().unwrap()[0], "main+0x10");
    }

    #[test]
    fn leaves_stack_none_when_addr_missing() {
        let reach = report_with_lost_blocks(&[(0x2000, 4096)]);
        let alloc = alloc_report_with_stacks(&[(0x9999, &["unrelated"])]);
        let out = attribute_leaks(&reach, Some(&alloc));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].stack, None);
    }

    #[test]
    fn no_alloc_report_means_no_stacks_for_anyone() {
        let reach = report_with_lost_blocks(&[(0x3000, 1024), (0x4000, 2048)]);
        let out = attribute_leaks(&reach, None);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|b| b.stack.is_none()));
    }

    #[test]
    fn class_severity_then_size_ordering() {
        let mut reach = report_with_lost_blocks(&[(0x1000, 1024)]);
        let cb = ClassifiedBlock {
            addr: 0x2000,
            size: 99999,
            class: BlockClass::IndirectlyLost,
        };
        reach.indirectly_lost.top_blocks.push(cb);
        reach.indirectly_lost.block_count = 1;
        reach.indirectly_lost.total_bytes = 99999;
        reach.all_leaked.push(cb);
        let out = attribute_leaks(&reach, None);
        assert_eq!(out.len(), 2);
        // definitely-lost outranks indirectly-lost even though it is far smaller here.
        assert_eq!(out[0].class, BlockClass::DefinitelyLost);
        assert_eq!(out[1].class, BlockClass::IndirectlyLost);
    }
}
