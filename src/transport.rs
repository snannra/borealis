use std::net::{SocketAddr, UdpSocket};

pub fn bind_udp(addr: SocketAddr) -> UdpSocket {
    UdpSocket::bind(addr).expect("failed to bind UDP transport")
}
