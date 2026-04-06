use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use rusqlite::{Connection, params};

use super::{PersistedSampleBatch, StorageError, StorageResult, StorageSink};

pub struct SqliteSink {
    conn: Connection,
}

impl SqliteSink {
    pub fn new<P: AsRef<Path>>(db_path: P) -> StorageResult<Self> {
        let db_path = db_path.as_ref();
        ensure_parent_dir(db_path)?;

        let conn = Connection::open(db_path)?;
        let mut sink = Self { conn };
        sink.init()?;
        Ok(sink)
    }

    pub fn new_in_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory()?;
        let mut sink = Self { conn };
        sink.init()?;
        Ok(sink)
    }

    fn init_schema(&mut self) -> StorageResult<()> {
        self.conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                first_seen_ms INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS process_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                collected_at_ms INTEGER NOT NULL,
                session_id TEXT NOT NULL,
                pid INTEGER NOT NULL,
                start_time_ticks INTEGER,
                executable_path TEXT,
                cmdline_hash TEXT,
                name TEXT NOT NULL,
                cmdline TEXT NOT NULL,
                cpu_top REAL NOT NULL,
                cpu_rel REAL NOT NULL,
                virtual_mem_kb INTEGER NOT NULL,
                physical_mem_kb INTEGER NOT NULL,
                thread_count INTEGER NOT NULL,
                FOREIGN KEY(session_id) REFERENCES sessions(session_id)
            );

            CREATE TABLE IF NOT EXISTS network_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                collected_at_ms INTEGER NOT NULL,
                session_id TEXT NOT NULL,
                pid INTEGER NOT NULL,
                start_time_ticks INTEGER,
                tcp_open INTEGER NOT NULL,
                tcp_established INTEGER NOT NULL,
                tcp_listen INTEGER NOT NULL,
                udp_open INTEGER NOT NULL,
                total_sockets INTEGER NOT NULL,
                FOREIGN KEY(session_id) REFERENCES sessions(session_id)
            );

            CREATE INDEX IF NOT EXISTS idx_process_session_time
                ON process_samples(session_id, collected_at_ms);
            CREATE INDEX IF NOT EXISTS idx_process_pid_time
                ON process_samples(pid, collected_at_ms);
            CREATE INDEX IF NOT EXISTS idx_network_session_time
                ON network_samples(session_id, collected_at_ms);
            CREATE INDEX IF NOT EXISTS idx_network_pid_time
                ON network_samples(pid, collected_at_ms);
            ",
        )?;

        Ok(())
    }
}

impl StorageSink for SqliteSink {
    fn init(&mut self) -> StorageResult<()> {
        self.init_schema()
    }

    fn persist_sample_batch(&mut self, batch: &PersistedSampleBatch) -> StorageResult<()> {
        let collected_at_ms = system_time_to_unix_ms(batch.collected_at)?;
        let session_id = batch.session_id.to_string();

        let tx = self.conn.transaction()?;

        tx.execute(
            "
            INSERT INTO sessions (session_id, first_seen_ms)
            VALUES (?1, ?2)
            ON CONFLICT(session_id) DO NOTHING
            ",
            params![session_id, collected_at_ms],
        )?;

        for process in &batch.processes {
            tx.execute(
                "
                INSERT INTO process_samples (
                    collected_at_ms,
                    session_id,
                    pid,
                    start_time_ticks,
                    executable_path,
                    cmdline_hash,
                    name,
                    cmdline,
                    cpu_top,
                    cpu_rel,
                    virtual_mem_kb,
                    physical_mem_kb,
                    thread_count
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                ",
                params![
                    collected_at_ms,
                    session_id,
                    i64::from(process.identity.pid),
                    process.identity.start_time_ticks.map(|v| v.to_string()),
                    process.fingerprint.executable_path,
                    process.fingerprint.cmdline_hash.map(|v| v.to_string()),
                    process.name,
                    process.cmdline,
                    process.cpu_top,
                    process.cpu_rel,
                    i64::from(process.virtual_mem),
                    i64::from(process.physical_mem),
                    i64::from(process.thread_count),
                ],
            )?;
        }

        for network in &batch.network {
            tx.execute(
                "
                INSERT INTO network_samples (
                    collected_at_ms,
                    session_id,
                    pid,
                    start_time_ticks,
                    tcp_open,
                    tcp_established,
                    tcp_listen,
                    udp_open,
                    total_sockets
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ",
                params![
                    collected_at_ms,
                    session_id,
                    i64::from(network.identity.pid),
                    network.identity.start_time_ticks.map(|v| v.to_string()),
                    i64::from(network.tcp_open),
                    i64::from(network.tcp_established),
                    i64::from(network.tcp_listen),
                    i64::from(network.udp_open),
                    i64::from(network.total_sockets),
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    fn flush(&mut self) -> StorageResult<()> {
        self.conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);")?;
        Ok(())
    }
}

fn ensure_parent_dir(db_path: &Path) -> StorageResult<()> {
    let parent: Option<PathBuf> = db_path.parent().map(Path::to_path_buf);
    if let Some(parent) = parent
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn system_time_to_unix_ms(collected_at: std::time::SystemTime) -> StorageResult<i64> {
    let duration = collected_at
        .duration_since(UNIX_EPOCH)
        .map_err(|err| StorageError::InvalidData(err.to_string()))?;

    i64::try_from(duration.as_millis())
        .map_err(|_| StorageError::InvalidData("timestamp does not fit i64".to_string()))
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use uuid::Uuid;

    use crate::storage::{
        PersistedNetworkSample, PersistedProcessSample, PersistedSampleBatch, ProcessFingerprint,
        ProcessIdentity, SqliteSink, StorageSink,
    };

    fn sample_batch(session_id: Uuid, pid: u32) -> PersistedSampleBatch {
        PersistedSampleBatch {
            collected_at: SystemTime::now(),
            session_id,
            processes: vec![PersistedProcessSample {
                identity: ProcessIdentity {
                    pid,
                    start_time_ticks: Some(1234),
                },
                fingerprint: ProcessFingerprint {
                    executable_path: Some("/usr/bin/app".to_string()),
                    cmdline_hash: Some(42),
                },
                name: "app".to_string(),
                cmdline: "app --flag".to_string(),
                cpu_top: 12.5,
                cpu_rel: 30.0,
                virtual_mem: 2048,
                physical_mem: 1024,
                thread_count: 7,
            }],
            network: vec![PersistedNetworkSample {
                identity: ProcessIdentity {
                    pid,
                    start_time_ticks: Some(1234),
                },
                tcp_open: 2,
                tcp_established: 1,
                tcp_listen: 1,
                udp_open: 0,
                total_sockets: 2,
            }],
        }
    }

    fn count_rows(sink: &SqliteSink, table: &str) -> i64 {
        sink.conn
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count query should succeed")
    }

    #[test]
    fn new_in_memory_initializes_sink() {
        let sink = SqliteSink::new_in_memory();
        assert!(sink.is_ok());
    }

    #[test]
    fn init_creates_expected_tables() {
        let sink = SqliteSink::new_in_memory().expect("sink should initialize");

        for table in ["sessions", "process_samples", "network_samples"] {
            let exists: i64 = sink
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("sqlite_master query should succeed");

            assert_eq!(exists, 1, "table {table} should exist");
        }
    }

    #[test]
    fn persist_single_batch_inserts_process_and_network_rows() {
        let mut sink = SqliteSink::new_in_memory().expect("sink should initialize");
        let batch = sample_batch(Uuid::new_v4(), 123);

        sink.persist_sample_batch(&batch)
            .expect("persist should succeed");

        assert_eq!(count_rows(&sink, "sessions"), 1);
        assert_eq!(count_rows(&sink, "process_samples"), 1);
        assert_eq!(count_rows(&sink, "network_samples"), 1);
    }

    #[test]
    fn same_session_id_is_not_duplicated() {
        let mut sink = SqliteSink::new_in_memory().expect("sink should initialize");
        let session_id = Uuid::new_v4();

        let batch1 = sample_batch(session_id, 111);
        let batch2 = sample_batch(session_id, 222);

        sink.persist_sample_batch(&batch1)
            .expect("first persist should succeed");
        sink.persist_sample_batch(&batch2)
            .expect("second persist should succeed");

        assert_eq!(count_rows(&sink, "sessions"), 1);
        assert_eq!(count_rows(&sink, "process_samples"), 2);
        assert_eq!(count_rows(&sink, "network_samples"), 2);
    }
}
