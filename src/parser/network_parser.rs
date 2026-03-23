use std::collections::{HashMap, HashSet};
use std::fs::{File, read_dir, read_link};
use std::io::{BufRead, BufReader};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::thread;

use crate::hashmap;
use crate::model::{NetworkSnapshotModel, ProcessNetworkStatsModel};
use crate::util::Inode;
use crate::{
    model::{NetworkProtocolEnum, PidSocketOwnershipModel, SocketInfoModel, TcpStateEnum},
    util::{ParseError, Pid, worker_count},
};

const NET_TCP_PATH: &str = "/proc/net/tcp";
const NET_TCP6_PATH: &str = "/proc/net/tcp6";
const NET_UDP_PATH: &str = "/proc/net/udp";
const NET_UDP6_PATH: &str = "/proc/net/udp6";

pub trait TraitNetworkParser {
    fn get_net_tcp_info(&self) -> Result<Vec<SocketInfoModel>, ParseError>;
    fn get_net_tcp6_info(&self) -> Result<Vec<SocketInfoModel>, ParseError>;
    fn get_net_udp_info(&self) -> Result<Vec<SocketInfoModel>, ParseError>;
    fn get_net_udp6_info(&self) -> Result<Vec<SocketInfoModel>, ParseError>;
    fn get_all_net_socket_info(&self) -> Result<Vec<SocketInfoModel>, ParseError>;
    fn get_pid_socket_ownership(&self, pid: Pid) -> Result<PidSocketOwnershipModel, ParseError>;
    fn get_all_pid_socket_ownership(&self) -> Result<Vec<PidSocketOwnershipModel>, ParseError>;
    fn get_process_network_stats(
        &self,
    ) -> Result<HashMap<Pid, ProcessNetworkStatsModel>, ParseError>;
    fn get_network_snapshot(&self) -> Result<NetworkSnapshotModel, ParseError>;
}

pub struct NetworkParser;

impl TraitNetworkParser for NetworkParser {
    fn get_net_tcp_info(&self) -> Result<Vec<SocketInfoModel>, ParseError> {
        self.parse_socket_table_file(NET_TCP_PATH, NetworkProtocolEnum::Tcp)
    }

    fn get_net_tcp6_info(&self) -> Result<Vec<SocketInfoModel>, ParseError> {
        self.parse_socket_table_file(NET_TCP6_PATH, NetworkProtocolEnum::Tcp6)
    }

    fn get_net_udp_info(&self) -> Result<Vec<SocketInfoModel>, ParseError> {
        self.parse_socket_table_file(NET_UDP_PATH, NetworkProtocolEnum::Udp)
    }

    fn get_net_udp6_info(&self) -> Result<Vec<SocketInfoModel>, ParseError> {
        self.parse_socket_table_file(NET_UDP6_PATH, NetworkProtocolEnum::Udp6)
    }

    fn get_all_net_socket_info(&self) -> Result<Vec<SocketInfoModel>, ParseError> {
        let mut all = Vec::new();

        thread::scope(|scope| -> Result<(), ParseError> {
            let tcp_handle = scope.spawn(|| self.get_net_tcp_info());
            let tcp6_handle = scope.spawn(|| self.get_net_tcp6_info());
            let udp_handle = scope.spawn(|| self.get_net_udp_info());
            let udp6_handle = scope.spawn(|| self.get_net_udp6_info());

            let tcp = tcp_handle
                .join()
                .map_err(|_| ParseError::ParsingError("tcp parser worker panicked".to_string()))??;
            let tcp6 = tcp6_handle
                .join()
                .map_err(|_| ParseError::ParsingError("tcp6 parser worker panicked".to_string()))??;
            let udp = udp_handle
                .join()
                .map_err(|_| ParseError::ParsingError("udp parser worker panicked".to_string()))??;
            let udp6 = udp6_handle
                .join()
                .map_err(|_| ParseError::ParsingError("udp6 parser worker panicked".to_string()))??;

            all.extend(tcp);
            all.extend(tcp6);
            all.extend(udp);
            all.extend(udp6);

            Ok(())
        })?;

        Ok(all)
    }

    fn get_pid_socket_ownership(&self, pid: Pid) -> Result<PidSocketOwnershipModel, ParseError> {
        let fd_dir = format!("/proc/{pid}/fd");
        let entries = read_dir(fd_dir).map_err(|err| ParseError::ParsingError(err.to_string()))?;

        let mut socket_inodes: HashSet<u64> = HashSet::new();

        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };

            let Ok(target) = read_link(entry.path()) else {
                continue;
            };

            let Some(target_text) = target.to_str() else {
                continue;
            };

            if let Some(inode) = self.parse_socket_inode_target(target_text) {
                socket_inodes.insert(inode);
            }
        }

        let mut socket_inodes: Vec<u64> = socket_inodes.into_iter().collect();
        socket_inodes.sort();

        Ok(PidSocketOwnershipModel::with_values(pid, socket_inodes))
    }

    fn get_all_pid_socket_ownership(&self) -> Result<Vec<PidSocketOwnershipModel>, ParseError> {
        let entries = read_dir("/proc").map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let mut pids: Vec<Pid> = Vec::new();

        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let Some(pid_text) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };

            let Ok(pid) = pid_text.parse::<u32>() else {
                continue;
            };

            pids.push(pid);
        }

        if pids.is_empty() {
            return Ok(vec![]);
        }

        let workers = worker_count(pids.len());
        let chunk_size = pids.len().div_ceil(workers);

        let mut ownership = Vec::new();
        thread::scope(|scope| {
            let mut handles = Vec::new();
            for chunk in pids.chunks(chunk_size) {
                handles.push(scope.spawn(move || {
                    let mut partial = Vec::new();
                    for pid in chunk {
                        // in case process exits during scan, skip it
                        if let Ok(pid_ownership) = self.get_pid_socket_ownership(*pid) {
                            partial.push(pid_ownership);
                        }
                    }

                    partial
                }));
            }

            for handle in handles {
                if let Ok(mut partial) = handle.join() {
                    ownership.append(&mut partial);
                }
            }
        });

        ownership.sort_by_key(|entry| entry.pid);

        Ok(ownership)
    }

    fn get_process_network_stats(
        &self,
    ) -> Result<HashMap<Pid, ProcessNetworkStatsModel>, ParseError> {
        let sockets_by_inode = self.get_sockets_by_inode()?;
        self.get_process_stats_for_sockets(&sockets_by_inode)
    }

    fn get_network_snapshot(&self) -> Result<NetworkSnapshotModel, ParseError> {
        let sockets_by_inode = self.get_sockets_by_inode()?;
        let process_stats_by_pid = self.get_process_stats_for_sockets(&sockets_by_inode)?;

        Ok(NetworkSnapshotModel::with_values(
            sockets_by_inode,
            process_stats_by_pid,
        ))
    }
}

impl NetworkParser {
    pub fn new() -> Self {
        NetworkParser
    }

    fn parse_socket_table_file(
        &self,
        path: &str,
        protocol: NetworkProtocolEnum,
    ) -> Result<Vec<SocketInfoModel>, ParseError> {
        let file = File::open(path).map_err(|err| ParseError::ParsingError(err.to_string()))?;
        let reader = BufReader::new(file);
        self.parse_socket_table(reader, protocol)
    }

    fn parse_socket_table<R>(
        &self,
        reader: R,
        protocol: NetworkProtocolEnum,
    ) -> Result<Vec<SocketInfoModel>, ParseError>
    where
        R: BufRead,
    {
        let mut sockets = Vec::new();

        for (line_index, line) in reader.lines().enumerate() {
            let line = line.map_err(|err| ParseError::ParsingError(err.to_string()))?;
            if line_index == 0 {
                continue;
            }

            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 10 {
                continue;
            }

            let (local_addr, local_port) = self.parse_endpoint(cols[1], &protocol)?;
            let (remote_addr, remote_port) = self.parse_endpoint(cols[2], &protocol)?;
            let inode = cols[9]
                .parse::<u64>()
                .map_err(|err| ParseError::ParsingError(err.to_string()))?;

            let mut socket = match protocol {
                NetworkProtocolEnum::Tcp | NetworkProtocolEnum::Tcp6 => {
                    let tcp_state = self.parse_tcp_state(cols[3])?;
                    SocketInfoModel::with_tcp(protocol.clone(), local_addr, remote_addr, tcp_state)
                }
                NetworkProtocolEnum::Udp | NetworkProtocolEnum::Udp6 => {
                    SocketInfoModel::with_udp(protocol.clone(), local_addr, remote_addr)
                }
            };

            socket.add_ports(local_port, remote_port).add_inode(inode);

            sockets.push(socket);
        }

        Ok(sockets)
    }

    fn parse_endpoint(
        &self,
        endpoint: &str,
        protocol: &NetworkProtocolEnum,
    ) -> Result<(IpAddr, u16), ParseError> {
        let (raw_ip, raw_port) = endpoint
            .split_once(':')
            .ok_or_else(|| ParseError::ParsingError(format!("invalid endpoint: {}", endpoint)))?;

        let port = u16::from_str_radix(raw_port, 16)
            .map_err(|err| ParseError::ParsingError(err.to_string()))?;

        let ip = match protocol {
            NetworkProtocolEnum::Tcp | NetworkProtocolEnum::Udp => {
                IpAddr::V4(self.parse_ipv4(raw_ip)?)
            }
            NetworkProtocolEnum::Tcp6 | NetworkProtocolEnum::Udp6 => {
                IpAddr::V6(self.parse_ipv6(raw_ip)?)
            }
        };

        Ok((ip, port))
    }

    fn parse_ipv4(&self, raw_ip: &str) -> Result<Ipv4Addr, ParseError> {
        if raw_ip.len() != 8 {
            return Err(ParseError::ParsingError(format!(
                "invalid ipv4 hex size: {}",
                raw_ip
            )));
        }

        let mut bytes = [0_u8; 4];
        for (i, slot) in bytes.iter_mut().enumerate() {
            let start = i * 2;
            let end = start + 2;
            *slot = u8::from_str_radix(&raw_ip[start..end], 16)
                .map_err(|err| ParseError::ParsingError(err.to_string()))?;
        }

        bytes.reverse();
        Ok(Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]))
    }

    fn parse_ipv6(&self, raw_ip: &str) -> Result<Ipv6Addr, ParseError> {
        if raw_ip.len() != 32 {
            return Err(ParseError::ParsingError(format!(
                "invalid ipv6 hex size: {}",
                raw_ip
            )));
        }

        let mut bytes = [0_u8; 16];
        for i in 0..16 {
            let start = i * 2;
            let end = start + 2;
            bytes[i] = u8::from_str_radix(&raw_ip[start..end], 16)
                .map_err(|err| ParseError::ParsingError(err.to_string()))?;
        }

        // /proc/net encodes IPv6 in little endian 32-bit words.
        for chunk in bytes.chunks_mut(4) {
            chunk.reverse();
        }

        Ok(Ipv6Addr::from(bytes))
    }

    fn parse_tcp_state(&self, raw_state: &str) -> Result<TcpStateEnum, ParseError> {
        match raw_state {
            "01" => Ok(TcpStateEnum::Established),
            "02" => Ok(TcpStateEnum::SynSent),
            "03" => Ok(TcpStateEnum::SynRecv),
            "04" => Ok(TcpStateEnum::FinWait1),
            "05" => Ok(TcpStateEnum::FinWait2),
            "06" => Ok(TcpStateEnum::TimeWait),
            "07" => Ok(TcpStateEnum::Close),
            "08" => Ok(TcpStateEnum::CloseWait),
            "09" => Ok(TcpStateEnum::LastAck),
            "0A" => Ok(TcpStateEnum::Listen),
            "0B" => Ok(TcpStateEnum::Closing),
            _ => Err(ParseError::ParsingError(format!(
                "unknown tcp state code: {}",
                raw_state
            ))),
        }
    }

    fn parse_socket_inode_target(&self, target: &str) -> Option<u64> {
        let stripped = target.strip_prefix("socket:[")?.strip_suffix(']')?;
        stripped.parse::<u64>().ok()
    }

    fn build_sockets_by_inode(
        &self,
        sockets: Vec<SocketInfoModel>,
    ) -> HashMap<Inode, SocketInfoModel> {
        let mut sockets_by_inode: HashMap<Inode, SocketInfoModel> = hashmap![];
        for socket in sockets {
            sockets_by_inode.insert(socket.inode, socket);
        }

        sockets_by_inode
    }

    fn build_process_stats_by_pid(
        &self,
        ownership: Vec<PidSocketOwnershipModel>,
        sockets_by_inode: &HashMap<Inode, SocketInfoModel>,
    ) -> HashMap<Pid, ProcessNetworkStatsModel> {
        if ownership.is_empty() {
            return hashmap![];
        }

        let workers = worker_count(ownership.len());
        let chunk_size = ownership.len().div_ceil(workers);

        let mut process_stats_by_pid: HashMap<Pid, ProcessNetworkStatsModel> = hashmap![];
        thread::scope(|scope| {
            let mut handles = Vec::new();
            for chunk in ownership.chunks(chunk_size) {
                handles.push(scope.spawn(move || {
                    let mut partial: HashMap<Pid, ProcessNetworkStatsModel> = hashmap![];
                    for pid_ownership in chunk {
                        let stats = self.aggregate_process_stats(pid_ownership, sockets_by_inode);
                        partial.insert(stats.pid, stats);
                    }

                    partial
                }));
            }

            for handle in handles {
                if let Ok(partial) = handle.join() {
                    process_stats_by_pid.extend(partial);
                }
            }
        });

        process_stats_by_pid
    }
    fn get_sockets_by_inode(&self) -> Result<HashMap<Inode, SocketInfoModel>, ParseError> {
        let sockets = self.get_all_net_socket_info()?;
        Ok(self.build_sockets_by_inode(sockets))
    }

    fn get_process_stats_for_sockets(
        &self,
        sockets_by_inode: &HashMap<Inode, SocketInfoModel>,
    ) -> Result<HashMap<Pid, ProcessNetworkStatsModel>, ParseError> {
        let ownership = self.get_all_pid_socket_ownership()?;
        Ok(self.build_process_stats_by_pid(ownership, sockets_by_inode))
    }

    fn aggregate_process_stats(
        &self,
        ownership: &PidSocketOwnershipModel,
        sockets_by_inode: &HashMap<Inode, SocketInfoModel>,
    ) -> ProcessNetworkStatsModel {
        let mut stats = ProcessNetworkStatsModel::with_pid(ownership.pid);
        for inode in &ownership.socket_inodes {
            if let Some(socket) = sockets_by_inode.get(inode) {
                stats.accumulate_socket(*inode, socket);
            }
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use crate::model::{
        NetworkProtocolEnum, PidSocketOwnershipModel, SocketInfoModel, TcpStateEnum,
    };

    use super::NetworkParser;

    #[test]
    fn parse_socket_table_parses_tcp_row() {
        let parser = NetworkParser::new();
        let input = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 0100007F:1F90 0200007F:0035 01 00000000:00000000 00:00000000 00000000  1000        0 55555\n";

        let sockets = parser
            .parse_socket_table(Cursor::new(input), NetworkProtocolEnum::Tcp)
            .expect("tcp table should have parsed");

        assert_eq!(sockets.len(), 1);
        let sock = &sockets[0];
        assert_eq!(sock.inode, 55555);
        assert_eq!(sock.local_port, 8080);
        assert_eq!(sock.remote_port, 53);
        assert_eq!(sock.local_addr, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!(sock.remote_addr, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)));
        assert!(matches!(sock.tcp_state, Some(TcpStateEnum::Established)));
    }

    #[test]
    fn parse_socket_table_parses_udp6_row_without_state() {
        let parser = NetworkParser::new();
        let input = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n   0: 00000000000000000000000000000000:14E9 00000000000000000000000000000000:0000 07 00000000:00000000 00:00000000 00000000  1000        0 77777\n";

        let sockets = parser
            .parse_socket_table(Cursor::new(input), NetworkProtocolEnum::Udp6)
            .expect("udp6 table should have parsed");

        assert_eq!(sockets.len(), 1);
        let sock = &sockets[0];
        assert_eq!(sock.inode, 77777);
        assert_eq!(sock.local_port, 5353);
        assert_eq!(sock.remote_port, 0);
        assert!(matches!(sock.protocol, NetworkProtocolEnum::Udp6));
        assert!(matches!(sock.local_addr, IpAddr::V6(Ipv6Addr::UNSPECIFIED)));
        assert!(sock.tcp_state.is_none());
    }

    #[test]
    fn parse_tcp_state_maps_known_values() {
        let parser = NetworkParser::new();
        let state = parser
            .parse_tcp_state("0A")
            .expect("tcp state should have parsed");
        assert!(matches!(state, TcpStateEnum::Listen));
    }

    #[test]
    fn parse_socket_inode_target_extracts_inode() {
        let parser = NetworkParser::new();
        let inode = parser.parse_socket_inode_target("socket:[123456]");
        assert_eq!(inode, Some(123456));
    }

    #[test]
    fn parse_socket_inode_target_rejects_non_socket_targets() {
        let parser = NetworkParser::new();
        assert_eq!(parser.parse_socket_inode_target("pipe:[123]"), None);
        assert_eq!(
            parser.parse_socket_inode_target("anon_inode:[eventfd]"),
            None
        );
        assert_eq!(parser.parse_socket_inode_target("socket:123"), None);
    }

    #[test]
    fn build_sockets_by_inode_indexes_sockets() {
        let parser = NetworkParser::new();

        let mut tcp = SocketInfoModel::with_tcp(
            NetworkProtocolEnum::Tcp,
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            TcpStateEnum::Listen,
        );
        tcp.add_inode(11).add_ports(8080, 0);

        let mut udp = SocketInfoModel::with_udp(
            NetworkProtocolEnum::Udp,
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
        );
        udp.add_inode(22).add_ports(53, 0);

        let sockets_by_inode = parser.build_sockets_by_inode(vec![tcp, udp]);

        assert_eq!(sockets_by_inode.len(), 2);
        assert!(sockets_by_inode.contains_key(&11));
        assert!(sockets_by_inode.contains_key(&22));
    }

    #[test]
    fn aggregate_process_stats_counts_socket_types_and_states() {
        let parser = NetworkParser::new();
        let ownership = PidSocketOwnershipModel::with_values(42, vec![100, 101, 102, 999]);

        let mut tcp_established = SocketInfoModel::with_tcp(
            NetworkProtocolEnum::Tcp,
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)),
            TcpStateEnum::Established,
        );
        tcp_established.add_inode(100).add_ports(8080, 443);

        let mut tcp_listen = SocketInfoModel::with_tcp(
            NetworkProtocolEnum::Tcp,
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            TcpStateEnum::Listen,
        );
        tcp_listen.add_inode(101).add_ports(22, 0);

        let mut udp = SocketInfoModel::with_udp(
            NetworkProtocolEnum::Udp,
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );
        udp.add_inode(102).add_ports(5353, 0);

        let mut sockets_by_inode = HashMap::new();
        sockets_by_inode.insert(100, tcp_established);
        sockets_by_inode.insert(101, tcp_listen);
        sockets_by_inode.insert(102, udp);

        let stats = parser.aggregate_process_stats(&ownership, &sockets_by_inode);

        assert_eq!(stats.pid, 42);
        assert_eq!(stats.tcp_open, 2);
        assert_eq!(stats.tcp_established, 1);
        assert_eq!(stats.tcp_listen, 1);
        assert_eq!(stats.udp_open, 1);
        assert_eq!(stats.socket_inodes, vec![100, 101, 102]);
    }

    #[test]
    fn build_process_stats_by_pid_groups_ownership_by_pid() {
        let parser = NetworkParser::new();

        let ownership = vec![
            PidSocketOwnershipModel::with_values(1000, vec![1, 2]),
            PidSocketOwnershipModel::with_values(2000, vec![3]),
        ];

        let mut socket1 = SocketInfoModel::with_tcp(
            NetworkProtocolEnum::Tcp,
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            TcpStateEnum::Established,
        );
        socket1.add_inode(1).add_ports(9000, 9001);

        let mut socket2 = SocketInfoModel::with_tcp(
            NetworkProtocolEnum::Tcp,
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            TcpStateEnum::Listen,
        );
        socket2.add_inode(2).add_ports(22, 0);

        let mut socket3 = SocketInfoModel::with_udp(
            NetworkProtocolEnum::Udp,
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );
        socket3.add_inode(3).add_ports(5353, 0);

        let mut sockets_by_inode = HashMap::new();
        sockets_by_inode.insert(1, socket1);
        sockets_by_inode.insert(2, socket2);
        sockets_by_inode.insert(3, socket3);

        let process_stats = parser.build_process_stats_by_pid(ownership, &sockets_by_inode);

        assert_eq!(process_stats.len(), 2);

        let pid_1000 = process_stats
            .get(&1000)
            .expect("pid 1000 should have process stats");
        assert_eq!(pid_1000.tcp_open, 2);
        assert_eq!(pid_1000.tcp_established, 1);
        assert_eq!(pid_1000.tcp_listen, 1);
        assert_eq!(pid_1000.udp_open, 0);

        let pid_2000 = process_stats
            .get(&2000)
            .expect("pid 2000 should have process stats");
        assert_eq!(pid_2000.tcp_open, 0);
        assert_eq!(pid_2000.udp_open, 1);
    }
}
