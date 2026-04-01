use system_monitor::{dto::ProcessNetworkSampleDTO, util::Pid};

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkRowViewModel {
    pub pid: Pid,
    pub name: String,
    pub tcp_open: u32,
    pub tcp_established: u32,
    pub tcp_listen: u32,
    pub udp_open: u32,
    pub total_sockets: u32,
}

pub fn network_rows_from_dtos(samples: &[ProcessNetworkSampleDTO]) -> Vec<NetworkRowViewModel> {
    samples
        .iter()
        .map(|sample| NetworkRowViewModel {
            pid: sample.pid,
            name: sample.name.clone(),
            tcp_open: sample.tcp_open,
            tcp_established: sample.tcp_established,
            tcp_listen: sample.tcp_listen,
            udp_open: sample.udp_open,
            total_sockets: sample.total_sockets,
        })
        .collect()
}
