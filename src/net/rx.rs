extern crate alloc;

use alloc::vec::Vec;
use core::pin::Pin;
use core::task::{Context, Poll};

use futures_util::task::AtomicWaker;
use futures_util::{Stream, StreamExt};

use crate::drivers::i82540em::DEVICE;
use crate::net::arp::ARPPacket;
use crate::net::device::NetworkDevice;
use crate::net::error::BufferTooSmall;
use crate::net::ethernet::{EthernetFrame, ethertype::EtherType};
use crate::net::ipv4::IPv4Packet;

pub(crate) static WAKER: AtomicWaker = AtomicWaker::new();

fn handle_incoming_ethernet_packet(buffer: &[u8]) {
    kprintln!("> Processing incoming packet...");

    let Ok(frame) = EthernetFrame::new(buffer) else {
        kprintln!(
            "Error: Couldn't parse incoming frame of size {}",
            buffer.len()
        );
        return;
    };

    match frame.ethertype() {
        EtherType::ARP => match ARPPacket::new(frame.payload()) {
            Ok(arp) => kprintln!("ARP packet: {}", arp),
            Err(BufferTooSmall) => kprintln!("Error: ARP packet too small!"),
        },
        EtherType::IPv4 => match IPv4Packet::new(frame.payload()) {
            Ok(ipv4) => kprintln!("IPv4 packet: {}", ipv4),
            Err(BufferTooSmall) => kprintln!("Error: IPv4 packet too small!"),
        },
    };

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
