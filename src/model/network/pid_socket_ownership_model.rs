use crate::util::{Inode, Pid};

#[derive(Debug, Clone)]
pub struct PidSocketOwnershipModel {
    pub pid: Pid,
    pub socket_inodes: Vec<Inode>,
}

impl PidSocketOwnershipModel {
    pub fn new() -> Self {
        PidSocketOwnershipModel {
            pid: 0,
            socket_inodes: vec![],
        }
    }

    pub fn with_values(pid: Pid, socket_inodes: Vec<Inode>) -> Self {
        PidSocketOwnershipModel {
            pid,
            socket_inodes,
        }
    }
}
