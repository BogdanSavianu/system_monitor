//! Headless driver for the reachability scanner, used to compare findings
//! against Valgrind.
//!
//! Needs root
//!
//! Usage:
//!   cargo run --example reachability_scan --features leakprobe -- \
//!       <fixture> [fixture-args...]

#[cfg(not(feature = "leakprobe"))]
fn main() {
    eprintln!("rebuild with --features leakprobe");
    std::process::exit(2);
}

#[cfg(feature = "leakprobe")]
fn main() {
    use std::process::Command;
    use std::thread::sleep;
    use std::time::Duration;

    use system_monitor::leakprobe::{GcoreReachabilityScanner, ReachabilityProfiler};

    let mut args = std::env::args().skip(1);
    let Some(fixture) = args.next() else {
        eprintln!("usage: reachability_scan <fixture> [fixture-args...]");
        std::process::exit(2);
    };
    let fixture_args: Vec<String> = args.collect();
    let scan_after_s: f64 = std::env::var("SCAN_AFTER_S")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8.0);

    let scanner = GcoreReachabilityScanner::new();
    if !scanner.is_available() {
        eprintln!(
            "scanner unavailable: needs root or CAP_SYS_PTRACE and gcore on PATH \
             (current ptrace_scope may also block it)"
        );
        std::process::exit(1);
    }

    let mut child = Command::new(&fixture)
        .args(&fixture_args)
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn {fixture}: {e}"));
    let pid = child.id();

    sleep(Duration::from_secs_f64(scan_after_s));

    let name = std::path::Path::new(&fixture)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| fixture.clone());
    let result = scanner.reachability_scan(pid, &name);

    let _ = child.kill();
    let _ = child.wait();

    let report = match result {
        Ok(r) => r,
        Err(e) => {
            eprintln!("reachability scan failed: {e}");
            std::process::exit(1);
        }
    };

    println!("allocator={}", report.allocator);
    println!(
        "total: {} bytes in {} blocks",
        report.total_bytes, report.total_blocks
    );
    println!(
        "definitely_lost: {} bytes in {} blocks",
        report.definitely_lost.total_bytes, report.definitely_lost.block_count
    );
    println!(
        "indirectly_lost: {} bytes in {} blocks",
        report.indirectly_lost.total_bytes, report.indirectly_lost.block_count
    );
    println!(
        "possibly_lost: {} bytes in {} blocks",
        report.possibly_lost.total_bytes, report.possibly_lost.block_count
    );
    println!(
        "still_reachable: {} bytes in {} blocks",
        report.still_reachable.total_bytes, report.still_reachable.block_count
    );
}
