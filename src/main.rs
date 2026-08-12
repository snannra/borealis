use boringtun::noise::TunnResult;
use dotenvy;
use serde_json;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread::spawn;
use tun::{self, Configuration};

fn main() {
    dotenvy::dotenv().unwrap();

    let node = std::env::var("NODE")
        .unwrap_or("Node not defined".to_string())
        .parse::<u32>()
        .unwrap();

    let ip = std::env::var("PUBLIC_IP")
        .unwrap_or("Node not defined".to_string())
        .parse::<String>()
        .unwrap();

    let static_private: Vec<u8> = serde_json::from_str(
        std::env::var("PRIVATE_KEY")
            .unwrap_or("Node not defined".to_string())
            .parse::<String>()
            .unwrap()
            .as_str(),
    )
    .unwrap();
    let peer_public_key: Vec<u8> = serde_json::from_str(
        std::env::var("PEER_PUBLIC_KEY")
            .unwrap_or("Node not defined".to_string())
            .parse::<String>()
            .unwrap()
            .as_str(),
    )
    .unwrap();

    let peer_socket = std::env::var("PEER_SOCKET")
        .expect("Couldn't unwrap peer socket")
        .parse::<String>()
        .unwrap();

    let private_arr: [u8; 32] = static_private
        .try_into()
        .expect("private key must be 32 bytes");
    let peer_public_arr: [u8; 32] = peer_public_key
        .try_into()
        .expect("peer public key must be 32 bytes");

    let my_secret = boringtun::x25519::StaticSecret::from(private_arr);
    let peer_public = boringtun::x25519::PublicKey::from(peer_public_arr);

    let mut config = Configuration::default();

    // local
    if node == 0 {
        config.address(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)));
        config.mtu(1500);
        config.up();
    } else {
        // droplet
        config.address(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 10)));
        config.mtu(1500);
        config.up();
    }

    let socket = Arc::new(UdpSocket::bind(ip).unwrap());

    let mut decap_dst_buffer = [0u8; 1500];
    let mut encap_dst_buffer = [0u8; 1500];

    let mut read_from = [0u8; 1500];

    let tunn_protocol = Arc::new(Mutex::new(boringtun::noise::Tunn::new(
        my_secret,
        peer_public,
        None,
        Some(25),
        0,
        None,
    )));

    let tunn = tunn_protocol.clone();

    let tunn_timer = tunn_protocol.clone(); // another Arc clone
    let socket_timer = Arc::clone(&socket);

    let device = tun::Device::new(&config).unwrap();

    let learned_peer: Arc<Mutex<Option<SocketAddr>>> = Arc::new(Mutex::new(None));

    let socket_clone = Arc::clone(&socket);
    let learned_peer_recv = Arc::clone(&learned_peer);
    let learned_peer_send = Arc::clone(&learned_peer);
    spawn(move || {
        loop {
            let (bytes_read, src_addr) = socket_clone.recv_from(&mut read_from).unwrap();
            println!("Read in {} bytes.", bytes_read);
            println!("{:?}", &read_from[..bytes_read]);
            *learned_peer_recv.lock().unwrap() = Some(src_addr);

            match tunn_protocol.lock().unwrap().decapsulate(
                Some(src_addr.ip()),
                &read_from[..bytes_read],
                &mut decap_dst_buffer,
            ) {
                TunnResult::Done => {
                    println!("decap: DONE");
                }
                TunnResult::Err(e) => {
                    println!("decap Error: {e:?}");
                }
                TunnResult::WriteToNetwork(buf) => {
                    println!(
                        "decap: WriteToNetwork {} bytes - sending response",
                        buf.len()
                    );
                    socket_clone.send_to(buf, src_addr).unwrap();
                    loop {
                        let mut tmp = [0u8; 1500];
                        match tunn_protocol
                            .lock()
                            .unwrap()
                            .decapsulate(None, &[], &mut tmp)
                        {
                            TunnResult::WriteToNetwork(b) => {
                                socket_clone.send_to(b, src_addr).unwrap();
                            }
                            _ => break,
                        }
                    }
                }
                TunnResult::WriteToTunnelV4(buf, _addr) => {
                    println!("decap: DECRYPTED PACKET {:?}", buf);
                }
                TunnResult::WriteToTunnelV6(_, _) => {}
            }
        }
    });

    let timer_socket = peer_socket.clone();

    spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(250));
            let mut tmp = [0u8; 1500];
            match tunn_timer.lock().unwrap().update_timers(&mut tmp) {
                TunnResult::WriteToNetwork(buf) => {
                    socket_timer.send_to(buf, timer_socket.clone()).unwrap();
                }
                _ => {}
            }
        }
    });

    let mut buffer = [0u8; 1500];

    loop {
        let num_bytes = device
            .recv(&mut buffer)
            .expect("Failed to read into buffer");
        println!("Read in {} bytes.", num_bytes);
        println!("{:?}", &buffer[..num_bytes]);
        match tunn
            .lock()
            .unwrap()
            .encapsulate(&buffer[..num_bytes], &mut encap_dst_buffer)
        {
            TunnResult::Done => {
                println!("encap: Done (queued?)");
            }
            TunnResult::Err(e) => {
                println!("Error: {e:?}");
            }
            TunnResult::WriteToNetwork(written_buf) => {
                println!(
                    "encap: writetonetwork {} encrypted bytes to peer",
                    written_buf.len()
                );
                if let Some(dest) = *learned_peer_send.lock().unwrap() {
                    socket.send_to(written_buf, dest).unwrap();
                }
            }
            TunnResult::WriteToTunnelV4(_, _) => {}
            TunnResult::WriteToTunnelV6(_, _) => {}
        }
    }
}
