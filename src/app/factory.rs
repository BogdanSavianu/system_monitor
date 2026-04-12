use std::{env, path::PathBuf, time::Duration};

use system_monitor::{
    monitor::Monitor,
    parser::{NetworkParser, ProcessParser, ThreadParser},
    storage::{DefaultSampleAccumulator, SqliteSink, StorageSink},
    util::ParseError,
};
use tracing::info;
use uuid::Uuid;

pub type AppMonitor = Monitor<ProcessParser, ThreadParser, NetworkParser>;

#[derive(Debug, Clone)]
pub struct MonitorBuildSettings {
    pub storage_enabled: bool,
    pub anomaly_enabled: bool,
    pub storage_db_path: PathBuf,
}

impl MonitorBuildSettings {
    pub fn from_env() -> Self {
        Self {
            storage_enabled: parse_bool_env("SM_ENABLE_STORAGE"),
            anomaly_enabled: parse_bool_env("SM_ENABLE_ANOMALY"),
            storage_db_path: env::var("SM_STORAGE_DB_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| default_storage_db_path()),
        }
    }

    pub fn effective_storage_enabled(&self) -> bool {
        self.storage_enabled || self.anomaly_enabled
    }

    #[cfg(feature = "dioxus-gui")]
    pub fn with_toggles(mut self, storage_enabled: bool, anomaly_enabled: bool) -> Self {
        self.storage_enabled = storage_enabled;
        self.anomaly_enabled = anomaly_enabled;
        self
    }
}

pub fn build_monitor(settings: &MonitorBuildSettings) -> Result<AppMonitor, ParseError> {
    let process_parser = ProcessParser::new();
    let thread_parser = ThreadParser::new();
    let network_parser = NetworkParser::new();

    if !settings.effective_storage_enabled() {
        return Ok(Monitor::with_parsers(
            process_parser,
            thread_parser,
            network_parser,
        ));
    }

    if settings.anomaly_enabled && !settings.storage_enabled {
        info!(
            target: "app::factory",
            "anomaly detection requested; enabling storage pipeline automatically"
        );
    }

    let sink = SqliteSink::new(&settings.storage_db_path).map_err(|err| {
        ParseError::ParsingError(format!(
            "failed to initialize sqlite sink at {}: {}",
            settings.storage_db_path.display(),
            err
        ))
    })?;

    let sink: Box<dyn StorageSink + Send> = Box::new(sink);
    let accumulator = Box::new(DefaultSampleAccumulator::new(
        Uuid::new_v4(),
        Duration::from_secs(15),
    ));

    Ok(Monitor::with_parsers_and_pipeline(
        process_parser,
        thread_parser,
        network_parser,
        Some(accumulator),
        Some(sink),
    ))
}

fn parse_bool_env(key: &str) -> bool {
    env::var(key)
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn default_storage_db_path() -> PathBuf {
    if let Some(home) = env::var_os("HOME") {
        PathBuf::from(home)
            .join(".system-monitor")
            .join("history.db")
    } else {
        PathBuf::from(".system-monitor").join("history.db")
    }
}