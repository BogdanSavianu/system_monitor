use std::{fs, sync::OnceLock};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt};

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

pub fn init_logging() {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    if let Err(err) = fs::create_dir_all("logs") {
        eprintln!("failed to create logs directory: {err}");
    }

    let file_appender = tracing_appender::rolling::never("logs", "system-monitor.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let _ = fmt()
        .with_env_filter(env_filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .try_init();
}
