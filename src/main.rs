use std::{env, thread::sleep, time::Duration};

use system_monitor::{
    monitor::Monitor,
    parser::{NetworkParser, ProcessParser, ThreadParser, network_parser::TraitNetworkParser},
    util::ParseError,
};

fn print_network() -> Result<(), ParseError> {
    let parser = NetworkParser::new();
    let sockets = parser.get_all_net_socket_info()?;

    println!("network_test: parsed {} sockets", sockets.len());

    for socket in sockets.iter().take(5) {
        println!(
            "proto={:?} inode={} {}:{} -> {}:{} state={:?}",
            socket.protocol,
            socket.inode,
            socket.local_addr,
            socket.local_port,
            socket.remote_addr,
            socket.remote_port,
            socket.tcp_state
        );
    }

    Ok(())
}

fn parse_args() -> Result<(bool, Option<u32>), ParseError> {
    let mut show_threads = false;
    let mut pid_filter: Option<u32> = None;

    for arg in env::args().skip(1) {
        if arg == "-threads" || arg == "--threads" {
            show_threads = true;
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
            "unknown argument '{}'. supported: -threads, -pid=<pid>",
            arg
        )));
    }

    Ok((show_threads, pid_filter))
}

fn print_samples(
    monitor: &mut Monitor<ProcessParser, ThreadParser>,
    show_threads: bool,
    pid_filter: Option<u32>,
) -> Result<(), ParseError> {
    if show_threads {
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
    let (show_threads, pid_filter) = parse_args()?;

    print_network()?;

    let process_parser = ProcessParser::new();
    let thread_parser = ThreadParser::new();
    let mut monitor = Monitor::with_parsers(process_parser, thread_parser);

    // t0
    monitor.initialize_sampling()?;

    sleep(Duration::from_millis(2000));

    // t1
    print_samples(&mut monitor, show_threads, pid_filter)?;

    //println!("{:#?}", system_state);
    //println!("{:#?}", parser.get_status_info());

    Ok(())
}
