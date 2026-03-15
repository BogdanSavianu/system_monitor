#[derive(Debug, Clone)]
pub enum NetworkProtocolEnum {
    Tcp,
    Tcp6,
    Udp,
    Udp6,
}

#[derive(Debug, Clone)]
pub enum TcpStateEnum {
    Listen,
    SynSent,
    SynRecv,
    Established,
    FinWait1,
    FinWait2,
    TimeWait,
    Close,
    CloseWait,
    LastAck,
    Closing,
}
