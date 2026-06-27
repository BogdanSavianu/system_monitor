//! end-to-end reachability-scan scan test against a real leaker. needs root (gcore uses
//! ptrace) and the c fixture built, so it's ignored by default

#![cfg(feature = "leakprobe")]

use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use system_monitor::leakprobe::{GcoreReachabilityScanner, ReachabilityProfiler};

#[test]
#[ignore = "needs root and experiments/leaks/steady_leak"]
fn reachability_scan_finds_steady_leak() {
    let leaker = Path::new("experiments/leaks/steady_leak");
    assert!(
        leaker.exists(),
        "build the fixture first: make -C experiments/leaks steady_leak"
    );

    let scanner = GcoreReachabilityScanner::new();
    assert!(
        scanner.is_available(),
        "scanner unavailable, run as root and ensure `gcore` is on PATH"
    );

    // 64 kb allocations every second, never freed. under glibc's 128 kb mmap
    // threshold, so they land in [heap] and take the linear chunk-walk path
    let mut child = Command::new(leaker)
        .args(["64", "1", "0"])
        .spawn()
        .expect("failed to spawn steady_leak");

    // let several allocations accumulate.
    sleep(Duration::from_secs(4));

    let result = scanner.reachability_scan(child.id(), "steady_leak");
    let _ = child.kill();
    let _ = child.wait();

    let report = result.expect("reachability scan failed");
    println!(
        "verdict: total={} bytes across {} blocks",
        report.total_bytes, report.total_blocks
    );
    println!(
        "  definitely_lost: {} blocks · {} B",
        report.definitely_lost.block_count, report.definitely_lost.total_bytes
    );
    println!(
        "  possibly_lost:   {} blocks · {} B",
        report.possibly_lost.block_count, report.possibly_lost.total_bytes
    );
    println!(
        "  indirectly_lost: {} blocks · {} B",
        report.indirectly_lost.block_count, report.indirectly_lost.total_bytes
    );
    println!(
        "  still_reachable: {} blocks · {} B",
        report.still_reachable.block_count, report.still_reachable.total_bytes
    );
    for b in report.definitely_lost.top_blocks.iter().take(5) {
        println!("    leaked: {} B @ {:#x}", b.size, b.addr);
    }

    assert_eq!(report.allocator, "glibc");
    assert!(
        report.total_blocks > 0,
        "expected glibc walker to find heap blocks"
    );
    assert!(
        report.definitely_lost.block_count > 0,
        "steady_leak overwrites its only pointer each iteration, so expect \
         at least one definitely-lost block"
    );
}
