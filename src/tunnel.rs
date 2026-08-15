use crate::peer::PeerEndpoint;
use boringtun::noise::{Tunn, TunnResult};
use std::io::{ErrorKind, Write};
use std::net::UdpSocket;
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

    pub fn run(self, tun_reader: Reader, tun_writer: Writer) {
        let tunnel = Arc::new(self);

        {
            let tunnel = Arc::clone(&tunnel);

            thread::spawn(move || {
                tunnel.udp_to_tun(tun_writer);
            });
        }

        {
            let tunnel = Arc::clone(&tunnel);

            thread::spawn(move || {
                tunnel.timer_loop();
            });
        }

        tunnel.tun_to_udp(tun_reader);
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

            println!("TUN -> WG: {n} bytes");

            let output = {
                let mut tunn = self.tunn.lock().unwrap();

                match tunn.encapsulate(&tun_buf[..n], &mut wg_buf) {
                    TunnResult::WriteToNetwork(buf) => Some(buf.to_vec()),
                    TunnResult::Done => {
                        println!("WG queued packet");
                        None
                    }
                    TunnResult::Err(e) => {
                        eprintln!("encapsulation error: {e:?}");
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

            println!("UDP -> WG: {n} bytes from {src}");

            self.peer.update(src);

            self.process_udp_packet(&udp_buf[..n], src.ip(), &mut writer)
        }
    }

    pub fn process_udp_packet(&self, packet: &[u8], src_ip: std::net::IpAddr, writer: &mut Writer) {
        let mut first = true;

        loop {
            let mut dst = [0u8; BUF_SIZE];

            let result = {
                let mut tunn = self.tunn.lock().unwrap();

                if first {
                    first = false;
                    tunn.decapsulate(Some(src_ip), packet, &mut dst)
                } else {
                    tunn.decapsulate(None, &[], &mut dst)
                }
            };

            match result {
                TunnResult::WriteToNetwork(buf) => {
                    println!("WG -> UDP: {} bytes", buf.len());

                    self.send_udp(buf);
                }

                TunnResult::WriteToTunnelV4(buf, _) => {
                    println!("WG -> TUN IPv4: {} bytes", buf.len());

                    writer
                        .write_all(buf)
                        .expect("failed writing IPv4 packet to TUN");
                }

                TunnResult::WriteToTunnelV6(buf, _) => {
                    println!("WG -> TUN IPv6: {} bytes", buf.len());

                    writer
                        .write_all(buf)
                        .expect("failed writing IPv6 packet to TUN");
                }

                TunnResult::Done => {
                    break;
                }

                TunnResult::Err(e) => {
                    eprintln!("decapsulation error: {e:?}");
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
                        eprintln!("timer error: {e:?}");
                        None
                    }
                    _ => None,
                }
            };

            if let Some(packet) = packet {
                println!("TIMER -> UDP: {} bytes", packet.len());
                self.send_udp(&packet);
            }
        }
    }

    fn send_udp(&self, packet: &[u8]) {
        let Some(dest) = self.peer.get() else {
            eprintln!("cannot send packet: peer endpoint unknown");
            return;
        };

        self.socket.send_to(packet, dest).expect("UDP send failed");
    }
}
