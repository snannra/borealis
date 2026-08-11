use boringtun::noise::TunnResult;
use dotenvy;
use serde_json;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
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
    // let public_key: Vec<u8> = serde_json::from_str(
    //     std::env::var("PUBLIC_KEY")
    //         .unwrap_or("Node not defined".to_string())
    //         .parse::<String>()
    //         .unwrap()
    //         .as_str(),
    // )
    // .unwrap();

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

    let socket = UdpSocket::bind(ip).unwrap();

    let mut buffer = [0u8; 1500];
    let mut dst_buffer = [0u8; 1500];

    let mut read_from = [0u8; 1500];

    let mut tunn = boringtun::noise::Tunn::new(my_secret, peer_public, None, Some(25), 0, None);

    if node == 0 {
        let device = tun::Device::new(&config).unwrap();
        loop {
            let num_bytes = device
                .recv(&mut buffer)
                .expect("Failed to read into buffer");
            println!("Read in {} bytes.", num_bytes);
            println!("{:?}", &buffer[..num_bytes]);
            match tunn.encapsulate(&buffer[..num_bytes], &mut dst_buffer) {
                TunnResult::Done => {}
                TunnResult::Err(e) => {
                    println!("Error: {e:?}");
                }
                TunnResult::WriteToNetwork(written_buf) => {
                    socket.send_to(written_buf, "157.230.144.32:51820").unwrap();
                }
                TunnResult::WriteToTunnelV4(_, _) => {}
                TunnResult::WriteToTunnelV6(_, _) => {}
            }
        }
    } else {
        loop {
            let (bytes_read, src_addr) = socket.recv_from(&mut read_from).unwrap();
            println!("Read in {} bytes.", bytes_read);
            println!("{:?}", &read_from[..bytes_read]);
            match tunn.decapsulate(
                Some(src_addr.ip()),
                &read_from[..bytes_read],
                &mut dst_buffer,
            ) {
                TunnResult::Done => {}
                TunnResult::Err(e) => {
                    println!("Error: {e:?}");
                }
                TunnResult::WriteToNetwork(buf) => {
                    socket.send_to(buf, src_addr).unwrap();
                    loop {
                        let mut tmp = [0u8; 1500];
                        match tunn.decapsulate(None, &[], &mut tmp) {
                            TunnResult::WriteToNetwork(b) => {
                                socket.send_to(b, src_addr).unwrap();
                            }
                            _ => break,
                        }
                    }
                }
                TunnResult::WriteToTunnelV4(buf, _addr) => {
                    println!("{:?}", buf);
                }
                TunnResult::WriteToTunnelV6(_, _) => {}
            }
        }
    }
}
