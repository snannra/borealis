//! Shared control-plane types for communication between Borealis nodes and the
//! coordination service.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::{Ipv4Addr, SocketAddr};

#[derive(Debug, Deserialize, Serialize)]
pub struct PeerInfo {
    pub node_id: i64,
    pub public_key: [u8; 32],
    pub overlay_ip: Ipv4Addr,
    pub endpoint: SocketAddr,
    pub lease_expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PeerMapResponse {
    pub peers: Vec<PeerInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HeartbeatRequest {
    pub listen_port: u16,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HeartbeatResponse {
    pub lease_expires_at: DateTime<Utc>,
}
