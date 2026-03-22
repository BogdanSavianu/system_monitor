use crate::model::ProcessNetworkStatsModel;
use crate::util::Pid;

#[derive(Debug, Clone)]
pub struct ProcessNetworkSampleDTO {
    pub pid: Pid,
    pub name: String,
    pub tcp_open: u32,
    pub tcp_established: u32,
    pub tcp_listen: u32,
    pub udp_open: u32,
    pub total_sockets: u32,
}

impl ProcessNetworkSampleDTO {
    pub fn with_values(
        pid: Pid,
        name: String,
        tcp_open: u32,
        tcp_established: u32,
        tcp_listen: u32,
        udp_open: u32,
        total_sockets: u32,
    ) -> Self {
        ProcessNetworkSampleDTO {
            pid,
            name,
            tcp_open,
            tcp_established,
            tcp_listen,
            udp_open,
            total_sockets,
        }
    }

    pub fn from_model(name: String, model: &ProcessNetworkStatsModel) -> Self {
        ProcessNetworkSampleDTO {
            pid: model.pid,
            name,
            tcp_open: model.tcp_open,
            tcp_established: model.tcp_established,
            tcp_listen: model.tcp_listen,
            udp_open: model.udp_open,
            total_sockets: model.socket_inodes.len() as u32,
        }
    }
}
