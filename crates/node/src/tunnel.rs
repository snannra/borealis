use crate::peer::PeerEndpoint;
use boringtun::noise::{Tunn, TunnResult};
use std::io::{ErrorKind, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tun::{Reader, Writer};

const BUF_SIZE: usize = 65535;

pub struct Tunnel {
    tunn: Arc<Mutex<Tunn>>,
    socket: Arc<UdpSocket>,
    peer: Arc<PeerEndpoint>,
}

impl Tunnel {
    pub fn new(tunn: Tunn, socket: UdpSocket, peer: PeerEndpoint) -> Self {
        Self {
            tunn: Arc::new(Mutex::new(tunn)),
            socket: Arc::new(socket),
            peer: Arc::new(peer),
        }
    }

    pub fn run(self, tun_reader: Reader, tun_writer: Writer) -> Result<(), String> {
        let tunnel = Arc::new(self);
        let mut workers = Vec::new();

        {
            let tunnel = Arc::clone(&tunnel);

            let handle = thread::Builder::new()
                .name("udp-to-tun".into())
                .spawn(move || tunnel.udp_to_tun(tun_writer))
                .map_err(|error| format!("failed to start udp-to-tun worker: {error}"))?;

            workers.push(("udp-to-tun", handle));
        }

        {
            let tunnel = Arc::clone(&tunnel);

            let handle = thread::Builder::new()
                .name("timer".into())
                .spawn(move || tunnel.timer_loop())
                .map_err(|error| format!("failed to start timer worker: {error}"))?;

            workers.push(("timer", handle));
        }

        {
            let handle = thread::Builder::new()
                .name("tun-to-udp".into())
                .spawn(move || tunnel.tun_to_udp(tun_reader))
                .map_err(|error| format!("failed to start tun-to-udp worker: {error}"))?;

            workers.push(("tun-to-udp", handle));
        }

        tracing::info!(workers = workers.len(), "tunnel workers started");

        loop {
            if let Some(index) = workers.iter().position(|(_, handle)| handle.is_finished()) {
                let (name, handle) = workers.swap_remove(index);

                return match handle.join() {
                    Ok(()) => Err(format!("{name} worker exited unexpectedly")),
                    Err(payload) => Err(format!(
                        "{name} worker panicked: {}",
                        panic_message(payload.as_ref())
                    )),
                };
            }

            thread::sleep(Duration::from_millis(100));
        }
    }

    fn tun_to_udp(&self, reader: Reader) {
        let mut tun_buf = [0u8; BUF_SIZE];
        let mut wg_buf = [0u8; BUF_SIZE];

        loop {
            let n = match reader.recv_timeout(&mut tun_buf, Duration::from_secs(1)) {
                Ok(n) => n,
                Err(error)
                    if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
                {
                    continue;
                }
                Err(error) => panic!("failed reading TUN: {error}"),
            };

            tracing::debug!(bytes = n, "read packet from TUN");

            let output = {
                let mut tunn = self.tunn.lock().unwrap();

                match tunn.encapsulate(&tun_buf[..n], &mut wg_buf) {
                    TunnResult::WriteToNetwork(buf) => Some(buf.to_vec()),
                    TunnResult::Done => {
                        tracing::debug!("WireGuard queued packet");
                        None
                    }
                    TunnResult::Err(e) => {
                        tracing::error!(error = ?e, "failed to encapsulate packet");
                        None
                    }
                    _ => None,
                }
            };

            if let Some(packet) = output {
                self.send_udp(&packet);
            }
        }
    }

    pub fn udp_to_tun(&self, mut writer: Writer) {
        let mut udp_buf = [0u8; BUF_SIZE];

        loop {
            let (n, src) = self
                .socket
                .recv_from(&mut udp_buf)
                .expect("UDP receive failed");

            tracing::debug!(bytes = n, source = %src, "received UDP packet");

            self.process_udp_packet(&udp_buf[..n], src, &mut writer)
        }
    }

    pub fn process_udp_packet(&self, packet: &[u8], src: SocketAddr, writer: &mut Writer) {
        let mut first = true;

        loop {
            let mut dst = [0u8; BUF_SIZE];
            let is_initial_decapsulation = first;

            let result = {
                let mut tunn = self.tunn.lock().unwrap();

                if first {
                    first = false;
                    tunn.decapsulate(Some(src.ip()), packet, &mut dst)
                } else {
                    tunn.decapsulate(None, &[], &mut dst)
                }
            };

            if is_initial_decapsulation
                && !matches!(&result, TunnResult::Err(_))
                && !is_cookie_challenge(packet, &result)
            {
                self.peer.update(src);
            }

            match result {
                TunnResult::WriteToNetwork(buf) => {
                    tracing::debug!(bytes = buf.len(), "WireGuard produced UDP packet");

                    self.send_udp(buf);
                }

                TunnResult::WriteToTunnelV4(buf, _) => {
                    tracing::debug!(bytes = buf.len(), "WireGuard produced IPv4 TUN packet");

                    writer
                        .write_all(buf)
                        .expect("failed writing IPv4 packet to TUN");
                }

                TunnResult::WriteToTunnelV6(buf, _) => {
                    tracing::debug!(bytes = buf.len(), "WireGuard produced IPv6 TUN packet");

                    writer
                        .write_all(buf)
                        .expect("failed writing IPv6 packet to TUN");
                }

                TunnResult::Done => {
                    break;
                }

                TunnResult::Err(e) => {
                    tracing::error!(error = ?e, "failed to decapsulate packet");
                    break;
                }
            }
        }
    }

    fn timer_loop(&self) {
        loop {
            thread::sleep(Duration::from_millis(250));

            let mut dst = [0u8; BUF_SIZE];

            let packet = {
                let mut tunn = self.tunn.lock().unwrap();

                match tunn.update_timers(&mut dst) {
                    TunnResult::WriteToNetwork(buf) => Some(buf.to_vec()),

                    TunnResult::Err(e) => {
                        tracing::error!(error = ?e, "WireGuard timer update failed");
                        None
                    }
                    _ => None,
                }
            };

            if let Some(packet) = packet {
                tracing::debug!(bytes = packet.len(), "WireGuard timer produced UDP packet");
                self.send_udp(&packet);
            }
        }
    }

    fn send_udp(&self, packet: &[u8]) {
        let Some(dest) = self.peer.get() else {
            tracing::warn!("cannot send packet because peer endpoint is unknown");
            return;
        };

        tracing::debug!(bytes = packet.len(), destination = %dest, "sending UDP packet");

        self.socket.send_to(packet, dest).expect("UDP send failed");
    }
}

fn is_cookie_challenge(packet: &[u8], result: &TunnResult<'_>) -> bool {
    const HANDSHAKE_INIT_TYPE: [u8; 4] = [1, 0, 0, 0];
    const COOKIE_REPLY_SIZE: usize = 64;

    packet.starts_with(&HANDSHAKE_INIT_TYPE)
        && matches!(result, TunnResult::WriteToNetwork(buf) if buf.len() == COOKIE_REPLY_SIZE)
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic payload")
}
