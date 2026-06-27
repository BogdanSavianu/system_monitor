//! end-to-end allocation-scan capture test against a real leaker. needs root and
//! bpftrace, so it's ignored by default

#![cfg(feature = "leakprobe")]

use std::io::{BufRead, BufReader};
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use system_monitor::leakprobe::{
    AllocationProfiler, BpftraceProfiler, ContinuousScanState, LeakVerdict, Symbolizer,
    process_continuous_chunk,
};

#[test]
#[ignore = "needs root/CAP_BPF and bpftrace"]
fn continuous_capture_detects_a_real_leak() {
    let profiler = BpftraceProfiler::new();
    assert!(
        profiler.is_available(),
        "profiler unavailable, run as root and ensure bpftrace is installed"
    );

    // 64 kb allocations held in a list forever
    let python = ["/usr/bin/python3", "/bin/python3"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
        .unwrap_or("python3");
    eprintln!("[test] leaker python = {python}");
    let mut child = Command::new(python)
        .args([
            "-c",
            "l=[]\nimport time\nwhile True:\n l.append(bytearray(65536))\n time.sleep(0.01)",
        ])
        .spawn()
        .expect("failed to spawn python leaker");
    sleep(Duration::from_secs(2));

    // start the continuous capture
    let (mut bpf, mut symbolizer) = profiler
        .start_continuous(child.id(), 2)
        .expect("start_continuous failed");
    let stdout = bpf.stdout.take().expect("bpftrace stdout piped");
    let reader = BufReader::new(stdout);
    let mut state = ContinuousScanState::new(child.id(), "python-leaker".to_string(), 2);
    let mut chunk = String::new();
    let mut last_report = None;

    // read enough snapshots to show growth (a verdict needs at least two)
    let mut snapshots_seen = 0usize;
    for line in reader.lines() {
        let line = line.expect("read bpftrace stdout");
        chunk.push_str(&line);
        chunk.push('\n');
        if line.trim() != "--- END_SNAPSHOT ---" {
            continue;
        }
        if let Some(report) = process_continuous_chunk(&chunk, &mut state, &mut symbolizer) {
            snapshots_seen += 1;
            last_report = Some(report);
        }
        chunk.clear();
        if snapshots_seen >= 3 {
            break;
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    unsafe {
        libc::kill(bpf.id() as i32, libc::SIGTERM);
    }
    let _ = bpf.wait();

    let report = last_report.expect("at least one snapshot");
    println!(
        "snapshots_seen={snapshots_seen} verdict={:?} total_outstanding={} bytes",
        report.verdict, report.total_outstanding_bytes
    );

    assert!(
        report.total_outstanding_bytes > 0,
        "expected some outstanding allocations"
    );
    assert_eq!(
        report.verdict,
        LeakVerdict::Leaking,
        "a steadily growing leak should be classified as leaking"
    );
}
