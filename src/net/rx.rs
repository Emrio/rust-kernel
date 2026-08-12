extern crate alloc;

use alloc::vec::Vec;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_util::task::AtomicWaker;
use futures_util::{Stream, StreamExt};

use crate::drivers::i82540em::DEVICE;
use crate::net::arp::{ARPOperation, ARPPacket};
use crate::net::device::NetworkDevice;
use crate::net::error::BufferTooSmall;
use crate::net::ethernet::address::EthernetAddress;
use crate::net::ethernet::{EthernetFrame, ethertype::EtherType};
use crate::net::icmp::ICMPPacket;
use crate::net::ipv4::IPv4Packet;
use crate::net::ipv4::address::IPv4Address;
use crate::net::ipv4::protocol::Protocol;
use crate::net::{STATE_MACHINE, StateMachine, generate_arp_reply, generate_echo_reply};

pub(crate) static WAKER: AtomicWaker = AtomicWaker::new();

#[derive(Default, Debug)]
pub struct NetContext {
    ipv4_address: Option<IPv4Address>,
    hardware_address: Option<EthernetAddress>,
}

impl NetContext {
    pub fn from_device_and_state(
        device: Option<&impl NetworkDevice>,
        state: &StateMachine,
    ) -> Self {
        Self {
            ipv4_address: state.ipv4,
            hardware_address: device.map(|device| device.hardware_address()),
        }
    }

    pub fn ipv4_address(&self) -> Option<IPv4Address> {
        self.ipv4_address
    }

    pub fn hardware_address(&self) -> Option<EthernetAddress> {
        self.hardware_address
    }
}

pub enum ProcessingResult {
    Nothing,
    SetIpv4(IPv4Address),
    Respond(EthernetFrame<Vec<u8>>),
}

pub fn process_ethernet_frame(ctx: &NetContext, frame: &EthernetFrame<&[u8]>) -> ProcessingResult {
    match frame.ethertype() {
        EtherType::ARP => match ARPPacket::new(frame.payload()) {
            Ok(arp) => {
                kprintln!("ARP packet: {}", arp);

                if arp.operation() == ARPOperation::Reply
                    && ctx.ipv4_address().is_none()
                    && let Some(hardware_address) = ctx.hardware_address()
                    && arp.target_hardware_address() == hardware_address
                {
                    kprintln!("My IPv4: {}", arp.target_protocol_address());
                    return ProcessingResult::SetIpv4(arp.target_protocol_address());
                }

                if arp.operation() == ARPOperation::Request
                    && ctx.hardware_address().is_some()
                    && let Some(ipv4_address) = ctx.ipv4_address()
                    && ipv4_address == arp.target_protocol_address()
                {
                    kprintln!(
                        "-> {}/{} wants my hardware address!",
                        arp.sender_hardware_address(),
                        arp.sender_protocol_address()
                    );

                    return ProcessingResult::Respond(generate_arp_reply(ctx, frame, &arp));
                }

                ProcessingResult::Nothing
            }

            Err(BufferTooSmall) => {
                kprintln!("Error: ARP packet too small!");
                ProcessingResult::Nothing
            }
        },

        EtherType::IPv4 => match IPv4Packet::new(frame.payload()) {
            Ok(ipv4) => {
                kprintln!("IPv4 packet: {}", ipv4);

                match ipv4.protocol() {
                    Protocol::ICMP => match ICMPPacket::new(ipv4.payload()) {
                        Ok(icmp) => {
                            if icmp.is_echo_reply() {
                                kprintln!("Echo reply!");
                                return ProcessingResult::Nothing;
                            }

                            if icmp.is_echo_request() {
                                kprintln!("Echo request, generating response!");
                                return ProcessingResult::Respond(generate_echo_reply(
                                    ctx, frame, &ipv4, &icmp,
                                ));
                            }

                            kprintln!("ICMP packet is not echo: {}", icmp.icmp_type());
                            ProcessingResult::Nothing
                        }

                        Err(BufferTooSmall) => {
                            kprintln!("Error: ICMP packet too small!");
                            ProcessingResult::Nothing
                        }
                    },

                    Protocol::TCP => todo!(),

                    Protocol::UDP => todo!(),
                }
            }
            Err(BufferTooSmall) => {
                kprintln!("Error: IPv4 packet too small!");
                ProcessingResult::Nothing
            }
        },
    }
}

pub fn handle_incoming_ethernet_packet(buffer: &[u8]) {
    kprintln!("> Processing incoming packet...");

    let Ok(frame) = EthernetFrame::new(buffer) else {
        kprintln!(
            "Error: Couldn't parse incoming frame of size {}",
            buffer.len()
        );
        return;
    };

    let mut state = STATE_MACHINE.lock();
    let device = DEVICE.get();
    let context = NetContext::from_device_and_state(device, &state);
    match process_ethernet_frame(&context, &frame) {
        ProcessingResult::Nothing => {}
        ProcessingResult::SetIpv4(ipv4_address) => state.ipv4 = Some(ipv4_address),
        ProcessingResult::Respond(ethernet_frame) => {
            if let Some(device) = device {
                device.send_packet(&ethernet_frame.into_inner())
            } else {
                kprintln!("Error: I want to send a response but I don't have any device")
            }
        }
    }

    kprintln!("> OK");
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
