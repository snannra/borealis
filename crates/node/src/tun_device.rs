use crate::config::DeviceConfig;
use std::net::{IpAddr, Ipv4Addr};
use tun::{Configuration, Device};

const TUN_MTU: u16 = 1420;

pub fn create(config: &DeviceConfig) -> Device {
    let mut tun_config = Configuration::default();

    tun_config
        .address(config.tunnel_ip)
        .netmask(IpAddr::V4(Ipv4Addr::new(255, 255, 255, 0)))
        .mtu(TUN_MTU)
        .up();

    let device = Device::new(&tun_config).expect("failed to create TUN device");

    tracing::info!(address = %config.tunnel_ip, mtu = TUN_MTU, "created TUN device");

    device
}
