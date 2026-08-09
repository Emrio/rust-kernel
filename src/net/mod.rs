use core::time::Duration;

use crate::{
    drivers::i82540em::DEVICE,
    net::{
        arp::{ARP_PACKET, ARPOperation, ARPPacket, HardwareType, ProtocolType},
        device::NetworkDevice,
        ethernet::{
            ETHERNET_HEADER, EthernetFrame, address::EthernetAddress, ethertype::EtherType,
        },
        ipv4::address::IPv4Address,
    },
    time::{Instant, sleep},
};

pub mod arp;
pub mod device;
pub mod error;
pub mod ethernet;
pub mod ipv4;
pub(crate) mod rx;

pub fn send_arp_request(device: &impl NetworkDevice) {
    kprintln!("< ARP Request:");

    let mut packet = [0; ETHERNET_HEADER + ARP_PACKET];
    let mut frame = EthernetFrame::new(&mut packet).unwrap();

    frame
        .set_destination(EthernetAddress::BROADCAST)
        .set_source(device.hardware_address())
        .set_ethertype(EtherType::ARP);
    kprintln!("< {}", frame);

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
    kprintln!("< {}", arp);

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
