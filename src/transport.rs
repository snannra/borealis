use std::net::{SocketAddr, UdpSocket};

pub fn bind_udp(addr: SocketAddr) -> UdpSocket {
    let socket = UdpSocket::bind(addr).expect("failed to bind UDP transport");
    let local_addr = socket
        .local_addr()
        .expect("failed to read bound UDP transport address");

    tracing::info!(address = %local_addr, "bound UDP transport");

    socket
}
