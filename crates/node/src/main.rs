mod config;
mod coordinator;
mod identity;
mod peer;
mod transport;
mod tun_device;
mod tunnel;

use boringtun::noise::Tunn;
use config::NodeConfig;
use coordinator::CoordinatorClient;
use identity::Identity;
use peer::PeerEndpoint;
use std::net::{Ipv4Addr, SocketAddr};
use tunnel::Tunnel;

fn main() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let config = NodeConfig::from_env();
    tracing::info!(
        coordinator = %config.coordinator_url,
        "loaded node configuration"
    );

    let identity = Identity::load_or_generate(&config.key_path)
        .unwrap_or_else(|error| panic!("failed to initialize node identity: {error}"));
    let socket = transport::bind_udp(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)));
    let listen_port = socket
        .local_addr()
        .expect("failed to read UDP listen address")
        .port();

    let coordinator = CoordinatorClient::new(config.coordinator_url)
        .unwrap_or_else(|error| panic!("failed to initialize coordinator client: {error}"));
    let registration = coordinator
        .register(identity.public_key, listen_port)
        .unwrap_or_else(|error| panic!("failed to register node: {error}"));

    tracing::info!(
        node_id = registration.node_id,
        overlay_ip = %registration.overlay_ip,
        lease_expires_at = %registration.lease_expires_at,
        "registered with coordinator"
    );

    coordinator
        .clone()
        .spawn_heartbeat(registration.node_id, listen_port)
        .unwrap_or_else(|error| panic!("failed to start lease maintenance: {error}"));

    let discovered_peer = coordinator
        .wait_for_peer(registration.node_id)
        .unwrap_or_else(|error| panic!("failed to discover peer: {error}"));
    tracing::info!(
        peer_node_id = discovered_peer.node_id,
        peer_overlay_ip = %discovered_peer.overlay_ip,
        peer_endpoint = %discovered_peer.endpoint,
        "discovered peer"
    );

    let device = tun_device::create(registration.overlay_ip);
    let (tun_reader, tun_writer) = device.split();
    let peer = PeerEndpoint::new(Some(discovered_peer.endpoint));
    let private_key = identity.private_key();
    let peer_public_key = boringtun::x25519::PublicKey::from(discovered_peer.public_key);

    let tunn = Tunn::new(private_key, peer_public_key, None, Some(25), 0, None);

    let tunnel = Tunnel::new(tunn, socket, peer);

    if let Err(error) = tunnel.run(tun_reader, tun_writer) {
        tracing::error!(%error, "tunnel stopped");
        std::process::exit(1);
    }
}
