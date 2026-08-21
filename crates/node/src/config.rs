use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;

pub struct NodeConfig {
    pub coordinator_url: String,
    pub key_path: PathBuf,
    pub bind_socket: SocketAddr,
}

impl NodeConfig {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let coordinator_url = std::env::var("COORDINATOR_URL")
            .expect("COORDINATOR_URL missing")
            .trim_end_matches('/')
            .to_owned();

        if coordinator_url.is_empty() {
            panic!("COORDINATOR_URL must not be empty");
        }

        let key_path = std::env::var_os("BOREALIS_KEY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(".borealis.key"));

        let bind_socket = std::env::var("BIND_SOCKET")
            .map(|value| {
                value
                    .parse::<SocketAddr>()
                    .expect("BIND_SOCKET must be a socket address")
            })
            .unwrap_or_else(|_| SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)));

        Self {
            coordinator_url,
            key_path,
            bind_socket,
        }
    }
}
