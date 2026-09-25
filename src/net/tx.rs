extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::drivers::i82540em::DEVICE;
use crate::net::arp::{
    ARP_PACKET, ARPOperation, ARPPacket, ARPResolutionError, HardwareType, ProtocolType,
    ResolveStart,
};
use crate::net::device::NetworkDevice;
use crate::net::dhcp::option::DHCPOption;
use crate::net::dhcp::{self, DHCP_HEADER, DHCPPacket};
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
use crate::net::tcp::sequence::Sequence;
use crate::net::tcp::{TCP_HEADER, TCPPacket};
use crate::net::udp::{UDP_HEADER, UDPPacket};
use crate::net::{DHCPConfiguration, STATE_MACHINE};
use crate::random::random_u32;

pub fn generate_echo_reply(
    request_ipv4: &IPv4Packet<&[u8]>,
    request_echo: &ICMPPacket<&[u8]>,
) -> L3 {
    L3::IPv4 {
        source: request_ipv4.destination(),
        destination: request_ipv4.source(),
        protocol: Protocol::ICMP,
        next: L4::IcmpEcho {
            code: 0,
            icmp_type: IcmpType::EchoReply,
            next: L7::Buffer(request_echo.payload().to_vec()),
        },
    }
}

pub fn generate_dhcp_discover(ctx: &NetContext) -> L3 {
    L3::IPv4 {
        source: IPv4Address::default(),
        destination: IPv4Address::BROADCAST,
        protocol: Protocol::UDP,
        next: L4::Udp {
            source: dhcp::ports::CLIENT,
            destination: dhcp::ports::SERVER,
            next: L7::Dhcp {
                operation: dhcp::operation::Operation::BootRequest,
                xid: random_u32(),
                hardware_address: ctx.hardware_address(),
                options: vec![
                    DHCPOption::MessageType(dhcp::option::MessageType::Discover),
                    DHCPOption::ParameterRequestList(vec![
                        dhcp::option::ParameterRequest::SubnetMask,
                        dhcp::option::ParameterRequest::Router,
                        dhcp::option::ParameterRequest::DomainNameServer,
                    ]),
                    DHCPOption::Hostname("RustKernel".into()),
                    DHCPOption::End,
                ],
            },
        },
    }
}

pub fn generate_dhcp_request(
    ctx: &NetContext,
    // request_frame: &EthernetFrame<&[u8]>,
    // request_ipv4: &IPv4Packet<&[u8]>,
    // request_udp: &UDPPacket<&[u8]>,
    dhcp_offer: &DHCPPacket<&[u8]>,
) -> L3 {
    let mut options = vec![
        DHCPOption::MessageType(dhcp::option::MessageType::Request),
        DHCPOption::RequestedAddress(dhcp_offer.your_address()),
        DHCPOption::Hostname("RustKernel".into()),
    ];
    if let Some(id) = dhcp_offer.options().get_server_identifier() {
        options.push(DHCPOption::ServerIdentifier(id));
    }
    options.push(DHCPOption::End);

    L3::IPv4 {
        source: IPv4Address::default(),
        destination: IPv4Address::BROADCAST,
        protocol: Protocol::UDP,
        next: L4::Udp {
            source: dhcp::ports::CLIENT,
            destination: dhcp::ports::SERVER,
            next: L7::Dhcp {
                operation: dhcp::operation::Operation::BootRequest,
                xid: dhcp_offer.xid(),
                hardware_address: ctx.hardware_address(),
                options,
            },
        },
    }
}

pub fn generate_dhcp_request_with_configuration(
    ctx: &NetContext,
    xid: u32,
    configuration: &DHCPConfiguration,
) -> L3 {
    let mut options = vec![
        DHCPOption::MessageType(dhcp::option::MessageType::Request),
        DHCPOption::RequestedAddress(configuration.ipv4),
        DHCPOption::Hostname("RustKernel".into()),
    ];
    // if let Some(id) = dhcp_offer.options().get_server_identifier() {
    //     options.push(DHCPOption::ServerIdentifier(id));
    // }
    options.push(DHCPOption::End);

    L3::IPv4 {
        source: IPv4Address::default(),
        destination: IPv4Address::BROADCAST,
        protocol: Protocol::UDP,
        next: L4::Udp {
            source: dhcp::ports::CLIENT,
            destination: dhcp::ports::SERVER,
            next: L7::Dhcp {
                operation: dhcp::operation::Operation::BootRequest,
                xid,
                hardware_address: ctx.hardware_address(),
                options,
            },
        },
    }
}

pub fn generate_ipv4(request_ipv4: &IPv4Packet<&[u8]>, protocol: Protocol, payload: Vec<u8>) -> L3 {
    L3::IPv4 {
        source: request_ipv4.destination(),
        destination: request_ipv4.source(),
        protocol,
        next: L4::Buffer(payload),
    }
}

pub(crate) enum L2 {
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

pub(crate) enum L3 {
    IPv4 {
        source: IPv4Address,
        destination: IPv4Address,
        protocol: Protocol,
        next: L4,
    },
    Arp {
        operation: ARPOperation,
        sender_hardware_address: EthernetAddress,
        sender_protocol_address: IPv4Address,
        target_hardware_address: EthernetAddress,
        target_protocol_address: IPv4Address,
    },
}

impl L3 {
    fn size(&self) -> usize {
        match self {
            Self::IPv4 { next, .. } => IPV4_PACKET + next.size(),
            Self::Arp { .. } => ARP_PACKET,
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

    fn source(&self) -> IPv4Address {
        let L3Frame::IPv4(ipv4_packet) = self;
        ipv4_packet.source()
    }

    fn destination(&self) -> IPv4Address {
        let L3Frame::IPv4(ipv4_packet) = self;
        ipv4_packet.destination()
    }
}

pub(crate) enum L4 {
    Buffer(Vec<u8>),
    Udp {
        source: u16,
        destination: u16,
        next: L7,
    },
    Tcp {
        source: u16,
        destination: u16,
        sequence: Sequence,
        acknowledgment: Sequence,
        cwr: bool,
        ece: bool,
        urg: bool,
        ack: bool,
        psh: bool,
        rst: bool,
        syn: bool,
        fin: bool,
        window: u16,
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
            Self::Buffer(buffer) => buffer.len(),
            Self::Udp { next, .. } => UDP_HEADER + next.size(),
            Self::Tcp { next, .. } => TCP_HEADER + next.size(),
            Self::IcmpEcho { next, .. } => ICMP_PACKET + next.size(),
        }
    }
}

enum L4Frame<'a> {
    Udp(UDPPacket<&'a mut [u8]>),
    Tcp(TCPPacket<&'a mut [u8]>),
    Icmp(ICMPPacket<&'a mut [u8]>),
}

impl<'a> L4Frame<'a> {
    fn payload_mut(&mut self) -> &mut [u8] {
        match self {
            Self::Udp(frame) => frame.payload_mut(),
            Self::Tcp(frame) => frame.payload_mut(),
            Self::Icmp(frame) => frame.payload_mut(),
        }
    }
}

pub(crate) enum L7 {
    Buffer(Vec<u8>),
    Dhcp {
        operation: dhcp::operation::Operation,
        xid: u32,
        hardware_address: EthernetAddress,
        options: Vec<DHCPOption>,
    },
}

impl L7 {
    fn size(&self) -> usize {
        match self {
            Self::Buffer(buffer) => buffer.len(),
            Self::Dhcp { options, .. } => DHCP_HEADER + dhcp::option::options_to_vec(options).len(),
        }
    }
}

pub(crate) fn build(l2: L2) -> Result<Vec<u8>, BufferTooSmall> {
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

    let l3size: usize = l3.size();
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
        L3::Arp {
            operation,
            sender_hardware_address,
            sender_protocol_address,
            target_hardware_address,
            target_protocol_address,
        } => {
            let mut arp = ARPPacket::new(l2frame.payload_mut()).unwrap();
            arp.set_hardware_type(HardwareType::Ethernet)
                .set_protocol_type(ProtocolType::IPv4)
                .set_hardware_length(EthernetAddress::SIZE as u8)
                .set_protocol_length(IPv4Address::SIZE as u8)
                .set_operation(operation)
                .set_sender_hardware_address(sender_hardware_address)
                .set_sender_protocol_address(sender_protocol_address)
                .set_target_hardware_address(target_hardware_address)
                .set_target_protocol_address(target_protocol_address);
            return Ok(packet);
        }
    };

    let l4size = l4.size();
    let (mut l4frame, l7) = match l4 {
        L4::Buffer(buffer) => {
            l3frame.payload_mut().copy_from_slice(&buffer);
            return Ok(packet);
        }
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
        L4::Tcp {
            source,
            destination,
            sequence,
            acknowledgment,
            cwr,
            ece,
            urg,
            ack,
            psh,
            rst,
            syn,
            fin,
            window,
            next,
        } => {
            let l3source = l3frame.source();
            let l3destination = l3frame.destination();
            let mut tcp = TCPPacket::new(l3frame.payload_mut())?;
            tcp.set_source(source)
                .set_destination(destination)
                .set_sequence(sequence)
                .set_acknowledgment(acknowledgment)
                .set_cwr(cwr)
                .set_ece(ece)
                .set_urg(urg)
                .set_ack(ack)
                .set_psh(psh)
                .set_rst(rst)
                .set_syn(syn)
                .set_fin(fin)
                .set_window(window)
                .set_data_offset_and_reserved()
                .compute_checksum(l3source, l3destination);
            (L4Frame::Tcp(tcp), next)
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
        L7::Dhcp {
            operation,
            xid,
            hardware_address,
            options,
        } => {
            let mut dhcp = DHCPPacket::new(l4frame.payload_mut())?;

            dhcp.set_operation(operation)
                .set_hardware_type(HardwareType::Ethernet)
                .set_hardware_length(EthernetAddress::SIZE)
                .set_xid(xid)
                .set_hardware_address(hardware_address)
                .set_magic_cookie();

            let mut dhcp_options = dhcp.options_mut();
            for option in options {
                dhcp_options.append(option)?;
            }
        }
    }

    if let L4Frame::Icmp(mut icmppacket) = l4frame {
        icmppacket.compute_checksum();
    };

    Ok(packet)
}

pub(super) fn send_l2(l2: L2) -> Result<(), NetworkError> {
    let device = DEVICE.get().expect("device is not ready");
    device.send_packet(&build(l2).expect("buffer too small"));
    Ok(())
}

#[derive(Debug)]
pub enum NetworkError {
    Arp(ARPResolutionError),
}

pub(super) async fn send_l3(l3: L3) -> Result<(), NetworkError> {
    let device = DEVICE.get().expect("device is not ready");
    match &l3 {
        L3::IPv4 { destination, .. } => send_l2(L2::Ethernet {
            source: device.hardware_address(),
            destination: match *destination {
                IPv4Address::BROADCAST => EthernetAddress::BROADCAST,
                destination => arp_resolve(destination).await.map_err(NetworkError::Arp)?,
            },
            ethertype: EtherType::IPv4,
            next: l3,
        }),
        L3::Arp {
            operation,
            target_hardware_address,
            ..
        } => send_l2(L2::Ethernet {
            source: device.hardware_address(),
            destination: match operation {
                ARPOperation::Reply => *target_hardware_address,
                ARPOperation::Request => EthernetAddress::BROADCAST,
            },
            ethertype: EtherType::ARP,
            next: l3,
        }),
    }
}

async fn arp_resolve(ipv4_address: IPv4Address) -> Result<EthernetAddress, ARPResolutionError> {
    let start = {
        let mut state = STATE_MACHINE.lock();
        state.arp.resolve_start(ipv4_address)?
    };

    match start {
        ResolveStart::Known(hardware_address) => Ok(hardware_address),
        ResolveStart::Pending(resolution) => {
            let hardware_address = resolution.await?;
            STATE_MACHINE
                .lock()
                .arp
                .resolve_finish(ipv4_address, hardware_address);
            Ok(hardware_address)
        }
    }
}
