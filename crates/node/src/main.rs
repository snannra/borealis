mod config;
mod peer;
mod transport;
mod tun_device;
mod tunnel;

use boringtun::noise::Tunn;
use config::DeviceConfig;
use peer::PeerEndpoint;
use tunnel::Tunnel;

fn main() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let config = DeviceConfig::from_env();
    tracing::info!(
        node = config.node,
        bind_address = %config.bind_socket,
        peer_endpoint = ?config.peer_socket,
        tunnel_ip = %config.tunnel_ip,
        "loaded tunnel configuration"
    );

    let device = tun_device::create(&config);

    let (tun_reader, tun_writer) = device.split();

    let socket = transport::bind_udp(config.bind_socket);

    let peer = PeerEndpoint::new(config.peer_socket);

    let private_key = boringtun::x25519::StaticSecret::from(config.private_key);

    let peer_public_key = boringtun::x25519::PublicKey::from(config.peer_public_key);

    let tunn = Tunn::new(private_key, peer_public_key, None, Some(25), 0, None);

    let tunnel = Tunnel::new(tunn, socket, peer);

    if let Err(error) = tunnel.run(tun_reader, tun_writer) {
        tracing::error!(%error, "tunnel stopped");
        std::process::exit(1);
    }
}
