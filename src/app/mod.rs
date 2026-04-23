mod args;
mod factory;
mod gui;
mod logging;
mod one_shot;
mod output;
mod worker;

use system_monitor::{
    monitor::Monitor,
    parser::{NetworkParser, ProcessParser, ThreadParser},
    util::ParseError,
};
use tracing::{info, warn};

pub use args::CliArgs;

type AppMonitor = Monitor<ProcessParser, ThreadParser, NetworkParser>;

fn build_monitor() -> AppMonitor {
    let settings = factory::MonitorBuildSettings::from_env();
    build_monitor_with_settings(settings)
}

fn build_monitor_with_settings(settings: factory::MonitorBuildSettings) -> AppMonitor {
    match factory::build_monitor(&settings) {
        Ok(monitor) => monitor,
        Err(err) => {
            warn!(
                target: "app::factory",
                error = ?err,
                "falling back to monitor without storage pipeline"
            );
            Monitor::with_parsers(
                ProcessParser::new(),
                ThreadParser::new(),
                NetworkParser::new(),
            )
        }
    }
}

pub fn run() -> Result<(), ParseError> {
    logging::init_logging();

    if !args::has_cli_args() {
        info!(target: "app::runtime", "starting in gui mode");
        return gui::run_gui_mode();
    }

    if args::is_worker_mode_requested() {
        info!(target: "app::runtime", "starting in worker mode");
        return worker::run_interactive_worker_mode();
    }

    let parsed = args::parse_args()?;
    info!(target: "app::runtime", "starting in one-shot mode");
    one_shot::run_once(&parsed)
}
