use crate::{model::{NetworkProtocolEnum, TcpStateEnum}, util::{Inode, Port}};

#[derive(Debug, Clone)]
pub struct SocketInfoModel {
    pub inode: Inode,
    pub protocol: NetworkProtocolEnum,
    pub local_addr: std::net::IpAddr,
    pub local_port: Port,
    pub remote_addr: std::net::IpAddr,
    pub remote_port: Port,
    // option because UDP won't have one
    pub tcp_state: Option<TcpStateEnum>,
}

impl SocketInfoModel {
    pub fn with_udp(
        protocol: NetworkProtocolEnum,
        local_addr: std::net::IpAddr,
        remote_addr: std::net::IpAddr,
    ) -> Self {
        SocketInfoModel {
            inode: 0,
            protocol,
            local_addr,
            local_port: 0,
            remote_addr,
            remote_port: 0,
            tcp_state: None,
        }
    }

    pub fn with_tcp(
        protocol: NetworkProtocolEnum,
        local_addr: std::net::IpAddr,
        remote_addr: std::net::IpAddr,
        tcp_state: TcpStateEnum,
    ) -> Self {
        SocketInfoModel {
            inode: 0,
            protocol,
            local_addr,
            local_port: 0,
            remote_addr,
            remote_port: 0,
            tcp_state: Some(tcp_state),
        }
    }

    pub fn add_inode(&mut self, inode: Inode) -> &mut Self {
        self.inode = inode;

        self
    }

    pub fn add_ports(&mut self, local_port: Port, remote_port: Port) -> &mut Self {
        self.add_local_port(local_port)
            .add_remote_port(remote_port)
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
