mod args;
mod logging;
mod one_shot;
mod output;
mod worker;

use system_monitor::{
    monitor::Monitor,
    parser::{NetworkParser, ProcessParser, ThreadParser},
    util::ParseError,
};
use tracing::info;

pub use args::CliArgs;

type AppMonitor = Monitor<ProcessParser, ThreadParser, NetworkParser>;

fn build_monitor() -> AppMonitor {
    Monitor::with_parsers(ProcessParser::new(), ThreadParser::new(), NetworkParser::new())
}

pub fn run() -> Result<(), ParseError> {
    logging::init_logging();

    if !args::has_cli_args() {
        info!(target: "app::runtime", "starting in worker mode");
        return worker::run_interactive_worker_mode();
    }

    let parsed = args::parse_args()?;
    info!(target: "app::runtime", "starting in one-shot mode");
    one_shot::run_once(&parsed)
}
