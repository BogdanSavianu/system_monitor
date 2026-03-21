use std::collections::HashSet;
use std::fs::{File, read_dir, read_link};
use std::io::{BufRead, BufReader};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::{
    model::{NetworkProtocolEnum, PidSocketOwnershipModel, SocketInfoModel, TcpStateEnum},
    util::{ParseError, Pid},
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
        all.extend(self.get_net_tcp_info()?);
        all.extend(self.get_net_tcp6_info()?);
        all.extend(self.get_net_udp_info()?);
        all.extend(self.get_net_udp6_info()?);
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
        let mut ownership = Vec::new();

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

            // in case process exits during scan, skip it
            if let Ok(pid_ownership) = self.get_pid_socket_ownership(pid) {
                ownership.push(pid_ownership);
            }
        }

        Ok(ownership)
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
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use crate::model::{NetworkProtocolEnum, TcpStateEnum};

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
}
