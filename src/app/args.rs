use std::env;

use system_monitor::util::ParseError;

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub show_threads: bool,
    pub show_network: bool,
    pub show_hierarchy: bool,
    pub pid_filter: Option<u32>,
}

pub fn has_cli_args() -> bool {
    env::args().nth(1).is_some()
}

pub fn is_worker_mode_requested() -> bool {
    let args: Vec<String> = env::args().skip(1).collect();
    args.len() == 1 && (args[0] == "--worker" || args[0] == "-worker")
}

pub fn is_dioxus_demo_requested() -> bool {
    let args: Vec<String> = env::args().skip(1).collect();
    args.len() == 1 && args[0] == "--dioxus-demo"
}

pub fn parse_args() -> Result<CliArgs, ParseError> {
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

    Ok(CliArgs {
        show_threads,
        show_network,
        show_hierarchy,
        pid_filter,
    })
}
