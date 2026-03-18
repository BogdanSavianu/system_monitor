use std::collections::HashMap;

use crate::{
    hashmap,
    model::{ProcessNetworkStatsModel, SocketInfoModel},
    util::{Inode, Pid},
};

#[derive(Debug, Clone)]
pub struct NetworkSnapshotModel {
    pub sockets_by_inode: HashMap<Inode, SocketInfoModel>,
    pub process_stats_by_pid: HashMap<Pid, ProcessNetworkStatsModel>,
}

impl NetworkSnapshotModel {
    pub fn new() -> Self {
        NetworkSnapshotModel {
            sockets_by_inode: hashmap![],
            process_stats_by_pid: hashmap![],
        }
    }

    pub fn with_values(
        sockets_by_inode: HashMap<Inode, SocketInfoModel>,
        process_stats_by_pid: HashMap<Pid, ProcessNetworkStatsModel>,
    ) -> Self {
        NetworkSnapshotModel {
            sockets_by_inode,
            process_stats_by_pid,
        }
    }
}
