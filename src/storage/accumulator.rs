use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::{Duration, SystemTime},
};

use uuid::Uuid;

use crate::{
    dto::{ProcessCpuSampleDTO, ProcessNetworkSampleDTO},
    state::SystemState,
};

use super::{
    PersistedNetworkSample, PersistedProcessSample, PersistedSampleBatch, ProcessFingerprint,
    ProcessIdentity,
};

pub trait TraitSampleAccumulator {
    fn accumulate(
        &mut self,
        collected_at: SystemTime,
        process_samples: &[ProcessCpuSampleDTO],
        network_samples: &[ProcessNetworkSampleDTO],
        state: &SystemState,
    ) -> Option<PersistedSampleBatch>;

    fn drain_pending(&mut self, collected_at: SystemTime) -> Option<PersistedSampleBatch>;
}

#[derive(Default)]
struct PendingSampleData {
    processes: Vec<PersistedProcessSample>,
    network: Vec<PersistedNetworkSample>,
}

pub struct DefaultSampleAccumulator {
    session_id: Uuid,
    pending: PendingSampleData,
    flush_interval: Duration,
    last_flush_at: Option<SystemTime>,
}

impl DefaultSampleAccumulator {
    pub fn new(session_id: Uuid, flush_interval: Duration) -> Self {
        Self {
            session_id,
            pending: PendingSampleData::default(),
            flush_interval,
            last_flush_at: None,
        }
    }

    fn hash_cmdline(cmdline: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        cmdline.hash(&mut hasher);
        hasher.finish()
    }

    fn build_process_samples(
        process_samples: &[ProcessCpuSampleDTO],
        state: &SystemState,
    ) -> Vec<PersistedProcessSample> {
        process_samples
            .iter()
            .map(|sample| {
                let process = state.get_process(sample.pid);
                let cmdline = process.map(|p| p.cmdline.clone()).unwrap_or_default();
                let thread_count = process.map(|p| p.thread_count).unwrap_or(0);

                PersistedProcessSample {
                    identity: ProcessIdentity {
                        pid: sample.pid,
                        start_time_ticks: None,
                    },
                    fingerprint: ProcessFingerprint {
                        executable_path: None,
                        cmdline_hash: if cmdline.is_empty() {
                            None
                        } else {
                            Some(Self::hash_cmdline(&cmdline))
                        },
                    },
                    name: sample.name.clone(),
                    cmdline,
                    cpu_top: sample.cpu_top,
                    cpu_rel: sample.cpu_rel,
                    virtual_mem: sample.virtual_mem,
                    physical_mem: sample.physical_mem,
                    thread_count,
                }
            })
            .collect()
    }

    fn build_network_samples(
        network_samples: &[ProcessNetworkSampleDTO],
    ) -> Vec<PersistedNetworkSample> {
        network_samples
            .iter()
            .map(|sample| PersistedNetworkSample {
                identity: ProcessIdentity {
                    pid: sample.pid,
                    start_time_ticks: None,
                },
                tcp_open: sample.tcp_open,
                tcp_established: sample.tcp_established,
                tcp_listen: sample.tcp_listen,
                udp_open: sample.udp_open,
                total_sockets: sample.total_sockets,
            })
            .collect()
    }

    fn should_flush(&self, collected_at: SystemTime) -> bool {
        match self.last_flush_at {
            Some(last) => collected_at
                .duration_since(last)
                .map(|delta| delta >= self.flush_interval)
                .unwrap_or(false),
            None => false,
        }
    }
}

impl TraitSampleAccumulator for DefaultSampleAccumulator {
    fn accumulate(
        &mut self,
        collected_at: SystemTime,
        process_samples: &[ProcessCpuSampleDTO],
        network_samples: &[ProcessNetworkSampleDTO],
        state: &SystemState,
    ) -> Option<PersistedSampleBatch> {
        if self.last_flush_at.is_none() {
            self.last_flush_at = Some(collected_at);
        }

        self.pending
            .processes
            .extend(Self::build_process_samples(process_samples, state));
        self.pending
            .network
            .extend(Self::build_network_samples(network_samples));

        if self.should_flush(collected_at) {
            return self.drain_pending(collected_at);
        }

        None
    }

    fn drain_pending(&mut self, collected_at: SystemTime) -> Option<PersistedSampleBatch> {
        if self.pending.processes.is_empty() && self.pending.network.is_empty() {
            return None;
        }

        self.last_flush_at = Some(collected_at);
        Some(PersistedSampleBatch {
            collected_at,
            session_id: self.session_id,
            processes: std::mem::take(&mut self.pending.processes),
            network: std::mem::take(&mut self.pending.network),
        })
    }
}
