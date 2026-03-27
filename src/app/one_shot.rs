use std::{thread::sleep, time::Duration};

use system_monitor::util::ParseError;
use tracing::info;

use super::{CliArgs, build_monitor, output};

pub fn run_once(args: &CliArgs) -> Result<(), ParseError> {
    let mut monitor = build_monitor();

    info!(target: "app::runtime", "starting one-shot mode");

    monitor.initialize_sampling()?;

    if !args.show_hierarchy {
        sleep(Duration::from_millis(2000));
    }

    output::print_samples(&mut monitor, args)
}
