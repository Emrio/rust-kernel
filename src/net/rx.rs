extern crate alloc;

use alloc::vec::Vec;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_util::task::AtomicWaker;
use futures_util::{Stream, StreamExt};

use crate::drivers::i82540em::DEVICE;
use crate::net::arp::{ARPOperation, ARPPacket};
use crate::net::device::NetworkDevice;
use crate::net::dhcp::DHCPPacket;
use crate::net::error::BufferTooSmall;
use crate::net::ethernet::address::EthernetAddress;
use crate::net::ethernet::{EthernetFrame, ethertype::EtherType};
use crate::net::icmp::ICMPPacket;
use crate::net::ipv4::IPv4Packet;
use crate::net::ipv4::address::IPv4Address;
use crate::net::ipv4::protocol::Protocol;
use crate::net::tx::{self, generate_arp_reply, generate_echo_reply, generate_pong_udp_packet};
use crate::net::udp::UDPPacket;
use crate::net::{DHCPConfiguration, DHCPStateMachine, STATE_MACHINE, StateMachine, dhcp};
use crate::time::Instant;

pub(crate) static WAKER: AtomicWaker = AtomicWaker::new();

#[derive(Debug)]
pub struct NetContext {
    ipv4_address: Option<IPv4Address>,
    hardware_address: EthernetAddress,
}

impl NetContext {
    pub fn from_device_and_state(device: &impl NetworkDevice, state: &StateMachine) -> Self {
        Self {
            ipv4_address: state.configuration().map(|conf| conf.ipv4),
            hardware_address: device.hardware_address(),
        }
    }

    pub fn from_addresses(hardware_address: EthernetAddress, ipv4_address: IPv4Address) -> Self {
        Self {
            ipv4_address: Some(ipv4_address),
            hardware_address,
        }
    }

    pub fn from_hardware_address(hardware_address: EthernetAddress) -> Self {
        Self {
            ipv4_address: None,
            hardware_address,
        }
    }

    pub fn ipv4_address(&self) -> Option<IPv4Address> {
        self.ipv4_address
    }

    pub fn hardware_address(&self) -> EthernetAddress {
        self.hardware_address
    }
}

pub enum ProcessingResult {
    Nothing,
    DHCPReset,
    DHCPOffered,
    DHCPAccepted(DHCPConfiguration),
    Respond(Vec<u8>),
}

pub fn process_ethernet_frame(
    ctx: &NetContext,
    frame: &EthernetFrame<&[u8]>,
) -> Result<ProcessingResult, BufferTooSmall> {
    kprintln!("-> Ethernet frame: {}", frame);

    if !frame.destination().is_broadcast() && frame.destination() != ctx.hardware_address() {
        kprintln!("-> This frame is not for me.");
        return Ok(ProcessingResult::Nothing);
    }

    match frame.ethertype() {
        EtherType::ARP => {
            let arp = ARPPacket::new(frame.payload())?;

            kprintln!("-> ARP packet: {}", arp);

            // if arp.operation() == ARPOperation::Reply
            //     && ctx.ipv4_address().is_none()
            //     && arp.target_hardware_address() == ctx.hardware_address()
            // {
            //     kprintln!("-> My IPv4: {}", arp.target_protocol_address());
            //     return Ok(ProcessingResult::SetIpv4(arp.target_protocol_address()));
            // }

            if arp.operation() == ARPOperation::Request
                && let Some(ipv4_address) = ctx.ipv4_address()
                && ipv4_address == arp.target_protocol_address()
            {
                kprintln!(
                    "-> {}/{} wants my hardware address!",
                    arp.sender_hardware_address(),
                    arp.sender_protocol_address()
                );

                return Ok(ProcessingResult::Respond(generate_arp_reply(
                    ctx, frame, &arp,
                )?));
            }

            Ok(ProcessingResult::Nothing)
        }

        EtherType::IPv4 => {
            let ipv4 = IPv4Packet::new(frame.payload())?;
            kprintln!("-> IPv4 packet: {}", ipv4);

            if let Some(ipv4_address) = ctx.ipv4_address()
                && ipv4_address != ipv4.destination()
            {
                kprintln!("-> IP packet is not for me");
                return Ok(ProcessingResult::Nothing);
            }

            match ipv4.protocol() {
                Protocol::ICMP => {
                    let icmp = ICMPPacket::new(ipv4.payload())?;

                    kprintln!("-> ICMP packet: {}", icmp);

                    if icmp.is_echo_request() {
                        kprintln!("-> Echo request, generating response!");
                        return Ok(ProcessingResult::Respond(generate_echo_reply(
                            ctx, frame, &ipv4, &icmp,
                        )?));
                    }

                    kprintln!("-> ICMP packet is not echo request");
                    Ok(ProcessingResult::Nothing)
                }

                Protocol::TCP => todo!(),

                Protocol::UDP => {
                    let udp = UDPPacket::new(ipv4.payload())?;
                    kprintln!("-> UDP packet: {}", udp);

                    if udp.source() == dhcp::ports::SERVER
                        && udp.destination() == dhcp::ports::CLIENT
                        && ctx.ipv4_address.is_none()
                    {
                        let dhcp = DHCPPacket::new(udp.payload())?;
                        kprintln!("-> DHCP packet: {}", dhcp);

                        let message_type = dhcp.options().get_message_type();

                        return match message_type {
                            Some(dhcp::option::MessageType::Offer) => Ok(
                                ProcessingResult::Respond(tx::generate_dhcp_request(ctx, &dhcp)?),
                            ),
                            Some(dhcp::option::MessageType::Ack) => {
                                let Some(invalid_at) = dhcp
                                    .options()
                                    .get_lease_time()
                                    .map(|lease_time| Instant::now() + lease_time)
                                else {
                                    return Ok(ProcessingResult::DHCPReset);
                                };

                                if dhcp.your_address().is_broadcast()
                                    || dhcp.your_address().is_zero()
                                {
                                    return Ok(ProcessingResult::DHCPReset);
                                }

                                Ok(ProcessingResult::DHCPAccepted(DHCPConfiguration {
                                    invalid_at,
                                    ipv4: dhcp.your_address(),
                                    netmask: dhcp.options().get_mask(),
                                    router: dhcp.options().get_router(),
                                    dns: dhcp.options().get_dns(),
                                }))
                            }
                            _ => Ok(ProcessingResult::Nothing),
                        };
                    }

                    Ok(ProcessingResult::Respond(generate_pong_udp_packet(
                        ctx, frame, &ipv4, &udp,
                    )?))
                }
            }
        }
    }
}

pub fn handle_incoming_ethernet_packet(buffer: &[u8]) {
    kprintln!("-> Processing incoming packet...");
    // kprintln!("-> {:02x?} (size: {})", buffer, buffer.len());

    let Ok(frame) = EthernetFrame::new(buffer) else {
        kprintln!(
            "-> Error: Couldn't parse incoming frame of size {}",
            buffer.len()
        );
        return;
    };

    let mut state = STATE_MACHINE.lock();
    let device = DEVICE.get().expect("device to be ready");
    let context = NetContext::from_device_and_state(device, &state);
    match process_ethernet_frame(&context, &frame) {
        Ok(ProcessingResult::Nothing) => {}
        Ok(ProcessingResult::DHCPReset) => {
            state.dhcp = DHCPStateMachine::Unconfigured(Instant::now())
        }
        Ok(ProcessingResult::DHCPOffered) => state.dhcp = DHCPStateMachine::Offered(Instant::now()),
        Ok(ProcessingResult::DHCPAccepted(configuration)) => {
            state.dhcp = DHCPStateMachine::Assigned(configuration)
        }
        Ok(ProcessingResult::Respond(buffer)) => device.send_packet(&buffer),
        Err(BufferTooSmall) => {
            kprintln!("-> Err: Could not decode packet: the packet is too small")
        }
    }
}

struct RxStream;

impl Stream for RxStream {
    type Item = Vec<u8>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Some(device) = DEVICE.get() else {
            return Poll::Ready(None);
        };

        if let Some(buffer) = device.poll_packet() {
            return Poll::Ready(Some(buffer));
        }

        WAKER.register(cx.waker());

        if let Some(buffer) = device.poll_packet() {
            return Poll::Ready(Some(buffer));
        }

        Poll::Pending
    }
}

pub async fn rx_loop() {
    let mut stream = RxStream;

    while let Some(packet) = stream.next().await {
        handle_incoming_ethernet_packet(&packet);
    }
}
