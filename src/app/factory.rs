use std::{env, path::PathBuf, time::Duration};

use system_monitor::{
    ml::MemoryLeakDetector,
    monitor::Monitor,
    parser::{NetworkParser, ProcessParser, ThreadParser},
    storage::{DefaultSampleAccumulator, SqliteSink, StorageSink},
    util::ParseError,
};
use tracing::{info, warn};
use uuid::Uuid;

pub type AppMonitor = Monitor<ProcessParser, ThreadParser, NetworkParser>;

#[derive(Debug, Clone)]
pub struct MonitorBuildSettings {
    pub storage_enabled: bool,
    pub anomaly_enabled: bool,
    pub storage_db_path: PathBuf,
    pub anomaly_model_path: PathBuf,
    pub anomaly_window_size: usize,
}

impl MonitorBuildSettings {
    pub fn from_env() -> Self {
        Self {
            storage_enabled: parse_bool_env("SM_ENABLE_STORAGE"),
            anomaly_enabled: parse_bool_env("SM_ENABLE_ANOMALY"),
            storage_db_path: env::var("SM_STORAGE_DB_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| default_storage_db_path()),
            anomaly_model_path: env::var("SM_MODEL_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| default_model_path()),
            anomaly_window_size: parse_usize_env("SM_ANOMALY_WINDOW").unwrap_or(24),
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
    let leak_detector = if settings.anomaly_enabled {
        match MemoryLeakDetector::load_from_path(
            &settings.anomaly_model_path,
            settings.anomaly_window_size,
        ) {
            Ok(detector) => {
                info!(
                    target: "app::factory",
                    model_path = %settings.anomaly_model_path.display(),
                    window = settings.anomaly_window_size,
                    "memory leak detector initialized"
                );
                Some(detector)
            }
            Err(err) => {
                warn!(
                    target: "app::factory",
                    model_path = %settings.anomaly_model_path.display(),
                    error = %err,
                    "failed to initialize memory leak detector; anomaly mode disabled for this run"
                );
                None
            }
        }
    } else {
        None
    };

    if !settings.effective_storage_enabled() {
        return Ok(Monitor::with_parsers_pipeline_and_detector(
            process_parser,
            thread_parser,
            network_parser,
            None,
            None,
            leak_detector,
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

    Ok(Monitor::with_parsers_pipeline_and_detector(
        process_parser,
        thread_parser,
        network_parser,
        Some(accumulator),
        Some(sink),
        leak_detector,
    ))
}

fn parse_bool_env(key: &str) -> bool {
    env::var(key)
        .map(|v| v.trim().eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn parse_usize_env(key: &str) -> Option<usize> {
    env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
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

fn default_model_path() -> PathBuf {
    PathBuf::from("leak_model.json")
}
