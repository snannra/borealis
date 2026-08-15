use std::net::SocketAddr;
use std::sync::Mutex;

pub struct PeerEndpoint {
    addr: Mutex<Option<SocketAddr>>,
}

impl PeerEndpoint {
    pub fn new(addr: Option<SocketAddr>) -> Self {
        Self {
            addr: Mutex::new(addr),
        }
    }

    pub fn get(&self) -> Option<SocketAddr> {
        *self.addr.lock().unwrap()
    }

    pub fn update(&self, addr: SocketAddr) {
        *self.addr.lock().unwrap() = Some(addr);
    }
}
