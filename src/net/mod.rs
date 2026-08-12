extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use crate::drivers::i82540em::DEVICE;
use crate::time::{Instant, sleep};
use arp::{ARP_PACKET, ARPOperation, ARPPacket, HardwareType, ProtocolType};
use device::NetworkDevice;
use ethernet::{ETHERNET_HEADER, EthernetFrame, address::EthernetAddress, ethertype::EtherType};
use icmp::{ECHO_PACKET, ICMPPacket, icmp_type::IcmpType};
use ipv4::{IPV4_PACKET, IPv4Packet, address::IPv4Address, protocol::Protocol, ttl::TimeToLive};
use rx::NetContext;

pub mod arp;
pub mod checksum;
pub mod device;
pub mod error;
pub mod ethernet;
pub mod icmp;
pub mod ipv4;
pub mod rx;
#[cfg(test)]
mod tests;

pub fn generate_echo_reply(
    ctx: &NetContext,
    request_frame: &EthernetFrame<&[u8]>,
    request_ipv4: &IPv4Packet<&[u8]>,
    request_echo: &ICMPPacket<&[u8]>,
) -> EthernetFrame<Vec<u8>> {
    assert!(request_ipv4.packet_length() >= IPV4_PACKET + ECHO_PACKET);
    let packet = vec![0u8; ETHERNET_HEADER + request_ipv4.packet_length()];
    let mut frame = EthernetFrame::new(packet).unwrap();

    frame
        .set_destination(request_frame.source())
        .set_source(
            ctx.hardware_address()
                .unwrap_or(request_frame.destination()),
        )
        .set_ethertype(EtherType::IPv4);

    let mut ipv4 = IPv4Packet::new(frame.payload_mut()).unwrap();
    ipv4.set_version_and_length()
        .set_packet_length(request_ipv4.packet_length())
        .set_protocol(Protocol::ICMP)
        .set_destination(request_ipv4.source())
        .set_source(ctx.ipv4_address().unwrap_or(request_ipv4.destination()))
        .set_ttl(TimeToLive::max())
        .compute_checksum();

    let mut icmp = ICMPPacket::new(ipv4.payload_mut()).unwrap();
    icmp.set_code(0)
        .set_icmp_type(IcmpType::EchoReply)
        .set_payload(request_echo.payload())
        .compute_checksum();

    frame
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

pub use rx::rx_loop;

pub struct StateMachine {
    ipv4: Option<IPv4Address>,
    last_arp_request: Instant,
}

static STATE_MACHINE: spin::Mutex<StateMachine> = spin::Mutex::new(StateMachine {
    ipv4: None,
    last_arp_request: Instant::zero(),
});

pub async fn net_loop() {
    loop {
        let mut state_machine = STATE_MACHINE.lock();
        if let Some(device) = DEVICE.get()
            && state_machine.ipv4.is_none()
            && Instant::now() - state_machine.last_arp_request > Duration::from_secs(5)
        {
            state_machine.last_arp_request = Instant::now();
            send_arp_request(device);
        }
        drop(state_machine);

        sleep(Duration::from_secs(1)).await;
    }
}

pub fn generate_arp_reply(
    ctx: &NetContext,
    request_frame: &EthernetFrame<&[u8]>,
    request_arp: &ARPPacket<&[u8]>,
) -> EthernetFrame<Vec<u8>> {
    let packet = vec![0; ETHERNET_HEADER + ARP_PACKET];
    let mut frame = EthernetFrame::new(packet).unwrap();

    let hardware_address = ctx.hardware_address().unwrap();

    frame
        .set_destination(request_frame.source())
        .set_source(hardware_address)
        .set_ethertype(EtherType::ARP);

    let mut arp = ARPPacket::new(frame.payload_mut()).unwrap();
    arp.set_hardware_type(HardwareType::Ethernet)
        .set_protocol_type(ProtocolType::IPv4)
        .set_hardware_length(EthernetAddress::SIZE as u8)
        .set_protocol_length(IPv4Address::SIZE as u8)
        .set_operation(ARPOperation::Reply)
        .set_sender_hardware_address(hardware_address)
        .set_sender_protocol_address(request_arp.target_protocol_address())
        .set_target_hardware_address(request_arp.sender_hardware_address())
        .set_target_protocol_address(request_arp.sender_protocol_address());

    frame
}
