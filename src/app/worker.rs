use std::{
    io::{self, BufRead},
    sync::mpsc,
    thread,
    thread::sleep,
    time::Duration,
};

use system_monitor::util::ParseError;
use tracing::{error, info};

use super::{CliArgs, build_monitor, output};

enum WorkerCommand {
    Sample(CliArgs),
    Shutdown,
}

fn command_summary(command: &WorkerCommand) -> (&'static str, Option<u32>) {
    match command {
        WorkerCommand::Sample(args) if args.show_hierarchy => ("hierarchy", None),
        WorkerCommand::Sample(args) if args.show_network => ("network", args.pid_filter),
        WorkerCommand::Sample(args) if args.show_threads => ("threads", args.pid_filter),
        WorkerCommand::Sample(args) => ("cpu", args.pid_filter),
        WorkerCommand::Shutdown => ("shutdown", None),
    }
}

fn parse_worker_command(input: &str) -> Result<WorkerCommand, ParseError> {
    let mut parts = input.split_whitespace();
    let Some(command) = parts.next() else {
        return Err(ParseError::ParsingError("empty command".to_string()));
    };

    let mut args = CliArgs {
        show_threads: false,
        show_network: false,
        show_hierarchy: false,
        pid_filter: None,
    };

    for part in parts {
        if let Some(pid_str) = part
            .strip_prefix("pid=")
            .or_else(|| part.strip_prefix("--pid="))
        {
            let pid = pid_str.parse::<u32>().map_err(|err| {
                ParseError::ParsingError(format!("invalid pid '{}': {}", pid_str, err))
            })?;
            args.pid_filter = Some(pid);
            continue;
        }

        return Err(ParseError::ParsingError(format!(
            "unknown command argument '{}'",
            part
        )));
    }

    match command {
        "cpu" => Ok(WorkerCommand::Sample(args)),
        "threads" => {
            args.show_threads = true;
            Ok(WorkerCommand::Sample(args))
        }
        "network" => {
            args.show_network = true;
            Ok(WorkerCommand::Sample(args))
        }
        "hierarchy" => {
            args.show_hierarchy = true;
            Ok(WorkerCommand::Sample(args))
        }
        "quit" | "exit" => Ok(WorkerCommand::Shutdown),
        _ => Err(ParseError::ParsingError(format!(
            "unknown command '{}'. supported: cpu, threads, network, hierarchy, quit",
            command
        ))),
    }
}

fn worker_loop(
    cmd_rx: mpsc::Receiver<WorkerCommand>,
    res_tx: mpsc::Sender<Result<String, ParseError>>,
) {
    let mut monitor = build_monitor();

    if let Err(err) = monitor.initialize_sampling() {
        let _ = res_tx.send(Err(err));
        return;
    }

    info!(target: "app::worker", "worker initialized and waiting for commands");

    while let Ok(command) = cmd_rx.recv() {
        let (kind, pid_filter) = command_summary(&command);
        info!(
            target: "app::worker",
            command = kind,
            pid_filter = ?pid_filter,
            "worker received command"
        );

        match command {
            WorkerCommand::Sample(args) => {
                if !args.show_hierarchy {
                    sleep(Duration::from_millis(2000));
                }

                let result = output::render_samples_to_string(&mut monitor, &args);
                let _ = res_tx.send(result);
            }
            WorkerCommand::Shutdown => {
                info!(target: "app::worker", "worker shutdown requested");
                break;
            }
        }
    }

    monitor.flush_storage_pipeline();
}

pub fn run_interactive_worker_mode() -> Result<(), ParseError> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCommand>();
    let (res_tx, res_rx) = mpsc::channel::<Result<String, ParseError>>();

    let worker = thread::spawn(move || worker_loop(cmd_rx, res_tx));

    println!("worker mode started");
    println!(
        "commands: cpu [pid=<pid>], threads [pid=<pid>], network [pid=<pid>], hierarchy, quit"
    );

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line.map_err(|err| ParseError::ParsingError(err.to_string()))?;
        if line.trim().is_empty() {
            continue;
        }

        info!(
            target: "app::worker",
            raw_command = line.trim(),
            "command received from stdin"
        );

        let command = match parse_worker_command(&line) {
            Ok(command) => command,
            Err(err) => {
                println!("error: {:?}", err);
                info!(target: "app::worker", error = ?err, "invalid command ignored");
                continue;
            }
        };

        let should_shutdown = matches!(command, WorkerCommand::Shutdown);

        cmd_tx
            .send(command)
            .map_err(|_| ParseError::ParsingError("worker command channel closed".to_string()))?;

        if should_shutdown {
            break;
        }

        match res_rx.recv() {
            Ok(Ok(output)) => {
                println!("{}", output);
            }
            Ok(Err(err)) => {
                error!("worker command failed: {:?}", err);
                println!("error: {:?}", err);
            }
            Err(_) => {
                return Err(ParseError::ParsingError(
                    "worker response channel closed".to_string(),
                ));
            }
        }
    }

    let _ = cmd_tx.send(WorkerCommand::Shutdown);
    drop(cmd_tx);

    worker.join().map_err(|_| {
        ParseError::ParsingError("worker thread panicked while shutting down".to_string())
    })?;

    Ok(())
}
