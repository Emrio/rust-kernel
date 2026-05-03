use crate::net::{
    arp::{ARP_PACKET, ARPOperation, ARPPacket, HardwareType, ProtocolType},
    device::NetworkDevice,
    error::BufferTooSmall,
    ethernet::{ETHERNET_HEADER, EthernetFrame, address::EthernetAddress, ethertype::EtherType},
    ipv4::{IPv4Packet, address::IPv4Address},
};

pub mod arp;
pub mod device;
pub mod error;
pub mod ethernet;
pub mod ipv4;

pub fn handle_incoming_ethernet_packet(buffer: &[u8]) {
    kprintln!("> Processing incoming packet...");

    let Ok(frame) = EthernetFrame::new(buffer) else {
        kprintln!(
            "Error: Couldn't parse incoming frame of size {}",
            buffer.len()
        );
        return;
    };

    match frame.ethertype() {
        EtherType::ARP => match ARPPacket::new(frame.payload()) {
            Ok(arp) => kprintln!("ARP packet: {}", arp),
            Err(BufferTooSmall) => kprintln!("Error: ARP packet too small!"),
        },
        EtherType::IPv4 => match IPv4Packet::new(frame.payload()) {
            Ok(ipv4) => kprintln!("IPv4 packet: {}", ipv4),
            Err(BufferTooSmall) => kprintln!("Error: IPv4 packet too small!"),
        },
    };

    kprintln!("> OK");
}

pub fn send_arp_request(device: &impl NetworkDevice) {
    kprintln!("< ARP Request:");

    let mut packet = [0; ETHERNET_HEADER + ARP_PACKET];
    let mut frame = EthernetFrame::new(&mut packet).unwrap();

    frame
        .set_destination(EthernetAddress::BROADCAST)
        .set_source(device.hardware_address())
        .set_ethertype(EtherType::ARP);
    kprintln!("< {}", frame);

    let mut arp = ARPPacket::new(frame.payload_mut()).unwrap();
    arp.set_hardware_type(HardwareType::Ethernet)
        .set_protocol_type(ProtocolType::IPv4)
        .set_hardware_length(EthernetAddress::SIZE as u8)
        .set_protocol_length(IPv4Address::SIZE as u8)
        .set_operation(ARPOperation::Request)
        .set_sender_hardware_address(device.hardware_address())
        .set_sender_protocol_address(IPv4Address::new(10, 0, 2, 3))
        .set_target_hardware_address(EthernetAddress::BROADCAST)
        .set_target_protocol_address(IPv4Address::new(10, 0, 2, 2));
    kprintln!("< {}", arp);

    device.send_packet(frame.into_inner());
}
