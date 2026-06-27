use std::collections::HashMap;

use crate::util::Pid;

use super::types::{
    AllocationSnapshot, AllocationSnapshotRow, PersistedAllocScan, PersistedReachabilityScan,
    PersistedSampleBatch, StorageError, StorageResult, StoredLeakedBlock, StoredProcessMemoryPoint,
    StoredReachabilityScan, StoredSessionInfo,
};

pub trait StorageSink {
    fn init(&mut self) -> StorageResult<()> {
        Ok(())
    }

    fn persist_sample_batch(&mut self, batch: &PersistedSampleBatch) -> StorageResult<()>;

    fn query_process_memory_history(
        &self,
        _pid: Pid,
        _start_ms: i64,
        _end_ms: i64,
    ) -> StorageResult<Vec<StoredProcessMemoryPoint>> {
        Err(StorageError::InvalidData(
            "history query is not supported by this sink".to_string(),
        ))
    }

    fn query_process_memory_history_by_name(
        &self,
        _name: &str,
        _start_ms: i64,
        _end_ms: i64,
    ) -> StorageResult<Vec<StoredProcessMemoryPoint>> {
        Err(StorageError::InvalidData(
            "history-by-name query is not supported by this sink".to_string(),
        ))
    }

    fn query_matching_process_names(
        &self,
        _prefix: &str,
        _limit: usize,
    ) -> StorageResult<Vec<String>> {
        Err(StorageError::InvalidData(
            "name search is not supported by this sink".to_string(),
        ))
    }

    fn query_sessions_list(&self, _limit: usize) -> StorageResult<Vec<StoredSessionInfo>> {
        Err(StorageError::InvalidData(
            "sessions list query is not supported by this sink".to_string(),
        ))
    }

    /// like query_sessions_list but restricted to sessions where a process
    /// matching the name pattern actually ran.
    fn query_sessions_list_for_name(
        &self,
        _name: &str,
        _limit: usize,
    ) -> StorageResult<Vec<StoredSessionInfo>> {
        Err(StorageError::InvalidData(
            "sessions-by-name query is not supported by this sink".to_string(),
        ))
    }

    fn query_memory_baselines(&self) -> StorageResult<HashMap<String, f64>> {
        Err(StorageError::InvalidData(
            "memory baselines query is not supported by this sink".to_string(),
        ))
    }

    fn persist_alloc_scan(&mut self, _scan: &PersistedAllocScan) -> StorageResult<()> {
        Err(StorageError::InvalidData(
            "alloc scan persistence is not supported by this sink".to_string(),
        ))
    }

    fn persist_reachability_scan(
        &mut self,
        _scan: &PersistedReachabilityScan,
    ) -> StorageResult<()> {
        Err(StorageError::InvalidData(
            "reachability scan persistence is not supported by this sink".to_string(),
        ))
    }

    fn query_reachability_scans_by_name(
        &self,
        _name: &str,
        _start_ms: i64,
        _end_ms: i64,
    ) -> StorageResult<Vec<StoredReachabilityScan>> {
        Err(StorageError::InvalidData(
            "reachability scan query is not supported by this sink".to_string(),
        ))
    }

    fn query_leaked_blocks_for_scan(&self, _scan_id: i64) -> StorageResult<Vec<StoredLeakedBlock>> {
        Err(StorageError::InvalidData(
            "leaked block query is not supported by this sink".to_string(),
        ))
    }

    fn persist_allocation_snapshot(&mut self, _row: &AllocationSnapshot) -> StorageResult<()> {
        Err(StorageError::InvalidData(
            "allocation snapshot persistence is not supported by this sink".to_string(),
        ))
    }

    fn query_allocation_snapshots_by_name(
        &self,
        _name: &str,
        _start_ms: i64,
        _end_ms: i64,
    ) -> StorageResult<Vec<AllocationSnapshotRow>> {
        Err(StorageError::InvalidData(
            "allocation snapshot query is not supported by this sink".to_string(),
        ))
    }

    fn truncate_history(&mut self) -> StorageResult<()> {
        Err(StorageError::InvalidData(
            "history truncation is not supported by this sink".to_string(),
        ))
    }

    fn flush(&mut self) -> StorageResult<()> {
        Ok(())
    }

    fn close(&mut self) -> StorageResult<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct NoopSink;

impl StorageSink for NoopSink {
    fn persist_sample_batch(&mut self, _batch: &PersistedSampleBatch) -> StorageResult<()> {
        Ok(())
    }
}
