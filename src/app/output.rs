use system_monitor::{
    dto::ProcessHierarchyNodeDTO,
    util::ParseError,
};

use super::{AppMonitor, CliArgs};

pub fn print_samples(monitor: &mut AppMonitor, args: &CliArgs) -> Result<(), ParseError> {
    let output = render_samples_to_string(monitor, args)?;
    if !output.is_empty() {
        println!("{}", output);
    }

    Ok(())
}

pub fn render_samples_to_string(
    monitor: &mut AppMonitor,
    args: &CliArgs,
) -> Result<String, ParseError> {
    let mut lines: Vec<String> = Vec::new();

    if args.show_hierarchy {
        let hierarchy_index = monitor.sample_process_hierarchy_indexes()?;
        lines.push("process_hierarchy_indexes".to_string());
        lines.push(format!("roots={:?}", hierarchy_index.roots));
        lines.push(format!("pid_to_ppid={:?}", hierarchy_index.pid_to_ppid));
        lines.push(format!(
            "children_by_pid={:?}",
            hierarchy_index.children_by_pid
        ));

        let hierarchy_tree = monitor.sample_process_hierarchy_tree()?;
        lines.push("process_hierarchy_tree".to_string());
        collect_tree_lines(&hierarchy_tree, 0, &mut lines);
    } else if args.show_network {
        let network_samples = monitor.sample_process_network_stats()?;
        for sample in network_samples
            .iter()
            .filter(|sample| args.pid_filter.is_none_or(|pid| sample.pid == pid))
        {
            lines.push(format!(
                "pid={} name={} tcp_open={} tcp_established={} tcp_listen={} udp_open={} sockets_total={}",
                sample.pid,
                sample.name,
                sample.tcp_open,
                sample.tcp_established,
                sample.tcp_listen,
                sample.udp_open,
                sample.total_sockets,
            ));
        }
    } else if args.show_threads {
        let cpu_samples = monitor.sample_thread_cpu_usage()?;
        for sample in cpu_samples
            .iter()
            .filter(|sample| args.pid_filter.is_none_or(|pid| sample.pid == pid))
        {
            lines.push(format!(
                "pid={} tid={} process_name={} thread_name={} cpu_norm={:.2}% cpu_top={:.2}% state={:?} last_cpu={:?} ctxsw_v={:?} ctxsw_nv={:?} io_read_bytes={:?} io_write_bytes={:?} io_rchar={:?} io_wchar={:?} io_syscr={:?} io_syscw={:?}",
                sample.pid,
                sample.tid,
                sample.process_name,
                sample.thread_name,
                sample.cpu_norm,
                sample.cpu_top,
                sample.state,
                sample.last_cpu,
                sample.voluntary_ctxt_switches,
                sample.nonvoluntary_ctxt_switches,
                sample.io_read_bytes,
                sample.io_write_bytes,
                sample.io_rchar,
                sample.io_wchar,
                sample.io_syscr,
                sample.io_syscw
            ));
        }
    } else {
        let cpu_samples = monitor.sample_cpu_usage()?;
        for sample in cpu_samples
            .iter()
            .filter(|sample| args.pid_filter.is_none_or(|pid| sample.pid == pid))
        {
            lines.push(format!(
                "pid={} name={} cpu_norm={:.2}% cpu_top={:.2}% cpu_rel={:.2}%",
                sample.pid, sample.name, sample.cpu_norm, sample.cpu_top, sample.cpu_rel
            ));
        }
    }

    Ok(lines.join("\n"))
}

fn collect_tree_lines(nodes: &[ProcessHierarchyNodeDTO], depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    for node in nodes {
        lines.push(format!(
            "{}pid={} ppid={} name={}",
            indent, node.pid, node.ppid, node.name
        ));
        collect_tree_lines(&node.children, depth + 1, lines);
    }
}
