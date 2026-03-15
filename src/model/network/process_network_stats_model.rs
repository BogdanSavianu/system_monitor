use crate::util::{Inode, Pid};

#[derive(Debug, Clone)]
pub struct ProcessNetworkStatsModel {
    pub pid: Pid,
    pub tcp_open: u32,
    pub tcp_established: u32,
    pub tcp_listen: u32,
    pub udp_open: u32,
    pub socket_inodes: Vec<Inode>,
}
