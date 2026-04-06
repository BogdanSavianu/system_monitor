use std::{fmt::Display, time::SystemTime};
use uuid::Uuid;

use crate::util::{Pid, Pm, Vm};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: Pid,
    pub start_time_ticks: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessFingerprint {
    pub executable_path: Option<String>,
    pub cmdline_hash: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistedSampleBatch {
    pub collected_at: SystemTime,
    pub session_id: Uuid,
    pub processes: Vec<PersistedProcessSample>,
    pub network: Vec<PersistedNetworkSample>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistedProcessSample {
    pub identity: ProcessIdentity,
    pub fingerprint: ProcessFingerprint,
    pub name: String,
    pub cmdline: String,
    pub cpu_top: f64,
    pub cpu_rel: f64,
    pub virtual_mem: Vm,
    pub physical_mem: Pm,
    pub thread_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PersistedNetworkSample {
    pub identity: ProcessIdentity,
    pub tcp_open: u32,
    pub tcp_established: u32,
    pub tcp_listen: u32,
    pub udp_open: u32,
    pub total_sockets: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StorageError {
    Sqlite(String),
    Io(String),
    InvalidData(String),
}

pub type StorageResult<T> = Result<T, StorageError>;

impl Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(msg) => write!(f, "sqlite error: {}", msg),
            Self::Io(msg) => write!(f, "io error: {}", msg),
            Self::InvalidData(msg) => write!(f, "invalid data: {}", msg),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sqlite(err.to_string())
    }
}
