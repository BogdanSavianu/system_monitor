use std::{env, thread::sleep, time::Duration};

use system_monitor::{
    dto::ProcessHierarchyNodeDTO,
    monitor::Monitor,
    parser::{NetworkParser, ProcessParser, ThreadParser},
    util::ParseError,
};

fn parse_args() -> Result<(bool, bool, bool, Option<u32>), ParseError> {
    let mut show_threads = false;
    let mut show_network = false;
    let mut show_hierarchy = false;
    let mut pid_filter: Option<u32> = None;

    for arg in env::args().skip(1) {
        if arg == "-threads" || arg == "--threads" {
            show_threads = true;
            continue;
        }

        if arg == "-network" || arg == "--network" {
            show_network = true;
            continue;
        }

        if arg == "-hierarchy" || arg == "--hierarchy" {
            show_hierarchy = true;
            continue;
        }

        if let Some(pid_str) = arg
            .strip_prefix("-pid=")
            .or_else(|| arg.strip_prefix("--pid="))
        {
            let pid = pid_str.parse::<u32>().map_err(|err| {
                ParseError::ParsingError(format!("invalid pid '{}': {}", pid_str, err))
            })?;
            pid_filter = Some(pid);
            continue;
        }

        return Err(ParseError::ParsingError(format!(
            "unknown argument '{}'. supported: -threads, -network, -hierarchy, -pid=<pid>",
            arg
        )));
    }

    Ok((show_threads, show_network, show_hierarchy, pid_filter))
}

fn print_tree_nodes(nodes: &[ProcessHierarchyNodeDTO], depth: usize) {
    let indent = "  ".repeat(depth);
    for node in nodes {
        println!(
            "{}pid={} ppid={} name={}",
            indent, node.pid, node.ppid, node.name
        );
        print_tree_nodes(&node.children, depth + 1);
    }
}

fn print_samples(
    monitor: &mut Monitor<ProcessParser, ThreadParser, NetworkParser>,
    show_threads: bool,
    show_network: bool,
    show_hierarchy: bool,
    pid_filter: Option<u32>,
) -> Result<(), ParseError> {
    if show_hierarchy {
        let hierarchy_index = monitor.sample_process_hierarchy_indexes()?;
        println!("process_hierarchy_indexes");
        println!("roots={:?}", hierarchy_index.roots);
        println!("pid_to_ppid={:?}", hierarchy_index.pid_to_ppid);
        println!("children_by_pid={:?}", hierarchy_index.children_by_pid);

        let hierarchy_tree = monitor.sample_process_hierarchy_tree()?;
        println!("process_hierarchy_tree");
        print_tree_nodes(&hierarchy_tree, 0);
    } else if show_network {
        let network_samples = monitor.sample_process_network_stats()?;

        for sample in network_samples
            .iter()
            .filter(|sample| pid_filter.is_none_or(|pid| sample.pid == pid))
        {
            println!(
                "pid={} name={} tcp_open={} tcp_established={} tcp_listen={} udp_open={} sockets_total={}",
                sample.pid,
                sample.name,
                sample.tcp_open,
                sample.tcp_established,
                sample.tcp_listen,
                sample.udp_open,
                sample.total_sockets,
            );
        }
    } else if show_threads {
        let cpu_samples = monitor.sample_thread_cpu_usage()?;

        for sample in cpu_samples
            .iter()
            .filter(|sample| pid_filter.is_none_or(|pid| sample.pid == pid))
        {
            println!(
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
            );
        }
    } else {
        let cpu_samples = monitor.sample_cpu_usage()?;

        for sample in cpu_samples
            .iter()
            .filter(|sample| pid_filter.is_none_or(|pid| sample.pid == pid))
        {
            println!(
                "pid={} name={} cpu_norm={:.2}% cpu_top={:.2}% cpu_rel={:.2}%",
                sample.pid, sample.name, sample.cpu_norm, sample.cpu_top, sample.cpu_rel
            );
        }
    }

    Ok(())
}

fn main() -> Result<(), ParseError> {
    let (show_threads, show_network, show_hierarchy, pid_filter) = parse_args()?;

    let process_parser = ProcessParser::new();
    let thread_parser = ThreadParser::new();
    let network_parser = NetworkParser::new();
    let mut monitor = Monitor::with_parsers(process_parser, thread_parser, network_parser);

    // t0
    monitor.initialize_sampling()?;

    if !show_hierarchy {
        sleep(Duration::from_millis(2000));
    }

    // t1
    print_samples(
        &mut monitor,
        show_threads,
        show_network,
        show_hierarchy,
        pid_filter,
    )?;

    //println!("{:#?}", system_state);
    //println!("{:#?}", parser.get_status_info());

    Ok(())
}
