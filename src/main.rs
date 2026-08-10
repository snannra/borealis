use dotenvy;
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

    let mut config = Configuration::default();

    // local
    if node == 0 {
        config.address(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9)));
        config.destination(IpAddr::V4(Ipv4Addr::new(157, 230, 144, 32)));
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

    let mut read_from = [0u8; 1500];

    if node == 0 {
        let device = tun::Device::new(&config).unwrap();
        loop {
            let num_bytes = device
                .recv(&mut buffer)
                .expect("Failed to read into buffer");
            println!("Read in {} bytes.", num_bytes);
            println!("{:?}", &buffer[..num_bytes]);
            socket
                .send_to(&buffer[..num_bytes], "157.230.144.32:51820")
                .unwrap();
        }
    } else {
        loop {
            let (bytes_read, _) = socket.recv_from(&mut read_from).unwrap();
            println!("Read in {} bytes.", bytes_read);
            println!("{:?}", &read_from[..bytes_read]);
        }
    }
}
