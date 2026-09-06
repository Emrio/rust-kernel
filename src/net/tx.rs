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
use crate::net::icmp::{ICMP_PACKET, ICMPPacket};
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
) -> Result<Vec<u8>, BufferTooSmall> {
    build(L2::Ethernet {
        source: ctx.hardware_address(),
        destination: request_frame.source(),
        ethertype: EtherType::IPv4,
        next: L3::IPv4 {
            source: ctx.ipv4_address().unwrap_or(request_ipv4.destination()),
            destination: request_ipv4.source(),
            protocol: Protocol::ICMP,
            next: L4::IcmpEcho {
                code: 0,
                icmp_type: IcmpType::EchoReply,
                next: L7::Buffer(request_echo.payload().to_vec()),
            },
        },
    })
}

pub fn generate_pong_udp_packet(
    ctx: &NetContext,
    request_frame: &EthernetFrame<&[u8]>,
    request_ipv4: &IPv4Packet<&[u8]>,
    request_udp: &UDPPacket<&[u8]>,
) -> Result<Vec<u8>, BufferTooSmall> {
    const PREFIX: &str = "Pong: ";
    let mut payload = vec![0u8; request_udp.payload().len() + PREFIX.len()];
    payload[0..PREFIX.len()].copy_from_slice(PREFIX.as_bytes());
    payload[PREFIX.len()..].copy_from_slice(request_udp.payload());

    build(L2::Ethernet {
        source: ctx.hardware_address(),
        destination: request_frame.source(),
        ethertype: EtherType::IPv4,
        next: L3::IPv4 {
            source: ctx.ipv4_address().unwrap_or(request_ipv4.destination()),
            destination: request_ipv4.source(),
            protocol: Protocol::UDP,
            next: L4::Udp {
                source: request_udp.destination(),
                destination: request_udp.source(),
                next: L7::Buffer(payload),
            },
        },
    })
}

enum L2 {
    Ethernet {
        source: EthernetAddress,
        destination: EthernetAddress,
        ethertype: EtherType,
        next: L3,
    },
}

impl L2 {
    fn size(&self) -> usize {
        match self {
            Self::Ethernet { next, .. } => ETHERNET_HEADER + next.size(),
        }
    }
}

enum L2Frame<'a> {
    Ethernet(EthernetFrame<&'a mut [u8]>),
}

impl<'a> L2Frame<'a> {
    fn payload_mut(&mut self) -> &mut [u8] {
        match self {
            Self::Ethernet(frame) => frame.payload_mut(),
        }
    }
}

enum L3 {
    IPv4 {
        source: IPv4Address,
        destination: IPv4Address,
        protocol: Protocol,
        next: L4,
    },
}

impl L3 {
    fn size(&self) -> usize {
        match self {
            Self::IPv4 { next, .. } => IPV4_PACKET + next.size(),
        }
    }
}

enum L3Frame<'a> {
    IPv4(IPv4Packet<&'a mut [u8]>),
}

impl<'a> L3Frame<'a> {
    fn payload_mut(&mut self) -> &mut [u8] {
        match self {
            Self::IPv4(frame) => frame.payload_mut(),
        }
    }
}

enum L4 {
    Udp {
        source: u16,
        destination: u16,
        next: L7,
    },
    IcmpEcho {
        code: u8,
        icmp_type: IcmpType,
        next: L7,
    },
}

impl L4 {
    fn size(&self) -> usize {
        match self {
            Self::Udp { next, .. } => UDP_HEADER + next.size(),
            Self::IcmpEcho { next, .. } => ICMP_PACKET + next.size(),
        }
    }
}

enum L4Frame<'a> {
    Udp(UDPPacket<&'a mut [u8]>),
    Icmp(ICMPPacket<&'a mut [u8]>),
}

impl<'a> L4Frame<'a> {
    fn payload_mut(&mut self) -> &mut [u8] {
        match self {
            Self::Udp(frame) => frame.payload_mut(),
            Self::Icmp(frame) => frame.payload_mut(),
        }
    }
}

enum L7 {
    Buffer(Vec<u8>),
}

impl L7 {
    fn size(&self) -> usize {
        match self {
            Self::Buffer(buffer) => buffer.len(),
        }
    }
}

fn build(l2: L2) -> Result<Vec<u8>, BufferTooSmall> {
    let packet_size = l2.size();
    let mut packet = vec![0u8; packet_size];

    let (mut l2frame, l3) = match l2 {
        L2::Ethernet {
            source,
            destination,
            ethertype,
            next,
        } => {
            let mut frame = EthernetFrame::new(packet.as_mut_slice())?;
            frame
                .set_destination(destination)
                .set_source(source)
                .set_ethertype(ethertype);
            (L2Frame::Ethernet(frame), next)
        }
    };

    let l3size = l3.size();
    let (mut l3frame, l4) = match l3 {
        L3::IPv4 {
            source,
            destination,
            protocol,
            next,
        } => {
            let mut ipv4 = IPv4Packet::new(l2frame.payload_mut())?;
            ipv4.set_version_and_length()
                .set_packet_length(l3size)
                .set_protocol(protocol)
                .set_destination(destination)
                .set_source(source)
                .set_ttl(TimeToLive::max())
                .compute_checksum();
            (L3Frame::IPv4(ipv4), next)
        }
    };

    let l4size = l4.size();
    let (mut l4frame, l7) = match l4 {
        L4::Udp {
            source,
            destination,
            next,
        } => {
            let mut udp = UDPPacket::new(l3frame.payload_mut())?;
            udp.set_source(source)
                .set_destination(destination)
                .set_length(l4size);
            (L4Frame::Udp(udp), next)
        }
        L4::IcmpEcho {
            code,
            icmp_type,
            next,
        } => {
            let mut icmp = ICMPPacket::new(l3frame.payload_mut())?;
            icmp.set_code(code).set_icmp_type(icmp_type);

            (L4Frame::Icmp(icmp), next)
        }
    };

    match l7 {
        L7::Buffer(buffer) => l4frame.payload_mut().copy_from_slice(&buffer),
    }

    if let L4Frame::Icmp(mut icmppacket) = l4frame {
        icmppacket.compute_checksum();
    };

    Ok(packet)
}
