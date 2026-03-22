use crate::{
    model::{NetworkProtocolEnum, SocketInfoModel, TcpStateEnum},
    util::{Inode, Pid},
};

#[derive(Debug, Clone)]
pub struct ProcessNetworkStatsModel {
    pub pid: Pid,
    pub tcp_open: u32,
    pub tcp_established: u32,
    pub tcp_listen: u32,
    pub udp_open: u32,
    pub socket_inodes: Vec<Inode>,
}

impl ProcessNetworkStatsModel {
    pub fn with_pid(pid: Pid) -> Self {
        ProcessNetworkStatsModel {
            pid,
            tcp_open: 0,
            tcp_established: 0,
            tcp_listen: 0,
            udp_open: 0,
            socket_inodes: vec![],
        }
    }

    pub fn accumulate_socket(&mut self, inode: Inode, socket: &SocketInfoModel) {
        self.socket_inodes.push(inode);

        match socket.protocol {
            NetworkProtocolEnum::Tcp | NetworkProtocolEnum::Tcp6 => {
                self.tcp_open += 1;

                if let Some(TcpStateEnum::Established) = socket.tcp_state {
                    self.tcp_established += 1;
                }

                if let Some(TcpStateEnum::Listen) = socket.tcp_state {
                    self.tcp_listen += 1;
                }
            }

            NetworkProtocolEnum::Udp | NetworkProtocolEnum::Udp6 => {
                self.udp_open += 1;
            }
        }
    }
}
