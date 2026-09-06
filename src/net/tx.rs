extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::net::arp::{ARP_PACKET, ARPOperation, ARPPacket, HardwareType, ProtocolType};
use crate::net::device::NetworkDevice;
use crate::net::error::BufferTooSmall;
use crate::net::ethernet::address::EthernetAddress;
use crate::net::ethernet::ethertype::EtherType;
use crate::net::ethernet::{ETHERNET_HEADER, EthernetFrame};
use crate::net::icmp::icmp_type::IcmpType;
use crate::net::icmp::{ECHO_PACKET, ICMPPacket};
use crate::net::ipv4::address::IPv4Address;
use crate::net::ipv4::protocol::Protocol;
use crate::net::ipv4::ttl::TimeToLive;
use crate::net::ipv4::{IPV4_PACKET, IPv4Packet};
use crate::net::rx::NetContext;
use crate::net::udp::{UDP_HEADER, UDPPacket};

pub fn generate_arp_reply(
    ctx: &NetContext,
    request_frame: &EthernetFrame<&[u8]>,
    request_arp: &ARPPacket<&[u8]>,
) -> Result<EthernetFrame<Vec<u8>>, BufferTooSmall> {
    let packet = vec![0; ETHERNET_HEADER + ARP_PACKET];
    let mut frame = EthernetFrame::new(packet)?;

    frame
        .set_destination(request_frame.source())
        .set_source(ctx.hardware_address())
        .set_ethertype(EtherType::ARP);

    let mut arp = ARPPacket::new(frame.payload_mut())?;
    arp.set_hardware_type(HardwareType::Ethernet)
        .set_protocol_type(ProtocolType::IPv4)
        .set_hardware_length(EthernetAddress::SIZE as u8)
        .set_protocol_length(IPv4Address::SIZE as u8)
        .set_operation(ARPOperation::Reply)
        .set_sender_hardware_address(ctx.hardware_address())
        .set_sender_protocol_address(request_arp.target_protocol_address())
        .set_target_hardware_address(request_arp.sender_hardware_address())
        .set_target_protocol_address(request_arp.sender_protocol_address());

    Ok(frame)
}

pub fn send_arp_request(device: &impl NetworkDevice) {
    kprintln!("<- Sending ARP request");

    let mut packet = [0; ETHERNET_HEADER + ARP_PACKET];
    let mut frame = EthernetFrame::new(&mut packet).unwrap();

    frame
        .set_destination(EthernetAddress::BROADCAST)
        .set_source(device.hardware_address())
        .set_ethertype(EtherType::ARP);
    kprintln!("<- {}", frame);

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
    kprintln!("<- {}", arp);

    device.send_packet(frame.into_inner());
}

pub fn generate_echo_reply(
    ctx: &NetContext,
    request_frame: &EthernetFrame<&[u8]>,
    request_ipv4: &IPv4Packet<&[u8]>,
    request_echo: &ICMPPacket<&[u8]>,
) -> Result<EthernetFrame<Vec<u8>>, BufferTooSmall> {
    assert!(request_ipv4.packet_length() >= IPV4_PACKET + ECHO_PACKET);
    let packet = vec![0u8; ETHERNET_HEADER + request_ipv4.packet_length()];
    let mut frame = EthernetFrame::new(packet)?;

    frame
        .set_destination(request_frame.source())
        .set_source(ctx.hardware_address())
        .set_ethertype(EtherType::IPv4);

    let mut ipv4 = IPv4Packet::new(frame.payload_mut())?;
    ipv4.set_version_and_length()
        .set_packet_length(request_ipv4.packet_length())
        .set_protocol(Protocol::ICMP)
        .set_destination(request_ipv4.source())
        .set_source(ctx.ipv4_address().unwrap_or(request_ipv4.destination()))
        .set_ttl(TimeToLive::max())
        .compute_checksum();

    let mut icmp = ICMPPacket::new(ipv4.payload_mut())?;
    icmp.set_code(0)
        .set_icmp_type(IcmpType::EchoReply)
        .set_payload(request_echo.payload())
        .compute_checksum();

    Ok(frame)
}

pub fn generate_pong_udp_packet(
    ctx: &NetContext,
    request_frame: &EthernetFrame<&[u8]>,
    request_ipv4: &IPv4Packet<&[u8]>,
    request_udp: &UDPPacket<&[u8]>,
) -> Result<EthernetFrame<Vec<u8>>, BufferTooSmall> {
    const PREFIX: &str = "Pong: ";

    assert!(request_ipv4.packet_length() >= IPV4_PACKET + UDP_HEADER);
    let packet = vec![0u8; ETHERNET_HEADER + request_ipv4.packet_length() + PREFIX.len()];
    let mut frame = EthernetFrame::new(packet)?;

    frame
        .set_destination(request_frame.source())
        .set_source(ctx.hardware_address())
        .set_ethertype(EtherType::IPv4);

    let mut ipv4 = IPv4Packet::new(frame.payload_mut())?;
    ipv4.set_version_and_length()
        .set_packet_length(request_ipv4.packet_length() + PREFIX.len())
        .set_protocol(Protocol::UDP)
        .set_destination(request_ipv4.source())
        .set_source(ctx.ipv4_address().unwrap_or(request_ipv4.destination()))
        .set_ttl(TimeToLive::max())
        .compute_checksum();

    let mut payload = vec![0u8; request_udp.payload().len() + PREFIX.len()];
    payload[0..PREFIX.len()].copy_from_slice(PREFIX.as_bytes());
    payload[PREFIX.len()..].copy_from_slice(request_udp.payload());

    let mut udp = UDPPacket::new(ipv4.payload_mut())?;
    udp.set_source(request_udp.destination())
        .set_destination(request_udp.source())
        .set_length(request_udp.packet_length() + PREFIX.len())
        .set_payload(&payload);

    Ok(frame)
}
