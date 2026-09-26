extern crate alloc;

use crate::net::arp::{ARP_PACKET, ARPCache, ARPOperation, ARPPacket, HardwareType, ProtocolType};
use crate::net::ethernet::address::EthernetAddress;
use crate::net::ethernet::ethertype::EtherType;
use crate::net::ethernet::{ETHERNET_HEADER, EthernetFrame};
use crate::net::icmp::icmp_type::IcmpType;
use crate::net::icmp::{ECHO_PACKET, ICMPPacket};
use crate::net::ipv4::address::IPv4Address;
use crate::net::ipv4::protocol::Protocol;
use crate::net::ipv4::{IPV4_PACKET, IPv4Packet};
use crate::net::rx::{NetContext, ProcessingResult, process_ethernet_frame};
use crate::net::socket::TCPConnectionPool;
use crate::net::tx::{L3, L4, L7};

#[test_case]
fn icmp_echo_request_is_met_with_reply() {
    let my_hardware_address = EthernetAddress::from_bytes(&[1, 2, 3, 4, 5, 6]);
    let mut packet = [0; ETHERNET_HEADER + IPV4_PACKET + ECHO_PACKET];
    let mut frame = EthernetFrame::new(&mut packet).unwrap();
    frame
        .set_destination(my_hardware_address)
        .set_source(EthernetAddress::from_bytes(&[7, 8, 9, 10, 11, 12]))
        .set_ethertype(EtherType::IPv4);
    let mut ipv4 = IPv4Packet::new(frame.payload_mut()).unwrap();
    ipv4.set_version_and_length()
        .set_packet_length(IPV4_PACKET + ECHO_PACKET)
        .set_protocol(Protocol::ICMP)
        .set_destination(IPv4Address::new(192, 168, 0, 5))
        .set_source(IPv4Address::new(192, 168, 0, 19));
    let mut icmp = ICMPPacket::new(ipv4.payload_mut()).unwrap();
    icmp.set_code(0)
        .set_icmp_type(IcmpType::EchoRequest)
        .set_echo_identifier(0x4242)
        .set_echo_sequence(0x1234)
        .compute_checksum();
    let frame = EthernetFrame::new(packet.as_slice()).unwrap();

    let Ok(ProcessingResult::Respond(response)) = process_ethernet_frame(
        &NetContext::from_hardware_address(my_hardware_address),
        &mut TCPConnectionPool::default(),
        &mut ARPCache::default(),
        &frame,
    ) else {
        panic!("Expected response")
    };

    // `Respond` now carries an `L3` descriptor -- the actual Ethernet framing
    // (and MAC resolution) happens later, in `send_l3`, not here anymore.
    let L3::IPv4 {
        source,
        destination,
        protocol,
        next,
    } = response
    else {
        panic!("expected an IPv4 response")
    };
    assert_eq!(source, IPv4Address::new(192, 168, 0, 5));
    assert_eq!(destination, IPv4Address::new(192, 168, 0, 19));
    assert_eq!(protocol, Protocol::ICMP);

    let L4::IcmpEcho {
        code,
        icmp_type,
        next,
    } = next
    else {
        panic!("expected an ICMP echo reply")
    };
    assert_eq!(code, 0);
    assert_eq!(icmp_type, IcmpType::EchoReply);

    let L7::Buffer(payload) = next else {
        panic!("expected a raw buffer payload")
    };
    // ICMP's "payload" (byte 4 onward) includes the echo identifier/sequence
    // fields themselves, not just app-level data beyond them.
    assert_eq!(payload, [0x42, 0x42, 0x12, 0x34]);
}

/// The actual ARP semantics (answering a request for a known address,
/// resolving a pending request from a reply, ignoring unrelated ones...) are
/// covered by `arp::cache::tests`, against the `Cache` directly. Answering a
/// request for real goes through `Cache::accept` -> `tx::send_l3`, which needs
/// a live `DEVICE` -- not available in this test harness. This test only
/// checks that an incoming ARP frame is routed to the cache at all.
#[test_case]
fn arp_request_is_dispatched_for_processing() {
    let target_hw = EthernetAddress::from_bytes(&[7, 8, 9, 10, 11, 12]);
    let target_ip = IPv4Address::new(10, 0, 2, 3);
    let sender_hw = EthernetAddress::from_bytes(&[1, 2, 3, 4, 5, 6]);
    let sender_ip = IPv4Address::new(10, 0, 2, 2);

    let mut packet = [0; ETHERNET_HEADER + ARP_PACKET];
    let mut frame = EthernetFrame::new(&mut packet).unwrap();
    frame
        .set_destination(EthernetAddress::BROADCAST)
        .set_source(sender_hw)
        .set_ethertype(EtherType::ARP);
    let mut arp = ARPPacket::new(frame.payload_mut()).unwrap();
    arp.set_hardware_type(HardwareType::Ethernet)
        .set_protocol_type(ProtocolType::IPv4)
        .set_hardware_length(EthernetAddress::SIZE as u8)
        .set_protocol_length(IPv4Address::SIZE as u8)
        .set_operation(ARPOperation::Request)
        .set_sender_hardware_address(sender_hw)
        .set_sender_protocol_address(sender_ip)
        .set_target_hardware_address(EthernetAddress::BROADCAST)
        .set_target_protocol_address(target_ip);
    let frame = EthernetFrame::new(packet.as_slice()).unwrap();

    let ctx = NetContext::from_addresses(target_hw, target_ip);
    let result = process_ethernet_frame(
        &ctx,
        &mut TCPConnectionPool::default(),
        &mut ARPCache::default(),
        &frame,
    )
    .unwrap();

    assert!(matches!(result, ProcessingResult::PushArpMessage(_)));
}
