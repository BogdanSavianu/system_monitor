use crate::{
    model::NetworkProtocolEnum,
    util::{Pid, Port},
};

#[derive(Debug, Clone)]
pub struct ProcessEndpointModel {
    pub pid: Pid,
    pub protocol: NetworkProtocolEnum,
    pub local_addr: std::net::IpAddr,
    pub local_port: Port,
    pub remote_addr: std::net::IpAddr,
    pub remote_port: Port,
}

impl ProcessEndpointModel {
    pub fn with_protocol_address(
        protocol: NetworkProtocolEnum,
        local_addr: std::net::IpAddr,
        remote_addr: std::net::IpAddr,
    ) -> Self {
        ProcessEndpointModel {
            pid: 0,
            protocol,
            local_addr,
            local_port: 0,
            remote_addr,
            remote_port: 0,
        }
    }

    pub fn add_ports(&mut self, local_port: Port, remote_port: Port) -> &mut Self {
        self.add_local_port(local_port).add_remote_port(remote_port)
    }

    pub fn add_local_port(&mut self, local_port: Port) -> &mut Self {
        self.local_port = local_port;

        self
    }

    pub fn add_remote_port(&mut self, remote_port: Port) -> &mut Self {
        self.remote_port = remote_port;

        self
    }
}
