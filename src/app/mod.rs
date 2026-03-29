mod args;
#[cfg(feature = "dioxus-demo")]
mod dioxus_demo;
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
use tracing::info;

pub use args::CliArgs;

type AppMonitor = Monitor<ProcessParser, ThreadParser, NetworkParser>;

fn build_monitor() -> AppMonitor {
    Monitor::with_parsers(
        ProcessParser::new(),
        ThreadParser::new(),
        NetworkParser::new(),
    )
}

pub fn run() -> Result<(), ParseError> {
    logging::init_logging();

    if args::is_dioxus_demo_requested() {
        info!(target: "app::runtime", "starting in dioxus demo mode");
        #[cfg(feature = "dioxus-demo")]
        return dioxus_demo::run_dioxus_demo();

        #[cfg(not(feature = "dioxus-demo"))]
        return Err(ParseError::ParsingError(
            "dioxus demo feature is disabled; run with `cargo run --features dioxus-demo -- --dioxus-demo` after installing pkg-config and GTK/WebKit dev libs"
                .to_string(),
        ));
    }

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
