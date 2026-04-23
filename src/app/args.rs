use std::env;

use system_monitor::util::ParseError;

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub show_threads: bool,
    pub show_network: bool,
    pub show_hierarchy: bool,
    pub pid_filter: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct TrainAnomalyArgs {
    pub csv_paths: Vec<String>,
    pub window: usize,
    pub train_ratio: f64,
}

pub fn has_cli_args() -> bool {
    env::args().nth(1).is_some()
}

pub fn is_worker_mode_requested() -> bool {
    let args: Vec<String> = env::args().skip(1).collect();
    args.len() == 1 && (args[0] == "--worker" || args[0] == "-worker")
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

pub fn parse_train_anomaly_args() -> Result<Option<TrainAnomalyArgs>, ParseError> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return Ok(None);
    }

    let mut csv_paths: Option<Vec<String>> = None;
    let mut window: usize = 12;
    let mut train_ratio: f64 = 0.8;

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        if arg == "--train-anomaly" {
            let next = args.get(i + 1).ok_or_else(|| {
                ParseError::ParsingError("--train-anomaly expects a comma-separated path list".to_string())
            })?;
            let values = next
                .split(',')
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect::<Vec<_>>();
            if values.is_empty() {
                return Err(ParseError::ParsingError(
                    "--train-anomaly requires at least one csv path".to_string(),
                ));
            }
            csv_paths = Some(values);
            i += 2;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--train-anomaly=") {
            let values = value
                .split(',')
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect::<Vec<_>>();
            if values.is_empty() {
                return Err(ParseError::ParsingError(
                    "--train-anomaly requires at least one csv path".to_string(),
                ));
            }
            csv_paths = Some(values);
            i += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--window=") {
            window = value.parse::<usize>().map_err(|err| {
                ParseError::ParsingError(format!("invalid --window '{}': {}", value, err))
            })?;
            i += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--train-ratio=") {
            train_ratio = value.parse::<f64>().map_err(|err| {
                ParseError::ParsingError(format!("invalid --train-ratio '{}': {}", value, err))
            })?;
            i += 1;
            continue;
        }

        i += 1;
    }

    let Some(csv_paths) = csv_paths else {
        return Ok(None);
    };

    if window < 2 {
        return Err(ParseError::ParsingError("--window must be >= 2".to_string()));
    }

    if !(0.1..=0.95).contains(&train_ratio) {
        return Err(ParseError::ParsingError(
            "--train-ratio must be in [0.1, 0.95]".to_string(),
        ));
    }

    Ok(Some(TrainAnomalyArgs {
        csv_paths,
        window,
        train_ratio,
    }))
}
