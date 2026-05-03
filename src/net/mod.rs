use crate::net::{
    arp::ARPPacket,
    error::BufferTooSmall,
    ethernet::{EthernetFrame, ethertype::EtherType},
    ipv4::IPv4Packet,
};

pub mod arp;
pub mod error;
pub mod ethernet;
pub mod ipv4;

pub fn handle_incoming_ethernet_packet(buffer: &[u8]) {
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
}
