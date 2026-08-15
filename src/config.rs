use dotenvy;
use std::net::{Ipv4Addr, SocketAddr};

pub struct DeviceConfig {
    pub node: u32,
    pub bind_socket: SocketAddr,
    pub peer_socket: SocketAddr,
    pub tunnel_ip: Ipv4Addr,
    pub private_key: [u8; 32],
    pub peer_public_key: [u8; 32],
}

impl DeviceConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let node = std::env::var("NODE")
            .expect("NODE missing")
            .parse::<u32>()
            .expect("NODE must be integer");

        let bind_socket = std::env::var("PUBLIC_SOCKET")
            .expect("PUBLIC_IP missing")
            .parse::<SocketAddr>()
            .expect("PUBLIC_IP must be socket addr");

        let peer_socket = std::env::var("PEER_SOCKET")
            .expect("PEER_SOCKET missing")
            .parse::<SocketAddr>()
            .expect("PEER_SOCKET must be socket addr");

        let private: Vec<u8> =
            serde_json::from_str(&std::env::var("PRIVATE_KEY").expect("PRIVATE_KEY missing"))
                .expect("invalid PRIVATE_KEY");

        let private_key: [u8; 32] = private
            .try_into()
            .expect("PRIVATE_KEY must contain 32 bytes");

        let peer_public: Vec<u8> = serde_json::from_str(
            &std::env::var("PEER_PUBLIC_KEY").expect("PEER_PUBLIC_KEY missing"),
        )
        .expect("invalid PEER_PUBLIC_KEY");

        let peer_public_key: [u8; 32] = peer_public
            .try_into()
            .expect("PEER_PUBLIC_KEY must contain 32 bytes");

        let tunnel_ip = if node == 0 {
            Ipv4Addr::new(10, 0, 0, 9)
        } else {
            Ipv4Addr::new(10, 0, 0, 10)
        };

        Self {
            node,
            bind_socket,
            peer_socket,
            tunnel_ip,
            private_key,
            peer_public_key,
        }
    }
}
