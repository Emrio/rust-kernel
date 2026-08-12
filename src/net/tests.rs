use super::*;

#[test_case]
fn icmp_echo_request_is_met_with_reply() {
    let mut packet = [0; ETHERNET_HEADER + IPV4_PACKET + ECHO_PACKET];
    let mut frame = EthernetFrame::new(&mut packet).unwrap();
    frame
        .set_destination(EthernetAddress::from_bytes(&[1, 2, 3, 4, 5, 6]))
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

    let rx::ProcessingResult::Respond(response) =
        rx::process_ethernet_frame(&NetContext::default(), &frame)
    else {
        panic!("Expected response")
    };

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

    let ctx = NetContext::from_addresses(Some(target_hw), Some(target_ip));
    let rx::ProcessingResult::Respond(response) = rx::process_ethernet_frame(&ctx, &frame) else {
        panic!("Expected response")
    };

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
