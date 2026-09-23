extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::net::arp::{ARP_PACKET, ARPOperation, ARPPacket, HardwareType, ProtocolType};
use crate::net::ethernet::address::EthernetAddress;
use crate::net::ethernet::ethertype::EtherType;
use crate::net::ethernet::{ETHERNET_HEADER, EthernetFrame};
use crate::net::icmp::icmp_type::IcmpType;
use crate::net::icmp::{ECHO_PACKET, ICMPPacket};
use crate::net::ipv4::address::IPv4Address;
use crate::net::ipv4::protocol::Protocol;
use crate::net::ipv4::ttl::TimeToLive;
use crate::net::ipv4::{IPV4_PACKET, IPv4Packet};
use crate::net::rx::{NetContext, ProcessingResult, process_ethernet_frame};
use crate::net::tcp::TCPPacket;
use crate::net::tcp::protocol::{ConnectionPool, Listen};
use crate::net::tcp::sequence::Sequence;
use crate::net::tx::{L2, L3, L4, L7, build};

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
        &mut ConnectionPool::default(),
        &frame,
    ) else {
        panic!("Expected response")
    };

    let response = EthernetFrame::new(response).expect("valid ethernet frame");

    assert_eq!(response.source(), frame.destination());
    assert_eq!(response.destination(), frame.source());
    assert_eq!(
        response.into_inner(),
        [
            0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, // ethernet destination
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, // ethernet source
            0x08, 0x00, // ipv4
            0x45, 0x00, 0x00, 0x1c, 0x00, 0x00, 0x00, 0x00, 0xff, // ipv4 headers
            0x01, // protocol: icmp
            0x3a, 0x78, // ip checksum
            0xc0, 0xa8, 0x00, 0x05, // source
            0xc0, 0xa8, 0x00, 0x13, // destination
            0x00, 0x00, 0xab, 0x89, 0x42, 0x42, 0x12, 0x34 // icmp
        ]
    );
}

#[test_case]
fn arp_request_for_me_is_met_with_reply() {
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
    let Ok(ProcessingResult::Respond(response)) =
        process_ethernet_frame(&ctx, &mut ConnectionPool::default(), &frame)
    else {
        panic!("Expected response")
    };

    let response = EthernetFrame::new(response).expect("valid ethernet frame");

    assert_eq!(response.source(), target_hw);
    assert_eq!(response.destination(), frame.source());
    let Ok(arp_response) = ARPPacket::new(response.payload()) else {
        panic!("Expected ARP response")
    };

    assert_eq!(arp_response.hardware_type(), HardwareType::Ethernet);
    assert_eq!(arp_response.protocol_type(), ProtocolType::IPv4);
    assert_eq!(arp_response.operation(), ARPOperation::Reply);
    assert_eq!(arp_response.sender_hardware_address(), target_hw);
    assert_eq!(arp_response.sender_protocol_address(), target_ip);
    assert_eq!(arp_response.target_hardware_address(), sender_hw);
    assert_eq!(arp_response.target_protocol_address(), sender_ip);
}

#[allow(clippy::too_many_arguments)]
fn build_tcp_frame(
    eth_dst: EthernetAddress,
    eth_src: EthernetAddress,
    ip_dst: IPv4Address,
    ip_src: IPv4Address,
    tcp_src_port: u16,
    tcp_dst_port: u16,
    seq: u32,
    ack: u32,
    syn: bool,
    ack_flag: bool,
    fin: bool,
    payload: &[u8],
) -> Vec<u8> {
    build(L2::Ethernet {
        source: eth_src,
        destination: eth_dst,
        ethertype: EtherType::IPv4,
        next: L3::IPv4 {
            source: ip_src,
            destination: ip_dst,
            protocol: Protocol::TCP,
            next: L4::Tcp {
                source: tcp_src_port,
                destination: tcp_dst_port,
                sequence: seq.into(),
                acknowledgment: ack.into(),
                cwr: false,
                ece: false,
                urg: false,
                ack: ack_flag,
                psh: false,
                rst: false,
                syn,
                fin,
                window: 0,
                next: L7::Buffer(payload.to_vec()),
            },
        },
    })
    .unwrap()
}

/// End-to-end regression test for the exact sequence manually verified against
/// a real `nc` client: SYN -> SYN-ACK -> ACK -> data -> FIN -> our FIN -> final ACK.
/// Goes through `process_ethernet_frame` (not the bare TCB), so it also exercises
/// `ConnectionPool`/`Listen` wiring.
#[test_case]
fn tcp_full_connection_lifecycle_through_rx() {
    let server_hw = EthernetAddress::from_bytes(&[1, 2, 3, 4, 5, 6]);
    let server_ip = IPv4Address::new(10, 0, 0, 1);
    let client_hw = EthernetAddress::from_bytes(&[7, 8, 9, 10, 11, 12]);
    let client_ip = IPv4Address::new(10, 0, 0, 2);
    let client_port = 1234;
    let server_port = 4242;

    let ctx = NetContext::from_addresses(server_hw, server_ip);
    let mut pool = ConnectionPool::default();
    pool.listen(Listen::AnyAddress(server_port));

    // --- SYN ---
    let syn = build_tcp_frame(
        server_hw,
        client_hw,
        server_ip,
        client_ip,
        client_port,
        server_port,
        2000,
        0,
        true,
        false,
        false,
        &[],
    );
    let syn_frame = EthernetFrame::new(syn.as_slice()).unwrap();
    let Ok(ProcessingResult::Respond(syn_ack)) =
        process_ethernet_frame(&ctx, &mut pool, &syn_frame)
    else {
        panic!("expected a SYN-ACK response")
    };
    let syn_ack_frame = EthernetFrame::new(syn_ack.as_slice()).unwrap();
    let syn_ack_ip = IPv4Packet::new(syn_ack_frame.payload()).unwrap();
    let syn_ack_tcp = TCPPacket::new(syn_ack_ip.payload()).unwrap();
    assert!(syn_ack_tcp.syn());
    assert!(syn_ack_tcp.ack());
    assert_eq!(syn_ack_tcp.acknowledgment(), Sequence::from(2001));
    let server_iss: u32 = syn_ack_tcp.sequence().into();

    // --- ACK completing the handshake ---
    let ack = build_tcp_frame(
        server_hw,
        client_hw,
        server_ip,
        client_ip,
        client_port,
        server_port,
        2001,
        server_iss + 1,
        false,
        true,
        false,
        &[],
    );
    let ack_frame = EthernetFrame::new(ack.as_slice()).unwrap();
    let result = process_ethernet_frame(&ctx, &mut pool, &ack_frame).unwrap();
    assert!(matches!(result, ProcessingResult::Nothing));

    // --- data ---
    let data = build_tcp_frame(
        server_hw,
        client_hw,
        server_ip,
        client_ip,
        client_port,
        server_port,
        2001,
        server_iss + 1,
        false,
        true,
        false,
        b"hello",
    );
    let data_frame = EthernetFrame::new(data.as_slice()).unwrap();
    let Ok(ProcessingResult::Respond(data_ack)) =
        process_ethernet_frame(&ctx, &mut pool, &data_frame)
    else {
        panic!("expected an ACK for the data")
    };
    let data_ack_frame = EthernetFrame::new(data_ack.as_slice()).unwrap();
    let data_ack_ip = IPv4Packet::new(data_ack_frame.payload()).unwrap();
    let data_ack_tcp = TCPPacket::new(data_ack_ip.payload()).unwrap();
    assert_eq!(data_ack_tcp.acknowledgment(), Sequence::from(2006)); // 2001 + 5 bytes

    // --- FIN ---
    let fin = build_tcp_frame(
        server_hw,
        client_hw,
        server_ip,
        client_ip,
        client_port,
        server_port,
        2006,
        server_iss + 1,
        false,
        true,
        true,
        &[],
    );
    let fin_frame = EthernetFrame::new(fin.as_slice()).unwrap();
    let Ok(ProcessingResult::Respond(fin_reply)) =
        process_ethernet_frame(&ctx, &mut pool, &fin_frame)
    else {
        panic!("expected our own FIN in response")
    };
    let fin_reply_frame = EthernetFrame::new(fin_reply.as_slice()).unwrap();
    let fin_reply_ip = IPv4Packet::new(fin_reply_frame.payload()).unwrap();
    let fin_reply_tcp = TCPPacket::new(fin_reply_ip.payload()).unwrap();
    assert!(fin_reply_tcp.fin());
    assert!(fin_reply_tcp.ack());
    assert_eq!(fin_reply_tcp.acknowledgment(), Sequence::from(2007));

    // --- final ACK closes the connection ---
    let final_ack = build_tcp_frame(
        server_hw,
        client_hw,
        server_ip,
        client_ip,
        client_port,
        server_port,
        2007,
        Into::<u32>::into(fin_reply_tcp.sequence()) + 1,
        false,
        true,
        false,
        &[],
    );
    let final_ack_frame = EthernetFrame::new(final_ack.as_slice()).unwrap();
    let result = process_ethernet_frame(&ctx, &mut pool, &final_ack_frame).unwrap();
    assert!(matches!(result, ProcessingResult::Nothing));
}
