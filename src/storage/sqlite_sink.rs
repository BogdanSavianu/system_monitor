use std::{
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use rusqlite::{Connection, params};

use std::collections::HashMap;

use super::{
    AllocationSnapshot, AllocationSnapshotRow, PersistedAllocScan, PersistedReachabilityScan,
    PersistedSampleBatch, StorageError, StorageResult, StorageSink, StoredLeakedBlock,
    StoredProcessMemoryPoint, StoredReachabilityScan, StoredSessionInfo,
};
use crate::util::Pid;

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
            CREATE INDEX IF NOT EXISTS idx_process_time
                ON process_samples(collected_at_ms);
            CREATE INDEX IF NOT EXISTS idx_network_session_time
                ON network_samples(session_id, collected_at_ms);
            CREATE INDEX IF NOT EXISTS idx_network_pid_time
                ON network_samples(pid, collected_at_ms);

            CREATE TABLE IF NOT EXISTS alloc_scans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pid INTEGER NOT NULL,
                name TEXT NOT NULL,
                started_at_ms INTEGER NOT NULL,
                duration_s INTEGER NOT NULL,
                verdict TEXT NOT NULL,
                total_outstanding_bytes INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS alloc_scan_sites (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                scan_id INTEGER NOT NULL,
                frame_text TEXT NOT NULL,
                outstanding_bytes INTEGER NOT NULL,
                outstanding_count INTEGER NOT NULL,
                growth_bytes_per_s REAL NOT NULL,
                FOREIGN KEY(scan_id) REFERENCES alloc_scans(id)
            );

            CREATE INDEX IF NOT EXISTS idx_alloc_scans_pid_time
                ON alloc_scans(pid, started_at_ms);
            CREATE INDEX IF NOT EXISTS idx_alloc_scan_sites_scan
                ON alloc_scan_sites(scan_id);

            CREATE TABLE IF NOT EXISTS reachability_scans (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pid INTEGER NOT NULL,
                name TEXT NOT NULL,
                started_at_ms INTEGER NOT NULL,
                duration_s INTEGER NOT NULL,
                allocator TEXT NOT NULL,
                total_blocks INTEGER NOT NULL,
                total_bytes INTEGER NOT NULL,
                definitely_lost_bytes INTEGER NOT NULL,
                possibly_lost_bytes INTEGER NOT NULL,
                indirectly_lost_bytes INTEGER NOT NULL,
                still_reachable_bytes INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS leaked_blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                scan_id INTEGER NOT NULL,
                addr INTEGER NOT NULL,
                size INTEGER NOT NULL,
                class TEXT NOT NULL,
                stack_text TEXT,
                FOREIGN KEY(scan_id) REFERENCES reachability_scans(id)
            );

            CREATE INDEX IF NOT EXISTS idx_reachability_scans_pid_time
                ON reachability_scans(pid, started_at_ms);
            CREATE INDEX IF NOT EXISTS idx_reachability_scans_name_time
                ON reachability_scans(name, started_at_ms);
            CREATE INDEX IF NOT EXISTS idx_leaked_blocks_scan
                ON leaked_blocks(scan_id);

            CREATE TABLE IF NOT EXISTS tier2_snapshots (
                id                       INTEGER PRIMARY KEY AUTOINCREMENT,
                pid                      INTEGER NOT NULL,
                name                     TEXT NOT NULL,
                collected_at_ms          INTEGER NOT NULL,
                snapshot_count           INTEGER NOT NULL,
                total_outstanding_bytes  INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_tier2_snapshots_name_time
                ON tier2_snapshots(name, collected_at_ms);
            CREATE INDEX IF NOT EXISTS idx_tier2_snapshots_pid_time
                ON tier2_snapshots(pid, collected_at_ms);
            ",
        )?;

        // migration: dbs from before stack_text lack the column, and sqlite's
        // ADD COLUMN has no IF NOT EXISTS, so check first.
        let has_stack_text: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('leaked_blocks') WHERE name = 'stack_text'",
            [],
            |row| row.get(0),
        )?;
        if has_stack_text == 0 {
            self.conn
                .execute("ALTER TABLE leaked_blocks ADD COLUMN stack_text TEXT", [])?;
        }

        // same trick for is_anomalous. NULL means unknown (pre-migration rows).
        let has_is_anomalous: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('process_samples') WHERE name = 'is_anomalous'",
            [],
            |row| row.get(0),
        )?;
        if has_is_anomalous == 0 {
            self.conn.execute(
                "ALTER TABLE process_samples ADD COLUMN is_anomalous INTEGER",
                [],
            )?;
        }

        Ok(())
    }
}

impl StorageSink for SqliteSink {
    fn init(&mut self) -> StorageResult<()> {
        self.init_schema()
    }

    fn truncate_history(&mut self) -> StorageResult<()> {
        // wipe every history table but keep the schema
        let tx = self.conn.transaction()?;
        for table in [
            "leaked_blocks",
            "reachability_scans",
            "alloc_scan_sites",
            "alloc_scans",
            "tier2_snapshots",
            "process_samples",
            "network_samples",
            "sessions",
        ] {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        tx.commit()?;
        // interesting fact about sqlite (and probably other engines)
        // when deleting entries, the pages where they were stored do not get freed,
        // instead they are kept so new subsequent entries already have a page ready for them.
        // the `VACUUM` command actually frees the storage
        self.conn.execute_batch("VACUUM")?;
        Ok(())
    }

    fn persist_reachability_scan(&mut self, scan: &PersistedReachabilityScan) -> StorageResult<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "
            INSERT INTO reachability_scans (
                pid, name, started_at_ms, duration_s, allocator,
                total_blocks, total_bytes,
                definitely_lost_bytes, possibly_lost_bytes,
                indirectly_lost_bytes, still_reachable_bytes
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ",
            params![
                scan.pid,
                scan.name,
                scan.started_at_ms,
                scan.duration_s as i64,
                scan.allocator,
                scan.total_blocks,
                scan.total_bytes as i64,
                scan.definitely_lost_bytes as i64,
                scan.possibly_lost_bytes as i64,
                scan.indirectly_lost_bytes as i64,
                scan.still_reachable_bytes as i64,
            ],
        )?;
        let scan_id = tx.last_insert_rowid();
        for block in &scan.top_blocks {
            tx.execute(
                "
                INSERT INTO leaked_blocks (scan_id, addr, size, class, stack_text)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    scan_id,
                    block.addr as i64,
                    block.size as i64,
                    block.class,
                    block.stack_text,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn persist_alloc_scan(&mut self, scan: &PersistedAllocScan) -> StorageResult<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "
            INSERT INTO alloc_scans (
                pid, name, started_at_ms, duration_s, verdict, total_outstanding_bytes
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                scan.pid,
                scan.name,
                scan.started_at_ms,
                scan.duration_s as i64,
                scan.verdict,
                scan.total_outstanding_bytes as i64,
            ],
        )?;
        let scan_id = tx.last_insert_rowid();
        for site in &scan.sites {
            tx.execute(
                "
                INSERT INTO alloc_scan_sites (
                    scan_id, frame_text, outstanding_bytes, outstanding_count, growth_bytes_per_s
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                params![
                    scan_id,
                    site.frame_text,
                    site.outstanding_bytes as i64,
                    site.outstanding_count as i64,
                    site.growth_bytes_per_s,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn persist_sample_batch(&mut self, batch: &PersistedSampleBatch) -> StorageResult<()> {
        let session_collected_at_ms = system_time_to_unix_ms(batch.collected_at)?;
        let session_id = batch.session_id.to_string();

        let tx = self.conn.transaction()?;

        tx.execute(
            "
            INSERT INTO sessions (session_id, first_seen_ms)
            VALUES (?1, ?2)
            ON CONFLICT(session_id) DO NOTHING
            ",
            params![session_id, session_collected_at_ms],
        )?;

        for process in &batch.processes {
            // per-sample timestamp so replay maps samples along the real time
            // axis, not in flush-interval clumps.
            let row_ms = system_time_to_unix_ms(process.collected_at)?;
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
                    thread_count,
                    is_anomalous
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                ",
                params![
                    row_ms,
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
                    process.is_anomalous.map(|b| if b { 1i64 } else { 0i64 }),
                ],
            )?;
        }

        for network in &batch.network {
            let row_ms = system_time_to_unix_ms(network.collected_at)?;
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
                    row_ms,
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

    fn query_memory_baselines(&self) -> StorageResult<HashMap<String, f64>> {
        let cutoff_ms = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64)
            - 7 * 24 * 3600 * 1000;

        let mut stmt = self.conn.prepare(
            "SELECT name, AVG(CAST(physical_mem_kb AS REAL))
             FROM process_samples
             WHERE collected_at_ms >= ?1
             GROUP BY name",
        )?;

        let rows = stmt.query_map(params![cutoff_ms], |row| {
            let name: String = row.get(0)?;
            let avg_kb: f64 = row.get(1)?;
            Ok((name, avg_kb))
        })?;

        let mut map = HashMap::new();
        for row in rows {
            let (name, avg_kb) = row?;
            map.insert(name, avg_kb / 1000.0); // convert to MB
        }
        Ok(map)
    }

    fn flush(&mut self) -> StorageResult<()> {
        self.conn.execute_batch("PRAGMA wal_checkpoint(PASSIVE);")?;
        Ok(())
    }

    fn query_process_memory_history(
        &self,
        pid: Pid,
        start_ms: i64,
        end_ms: i64,
    ) -> StorageResult<Vec<StoredProcessMemoryPoint>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT collected_at_ms, pid, name, physical_mem_kb, cpu_top, is_anomalous
            FROM process_samples
            WHERE pid = ?1 AND collected_at_ms >= ?2 AND collected_at_ms <= ?3
            ORDER BY collected_at_ms ASC
            ",
        )?;

        let rows = stmt.query_map(params![i64::from(pid), start_ms, end_ms], |row| {
            let pid_i64: i64 = row.get(1)?;
            let mem_i64: i64 = row.get(3)?;
            let cpu_top: f64 = row.get(4)?;
            let is_anomalous_opt: Option<i64> = row.get(5)?;

            Ok(StoredProcessMemoryPoint {
                collected_at_ms: row.get(0)?,
                pid: u32::try_from(pid_i64).unwrap_or_default(),
                name: row.get(2)?,
                physical_mem_kb: u64::try_from(mem_i64).unwrap_or_default(),
                cpu_top,
                is_anomalous: is_anomalous_opt.map(|v| v != 0).unwrap_or(false),
            })
        })?;

        let mut points = Vec::new();
        for point in rows {
            points.push(point?);
        }

        Ok(points)
    }

    fn query_process_memory_history_by_name(
        &self,
        name: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> StorageResult<Vec<StoredProcessMemoryPoint>> {
        let pattern = format!("%{}%", name);
        let mut stmt = self.conn.prepare(
            "
            SELECT collected_at_ms, pid, name, physical_mem_kb, cpu_top, is_anomalous
            FROM process_samples
            WHERE name LIKE ?1 AND collected_at_ms >= ?2 AND collected_at_ms <= ?3
            ORDER BY collected_at_ms ASC
            ",
        )?;

        let rows = stmt.query_map(params![pattern, start_ms, end_ms], |row| {
            let pid_i64: i64 = row.get(1)?;
            let mem_i64: i64 = row.get(3)?;
            let cpu_top: f64 = row.get(4)?;
            let is_anomalous_opt: Option<i64> = row.get(5)?;
            Ok(StoredProcessMemoryPoint {
                collected_at_ms: row.get(0)?,
                pid: u32::try_from(pid_i64).unwrap_or_default(),
                name: row.get(2)?,
                physical_mem_kb: u64::try_from(mem_i64).unwrap_or_default(),
                cpu_top,
                is_anomalous: is_anomalous_opt.map(|v| v != 0).unwrap_or(false),
            })
        })?;

        let mut points = Vec::new();
        for point in rows {
            points.push(point?);
        }
        Ok(points)
    }

    fn query_reachability_scans_by_name(
        &self,
        name: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> StorageResult<Vec<StoredReachabilityScan>> {
        let pattern = format!("%{}%", name);
        let mut stmt = self.conn.prepare(
            "
            SELECT id, pid, name, started_at_ms, duration_s, allocator,
                   total_blocks, total_bytes,
                   definitely_lost_bytes, possibly_lost_bytes,
                   indirectly_lost_bytes, still_reachable_bytes
            FROM reachability_scans
            WHERE name LIKE ?1 AND started_at_ms >= ?2 AND started_at_ms <= ?3
            ORDER BY started_at_ms ASC
            ",
        )?;

        let rows = stmt.query_map(params![pattern, start_ms, end_ms], |row| {
            let pid_i64: i64 = row.get(1)?;
            let duration_i64: i64 = row.get(4)?;
            let total_bytes_i64: i64 = row.get(7)?;
            let def_i64: i64 = row.get(8)?;
            let pos_i64: i64 = row.get(9)?;
            let ind_i64: i64 = row.get(10)?;
            let reach_i64: i64 = row.get(11)?;
            Ok(StoredReachabilityScan {
                id: row.get(0)?,
                pid: u32::try_from(pid_i64).unwrap_or_default(),
                name: row.get(2)?,
                started_at_ms: row.get(3)?,
                duration_s: u64::try_from(duration_i64).unwrap_or_default(),
                allocator: row.get(5)?,
                total_blocks: row.get(6)?,
                total_bytes: u64::try_from(total_bytes_i64).unwrap_or_default(),
                definitely_lost_bytes: u64::try_from(def_i64).unwrap_or_default(),
                possibly_lost_bytes: u64::try_from(pos_i64).unwrap_or_default(),
                indirectly_lost_bytes: u64::try_from(ind_i64).unwrap_or_default(),
                still_reachable_bytes: u64::try_from(reach_i64).unwrap_or_default(),
            })
        })?;

        let mut scans = Vec::new();
        for scan in rows {
            scans.push(scan?);
        }
        Ok(scans)
    }

    fn query_leaked_blocks_for_scan(&self, scan_id: i64) -> StorageResult<Vec<StoredLeakedBlock>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT addr, size, class, stack_text
            FROM leaked_blocks
            WHERE scan_id = ?1
            ORDER BY size DESC, id ASC
            ",
        )?;
        let rows = stmt.query_map(params![scan_id], |row| {
            let addr_i64: i64 = row.get(0)?;
            let size_i64: i64 = row.get(1)?;
            Ok(StoredLeakedBlock {
                addr: u64::try_from(addr_i64).unwrap_or_default(),
                size: u64::try_from(size_i64).unwrap_or_default(),
                class: row.get(2)?,
                stack_text: row.get(3)?,
            })
        })?;
        let mut blocks = Vec::new();
        for block in rows {
            blocks.push(block?);
        }
        Ok(blocks)
    }

    fn persist_allocation_snapshot(&mut self, row: &AllocationSnapshot) -> StorageResult<()> {
        self.conn.execute(
            "
            INSERT INTO tier2_snapshots (
                pid, name, collected_at_ms, snapshot_count, total_outstanding_bytes
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                i64::from(row.pid),
                row.name,
                row.collected_at_ms,
                row.snapshot_count as i64,
                row.total_outstanding_bytes as i64,
            ],
        )?;
        Ok(())
    }

    fn query_allocation_snapshots_by_name(
        &self,
        name: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> StorageResult<Vec<AllocationSnapshotRow>> {
        let pattern = format!("%{}%", name);
        let mut stmt = self.conn.prepare(
            "
            SELECT pid, name, collected_at_ms, snapshot_count, total_outstanding_bytes
            FROM tier2_snapshots
            WHERE name LIKE ?1 AND collected_at_ms >= ?2 AND collected_at_ms <= ?3
            ORDER BY collected_at_ms ASC
            ",
        )?;
        let rows = stmt.query_map(params![pattern, start_ms, end_ms], |row| {
            let pid_i64: i64 = row.get(0)?;
            let count_i64: i64 = row.get(3)?;
            let bytes_i64: i64 = row.get(4)?;
            Ok(AllocationSnapshotRow {
                pid: u32::try_from(pid_i64).unwrap_or_default(),
                name: row.get(1)?,
                collected_at_ms: row.get(2)?,
                snapshot_count: u64::try_from(count_i64).unwrap_or_default(),
                total_outstanding_bytes: u64::try_from(bytes_i64).unwrap_or_default(),
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    fn query_matching_process_names(
        &self,
        prefix: &str,
        limit: usize,
    ) -> StorageResult<Vec<String>> {
        let pattern = format!("%{}%", prefix);
        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT name FROM process_samples WHERE name LIKE ?1 ORDER BY name LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit_i64], |row| row.get(0))?;
        let mut names = Vec::new();
        for row in rows {
            names.push(row?);
        }
        Ok(names)
    }

    fn query_sessions_list(&self, limit: usize) -> StorageResult<Vec<StoredSessionInfo>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT s.session_id, s.first_seen_ms,
                   COALESCE(MAX(ps.collected_at_ms), s.first_seen_ms) AS last_seen_ms
            FROM sessions s
            LEFT JOIN process_samples ps ON ps.session_id = s.session_id
            GROUP BY s.session_id
            ORDER BY s.first_seen_ms DESC
            LIMIT ?1
            ",
        )?;

        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt.query_map(params![limit_i64], |row| {
            Ok(StoredSessionInfo {
                session_id: row.get(0)?,
                first_seen_ms: row.get(1)?,
                last_seen_ms: row.get(2)?,
            })
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        Ok(sessions)
    }

    fn query_sessions_list_for_name(
        &self,
        name: &str,
        limit: usize,
    ) -> StorageResult<Vec<StoredSessionInfo>> {
        // inner join drops sessions with no matching rows. last_seen_ms spans
        // all of the session's samples, not just matching ones, so the scrubber
        // covers the whole session even if the target exited mid-session.
        let pattern = format!("%{}%", name);
        let mut stmt = self.conn.prepare(
            "
            SELECT s.session_id, s.first_seen_ms,
                   COALESCE(MAX(ps_all.collected_at_ms), s.first_seen_ms) AS last_seen_ms
            FROM sessions s
            INNER JOIN process_samples ps_match
              ON ps_match.session_id = s.session_id
            LEFT JOIN process_samples ps_all
              ON ps_all.session_id = s.session_id
            WHERE ps_match.name LIKE ?1
            GROUP BY s.session_id
            ORDER BY s.first_seen_ms DESC
            LIMIT ?2
            ",
        )?;

        let limit_i64 = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt.query_map(params![pattern, limit_i64], |row| {
            Ok(StoredSessionInfo {
                session_id: row.get(0)?,
                first_seen_ms: row.get(1)?,
                last_seen_ms: row.get(2)?,
            })
        })?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        Ok(sessions)
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
        AllocationSnapshot, PersistedLeakedBlock, PersistedNetworkSample, PersistedProcessSample,
        PersistedReachabilityScan, PersistedSampleBatch, ProcessFingerprint, ProcessIdentity,
        SqliteSink, StorageSink,
    };

    fn sample_batch(session_id: Uuid, pid: u32) -> PersistedSampleBatch {
        let now = SystemTime::now();
        PersistedSampleBatch {
            collected_at: now,
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
                is_anomalous: Some(false),
                collected_at: now,
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
                collected_at: now,
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
    fn allocation_snapshot_round_trip_by_name() {
        let mut sink = SqliteSink::new_in_memory().expect("sink should initialize");
        let row = AllocationSnapshot {
            pid: 999,
            name: "leaker".to_string(),
            collected_at_ms: 1_700_000_000_000,
            snapshot_count: 12,
            total_outstanding_bytes: 8_192_000,
        };
        sink.persist_allocation_snapshot(&row)
            .expect("persist should succeed");

        let read_back = sink
            .query_allocation_snapshots_by_name(
                "leaker",
                1_700_000_000_000 - 1,
                1_700_000_000_000 + 1,
            )
            .expect("query should succeed");

        assert_eq!(read_back.len(), 1);
        let r = &read_back[0];
        assert_eq!(r.pid, 999);
        assert_eq!(r.name, "leaker");
        assert_eq!(r.collected_at_ms, 1_700_000_000_000);
        assert_eq!(r.snapshot_count, 12);
        assert_eq!(r.total_outstanding_bytes, 8_192_000);
    }

    #[test]
    fn reachability_scans_query_by_name_returns_matching_rows() {
        let mut sink = SqliteSink::new_in_memory().expect("sink should initialize");
        let scan = PersistedReachabilityScan {
            pid: 4242,
            name: "leaker".to_string(),
            started_at_ms: 1_700_000_000_000,
            duration_s: 30,
            allocator: "glibc".to_string(),
            total_blocks: 2,
            total_bytes: 4096,
            definitely_lost_bytes: 2048,
            possibly_lost_bytes: 0,
            indirectly_lost_bytes: 0,
            still_reachable_bytes: 2048,
            top_blocks: vec![
                PersistedLeakedBlock {
                    addr: 0xdead,
                    size: 1024,
                    class: "definitely".to_string(),
                    stack_text: Some("main.c:42".to_string()),
                },
                PersistedLeakedBlock {
                    addr: 0xbeef,
                    size: 1024,
                    class: "definitely".to_string(),
                    stack_text: None,
                },
            ],
        };
        sink.persist_reachability_scan(&scan)
            .expect("persist should succeed");

        let scans = sink
            .query_reachability_scans_by_name(
                "leaker",
                1_700_000_000_000 - 1,
                1_700_000_000_000 + 1,
            )
            .expect("query should succeed");
        assert_eq!(scans.len(), 1);
        assert_eq!(scans[0].pid, 4242);
        assert_eq!(scans[0].definitely_lost_bytes, 2048);

        let blocks = sink
            .query_leaked_blocks_for_scan(scans[0].id)
            .expect("blocks query should succeed");
        assert_eq!(blocks.len(), 2);
        // equal sizes, so id-asc breaks the tie.
        assert_eq!(blocks[0].addr, 0xdead);
        assert_eq!(blocks[0].stack_text.as_deref(), Some("main.c:42"));
        assert_eq!(blocks[1].addr, 0xbeef);
        assert_eq!(blocks[1].stack_text, None);
    }

    #[test]
    fn is_anomalous_migration_is_idempotent() {
        // running init twice must be a no-op the second time
        let mut sink = SqliteSink::new_in_memory().expect("sink should initialize");
        sink.init().expect("re-init should succeed");
        sink.init().expect("third init should also succeed");

        // column exists
        let has: i64 = sink
            .conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('process_samples') WHERE name = 'is_anomalous'",
                [],
                |row| row.get(0),
            )
            .expect("pragma query should succeed");
        assert_eq!(has, 1);
    }

    #[test]
    fn memory_history_round_trip_includes_cpu_and_anomaly() {
        let mut sink = SqliteSink::new_in_memory().expect("sink should initialize");
        let mut batch = sample_batch(Uuid::new_v4(), 7777);
        batch.processes[0].is_anomalous = Some(true);
        batch.processes[0].cpu_top = 88.5;
        sink.persist_sample_batch(&batch)
            .expect("persist should succeed");

        let rows = sink
            .query_process_memory_history_by_name("app", i64::MIN, i64::MAX)
            .expect("query should succeed");
        assert_eq!(rows.len(), 1);
        assert!((rows[0].cpu_top - 88.5).abs() < 1e-9);
        assert!(rows[0].is_anomalous);
    }

    #[test]
    fn query_sessions_list_for_name_filters_to_matching_processes() {
        let mut sink = SqliteSink::new_in_memory().expect("sink should initialize");

        // session a runs "leaker", session b runs only "other".
        let session_a = Uuid::new_v4();
        let mut batch_a = sample_batch(session_a, 100);
        batch_a.processes[0].name = "leaker".to_string();
        sink.persist_sample_batch(&batch_a)
            .expect("persist A should succeed");

        let session_b = Uuid::new_v4();
        let mut batch_b = sample_batch(session_b, 200);
        batch_b.processes[0].name = "other".to_string();
        sink.persist_sample_batch(&batch_b)
            .expect("persist B should succeed");

        let filtered = sink
            .query_sessions_list_for_name("leaker", 20)
            .expect("filtered query should succeed");
        assert_eq!(filtered.len(), 1, "only session A matches 'leaker'");
        assert_eq!(filtered[0].session_id, session_a.to_string());

        let unmatched = sink
            .query_sessions_list_for_name("nonexistent", 20)
            .expect("query should succeed");
        assert!(unmatched.is_empty(), "no session matches a missing name");

        // unfiltered query still returns both sessions.
        let all = sink
            .query_sessions_list(20)
            .expect("unfiltered query should succeed");
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn persist_sample_batch_writes_distinct_row_timestamps() {
        let mut sink = SqliteSink::new_in_memory().expect("sink should initialize");
        let session_id = Uuid::new_v4();
        let t1 = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_700_000_000_000);
        let t2 = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_700_000_002_000);
        let t3 = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_700_000_004_000);

        // accumulator buffering three ticks: one flush time, three distinct
        // per-sample timestamps
        let mut batch = sample_batch(session_id, 123);
        batch.collected_at = t3;
        batch.processes[0].collected_at = t1;
        batch.processes.push(PersistedProcessSample {
            collected_at: t2,
            ..batch.processes[0].clone()
        });
        batch.processes.push(PersistedProcessSample {
            collected_at: t3,
            ..batch.processes[0].clone()
        });
        sink.persist_sample_batch(&batch)
            .expect("persist should succeed");

        let timestamps: Vec<i64> = {
            let mut stmt = sink
                .conn
                .prepare("SELECT collected_at_ms FROM process_samples ORDER BY id ASC")
                .expect("prepare");
            stmt.query_map([], |row| row.get(0))
                .expect("query")
                .map(|r| r.expect("row"))
                .collect()
        };
        assert_eq!(
            timestamps,
            vec![1_700_000_000_000, 1_700_000_002_000, 1_700_000_004_000],
            "each per-tick sample should land with its own collected_at_ms"
        );
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
