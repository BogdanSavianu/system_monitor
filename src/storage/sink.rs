use super::types::{PersistedSampleBatch, StorageResult};

pub trait StorageSink {
    fn init(&mut self) -> StorageResult<()> {
        Ok(())
    }

    fn persist_sample_batch(&mut self, batch: &PersistedSampleBatch) -> StorageResult<()>;

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
