extern crate alloc;

use alloc::vec::Vec;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_util::task::AtomicWaker;
use futures_util::{Stream, StreamExt};

use crate::drivers::i82540em::DEVICE;
use crate::net::arp::{ARPCache, ARPMessage, ARPPacket};
use crate::net::device::NetworkDevice;
use crate::net::dhcp::DHCPPacket;
use crate::net::error::BufferTooSmall;
use crate::net::ethernet::address::EthernetAddress;
use crate::net::ethernet::{EthernetFrame, ethertype::EtherType};
use crate::net::icmp::ICMPPacket;
use crate::net::ipv4::IPv4Packet;
use crate::net::ipv4::address::IPv4Address;
use crate::net::ipv4::protocol::Protocol;
use crate::net::socket::{TCPConnectionPool, UDPMessage};
use crate::net::tcp::TCPPacket;
use crate::net::tx::{self, L3, NetworkError, generate_echo_reply, send_l3};
use crate::net::udp::UDPPacket;
use crate::net::{DHCPConfiguration, DHCPStateMachine, STATE_MACHINE, StateMachine, dhcp};
use crate::print::colors::Colorable;
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

pub(crate) enum ProcessingResult {
    Nothing,
    DHCPReset,
    DHCPOffered(L3, u32, DHCPConfiguration),
    DHCPAccepted(DHCPConfiguration),
    PushUdpMessage(UDPMessage),
    PushArpMessage(ARPMessage),
    Respond(L3),
}

pub(crate) fn process_ethernet_frame(
    ctx: &NetContext,
    tcp_pool: &mut TCPConnectionPool,
    arp_cache: &mut ARPCache,
    frame: &EthernetFrame<&[u8]>,
) -> Result<ProcessingResult, BufferTooSmall> {
    // klog!("net_rx", "Ethernet frame: ", frame);

    if !frame.destination().is_broadcast() && frame.destination() != ctx.hardware_address() {
        // klog!("net_rx", "This frame is not for me.");
        return Ok(ProcessingResult::Nothing);
    }

    match frame.ethertype() {
        EtherType::ARP => {
            let arp = ARPPacket::new(frame.payload())?;

            // klog!("net_rx", "ARP packet: ", arp);

            Ok(ProcessingResult::PushArpMessage(arp.into()))
        }

        EtherType::IPv4 => {
            let ipv4 = IPv4Packet::new(frame.payload())?;
            // klog!("net_rx", "IPv4 packet: ", ipv4);

            // TODO: il faut pas ajouter les ip qui ne sont pas dans le netmask
            arp_cache.add_entry(ipv4.source(), frame.source());

            if let Some(ipv4_address) = ctx.ipv4_address()
                && ipv4_address != ipv4.destination()
                && ipv4.destination() != IPv4Address::BROADCAST
            {
                // klog!("net_rx", "IP packet is not for me");
                return Ok(ProcessingResult::Nothing);
            }

            match ipv4.protocol() {
                Protocol::ICMP => {
                    let icmp = ICMPPacket::new(ipv4.payload())?;

                    // klog!("net_rx", "ICMP packet: ", icmp);

                    if icmp.is_echo_request() {
                        klog!("net_rx", "Echo request, generating response!");
                        return Ok(ProcessingResult::Respond(generate_echo_reply(&ipv4, &icmp)));
                    }

                    klog!("net_rx", "ICMP packet is not echo request");
                    Ok(ProcessingResult::Nothing)
                }

                Protocol::TCP => {
                    let tcp = TCPPacket::new(ipv4.payload())?;
                    klog!("net_rx", "TCP packet: ", tcp);

                    if let Some(response) = tcp_pool.accept(&ipv4, &tcp) {
                        Ok(ProcessingResult::Respond(tx::generate_ipv4(
                            &ipv4,
                            Protocol::TCP,
                            response,
                        )))
                    } else {
                        Ok(ProcessingResult::Nothing)
                    }
                }

                Protocol::UDP => {
                    let udp = UDPPacket::new(ipv4.payload())?;
                    klog!("net_rx", "UDP packet: ", udp);

                    if udp.source() == dhcp::ports::SERVER
                        && udp.destination() == dhcp::ports::CLIENT
                        && ctx.ipv4_address.is_none()
                    {
                        let dhcp = DHCPPacket::new(udp.payload())?;
                        klog!("net_rx", "DHCP packet: ", dhcp);

                        let message_type = dhcp.options().get_message_type();

                        if dhcp.your_address().is_broadcast() || dhcp.your_address().is_zero() {
                            return Ok(ProcessingResult::DHCPReset);
                        }

                        let Some(invalid_at) = dhcp
                            .options()
                            .get_lease_time()
                            .map(|lease_time| Instant::now() + lease_time)
                        else {
                            return Ok(ProcessingResult::DHCPReset);
                        };
                        let configuration = DHCPConfiguration {
                            invalid_at,
                            ipv4: dhcp.your_address(),
                            netmask: dhcp.options().get_mask(),
                            router: dhcp.options().get_router(),
                            dns: dhcp.options().get_dns(),
                        };

                        return match message_type {
                            Some(dhcp::option::MessageType::Offer) => {
                                Ok(ProcessingResult::DHCPOffered(
                                    tx::generate_dhcp_request(ctx, &dhcp),
                                    dhcp.xid(),
                                    configuration,
                                ))
                            }
                            Some(dhcp::option::MessageType::Ack) => {
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

                    let message = UDPMessage::new(&ipv4, &udp);
                    Ok(ProcessingResult::PushUdpMessage(message))
                }
            }
        }
    }
}

pub async fn handle_incoming_ethernet_packet(buffer: &[u8]) -> Result<(), NetworkError> {
    // kprintln!("-> {:02x?} (size: {})", buffer, buffer.len());

    let Ok(frame) = EthernetFrame::new(buffer) else {
        klog!(
            "net_rx",
            "Error".red(),
            ": Couldn't parse incoming frame of size",
            buffer.len().yellow()
        );
        return Ok(());
    };

    let mut state = STATE_MACHINE.lock();
    let device = DEVICE.get().expect("device to be ready");
    let context = NetContext::from_device_and_state(device, &state);
    let StateMachine { tcp, arp, .. } = &mut *state;
    let processing_result = process_ethernet_frame(&context, tcp, arp, &frame);
    drop(state);
    match processing_result {
        Ok(ProcessingResult::Nothing) => {}
        Ok(ProcessingResult::DHCPReset) => {
            let mut state = STATE_MACHINE.lock();
            state.dhcp = DHCPStateMachine::Unconfigured(Instant::now())
        }
        Ok(ProcessingResult::DHCPOffered(l3, xid, configuration)) => {
            send_l3(l3).await?;
            let mut state = STATE_MACHINE.lock();
            state.dhcp =
                DHCPStateMachine::Offered(Instant::now(), Instant::now(), xid, configuration);
        }
        Ok(ProcessingResult::DHCPAccepted(configuration)) => {
            klog!(
                "dhcp",
                "Now configured: ",
                configuration.ipv4.bright_green()
            );
            let mut state = STATE_MACHINE.lock();
            state.dhcp = DHCPStateMachine::Assigned(configuration);
            state.arp.identity = Some((device.hardware_address(), configuration.ipv4));
        }
        Ok(ProcessingResult::Respond(l3)) => send_l3(l3).await?,
        Ok(ProcessingResult::PushUdpMessage(message)) => {
            let state = STATE_MACHINE.lock();
            state.udp.accept(message)
        }
        Ok(ProcessingResult::PushArpMessage(message)) => {
            let mut state = STATE_MACHINE.lock();
            state.arp.accept(message).await
        }
        Err(BufferTooSmall) => {
            klog!(
                "net_rx",
                "Error".red(),
                ": Could not decode packet: the packet is too small",
            );
        }
    }
    Ok(())
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
        if let Err(err) = handle_incoming_ethernet_packet(&packet).await {
            kprintln!("Network error: {err:?}")
        }
    }
}
