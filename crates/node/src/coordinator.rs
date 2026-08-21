use borealis_protocol::{
    HeartbeatRequest, HeartbeatResponse, PeerInfo, PeerMapResponse, RegisterNodeRequest,
    RegisterNodeResponse,
};
use reqwest::StatusCode;
use reqwest::blocking::{Client, Response};
use std::thread;
use std::time::Duration;

const PEER_POLL_INTERVAL: Duration = Duration::from_secs(2);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct CoordinatorClient {
    base_url: String,
    client: Client,
}

impl CoordinatorClient {
    pub fn new(base_url: String) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|error| format!("failed to build coordinator client: {error}"))?;

        Ok(Self { base_url, client })
    }

    pub fn register(
        &self,
        public_key: [u8; 32],
        listen_port: u16,
    ) -> Result<RegisterNodeResponse, String> {
        let response = self
            .client
            .post(format!("{}/v1/nodes/register", self.base_url))
            .json(&RegisterNodeRequest {
                public_key,
                listen_port,
            })
            .send()
            .map_err(|error| format!("failed to contact coordinator: {error}"))?;

        parse_json(response, "node registration")
    }

    pub fn wait_for_peer(&self, node_id: i64) -> Result<PeerInfo, String> {
        loop {
            let response = self
                .client
                .get(format!("{}/v1/nodes/{node_id}/peers", self.base_url))
                .send()
                .map_err(|error| format!("failed to fetch peer map: {error}"))?;
            let mut peer_map: PeerMapResponse = parse_json(response, "peer discovery")?;

            match peer_map.peers.len() {
                0 => {
                    tracing::info!("waiting for a peer to join the network");
                    thread::sleep(PEER_POLL_INTERVAL);
                }
                1 => return Ok(peer_map.peers.remove(0)),
                count => {
                    return Err(format!(
                        "coordinator returned {count} peers, but this node currently supports exactly one"
                    ));
                }
            }
        }
    }

    pub fn spawn_heartbeat(self, node_id: i64, listen_port: u16) -> Result<(), String> {
        thread::Builder::new()
            .name("coordinator-heartbeat".into())
            .spawn(move || {
                loop {
                    thread::sleep(HEARTBEAT_INTERVAL);

                    if let Err(error) = self.heartbeat(node_id, listen_port) {
                        tracing::error!(%error, "coordinator heartbeat failed");
                    }
                }
            })
            .map(|_| ())
            .map_err(|error| format!("failed to start coordinator heartbeat worker: {error}"))
    }

    fn heartbeat(&self, node_id: i64, listen_port: u16) -> Result<(), String> {
        let response = self
            .client
            .post(format!("{}/v1/nodes/{node_id}/heartbeat", self.base_url))
            .json(&HeartbeatRequest { listen_port })
            .send()
            .map_err(|error| format!("failed to contact coordinator: {error}"))?;
        let heartbeat: HeartbeatResponse = parse_json(response, "heartbeat")?;

        tracing::debug!(lease_expires_at = %heartbeat.lease_expires_at, "renewed coordinator lease");
        Ok(())
    }
}

fn parse_json<T: serde::de::DeserializeOwned>(
    response: Response,
    operation: &str,
) -> Result<T, String> {
    let status = response.status();
    if status != StatusCode::OK {
        let body = response.text().unwrap_or_default();
        return Err(format!(
            "coordinator {operation} failed with {status}: {body}"
        ));
    }

    response
        .json()
        .map_err(|error| format!("coordinator returned an invalid {operation} response: {error}"))
}
