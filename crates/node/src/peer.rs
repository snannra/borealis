use std::net::SocketAddr;
use std::sync::Mutex;

pub struct PeerEndpoint {
    addr: Mutex<Option<SocketAddr>>,
}

impl PeerEndpoint {
    pub fn new(addr: Option<SocketAddr>) -> Self {
        match addr {
            Some(endpoint) => tracing::info!(%endpoint, "configured initial peer endpoint"),
            None => tracing::info!("peer endpoint will be learned from authenticated traffic"),
        }

        Self {
            addr: Mutex::new(addr),
        }
    }

    pub fn get(&self) -> Option<SocketAddr> {
        *self.addr.lock().unwrap()
    }

    pub fn update(&self, addr: SocketAddr) {
        let previous = {
            let mut endpoint = self.addr.lock().unwrap();
            let previous = *endpoint;
            *endpoint = Some(addr);
            previous
        };

        match previous {
            None => tracing::info!(endpoint = %addr, "learned authenticated peer endpoint"),
            Some(previous) if previous != addr => {
                tracing::info!(%previous, endpoint = %addr, "peer endpoint changed")
            }
            Some(_) => tracing::trace!(endpoint = %addr, "peer endpoint unchanged"),
        }
    }
}
