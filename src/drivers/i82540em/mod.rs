// https://www.intel.com/content/dam/doc/manual/pci-pci-x-family-gbe-controllers-software-dev-manual.pdf
// https://wiki.osdev.org/Intel_8254x

mod constants;
mod device;
mod rx;
mod tx;

pub use device::Device as I82540EMEthernetController;
use x86_64::instructions::hlt;

use crate::drivers::i82540em::rx::{RX_BUFFERS, RX_DESCS, setup_rx};
use crate::drivers::i82540em::tx::{send_packet, setup_tx};
use crate::memory::MemoryMapper;
use crate::net::arp::{ARP_PACKET, ARPOperation, ARPPacket, HardwareType, ProtocolType};
use crate::net::ethernet::address::EthernetAddress;
use crate::net::ethernet::ethertype::EtherType;
use crate::net::ethernet::{ETHERNET_HEADER, EthernetFrame};
use crate::net::l3_address::IPv4Address;
use crate::pci::{config_read_u32, config_write_u32, find_device};

const ID: u32 = 0x100e_8086;
const I8254_REG_CTRL: usize = 0x0;
const I8254_CTRL_ASDE: u32 = 1 << 5;
const I8254_CTRL_SLU: u32 = 1 << 6;
const I8254_CTRL_RESET: u32 = 1 << 26;
const I8254_REG_EERD: usize = 0x14;
const I8254_EERD_DONE: u32 = 1 << 4;
const I8254_REG_RAL: usize = 0x5400;
const I8254_REG_RAH: usize = 0x5404;

/// Bus Master Enable
const PCI_COMMAND_BME: u32 = 1 << 2;

pub fn find_and_setup_ethernet_controller(mapper: &MemoryMapper) {
    let Some((bus, device)) = find_device(ID) else {
        return;
    };

    setup_device(mapper, bus, device);
}

fn setup_device(mapper: &MemoryMapper, bus: u8, device: u8) {
    let bar0 = config_read_u32(bus, device, 0, 0x10);

    let eth_device = I82540EMEthernetController::from(mapper, bar0);

    let hwaddr = reset_nic(&eth_device);

    let command = config_read_u32(bus, device, 0, 0x04);
    config_write_u32(bus, device, 0, 0x04, command | PCI_COMMAND_BME);

    setup_rx(&eth_device, mapper);
    setup_tx(&eth_device, mapper);

    send_arp_request(&eth_device, mapper, hwaddr);

    send_packet(
        &eth_device,
        mapper,
        &[
            255, 255, 255, 255, 255, 255, 0, 17, 34, 51, 68, 85, 8, 6, 0, 1, 8, 0, 6, 4, 0, 1, 0,
            17, 34, 51, 68, 85, 10, 0, 2, 3, 0, 0, 0, 0, 0, 0, 10, 0, 2, 2,
        ],
    );

    for _ in 0..5 {
        hlt();
        hlt();
        hlt();
        hlt();
        hlt();
        unsafe {
            // kprintln!("{:#?}", *&raw const RX_DESCS);
            // kprintln!("{:?}", *&raw const RX_BUFFERS);
            if RX_DESCS[0].length != 0 {
                process_arp_reply(&RX_BUFFERS[0]);
            }
        }
    }

    // TODO: setup interrupts
    // TODO: send and receive packets
}

fn read_eeprom(device: &I82540EMEthernetController, address: u8) -> u16 {
    // TODO: lock with EECD before reading?

    let packet = (address as u32) << 8 | 1;
    device.write_register(I8254_REG_EERD, packet);

    loop {
        let result = device.read_register(I8254_REG_EERD);

        if result & I8254_EERD_DONE != 0 {
            return (result >> 16) as u16;
        }

        hlt();
    }
}

fn reset_nic(device: &I82540EMEthernetController) -> EthernetAddress {
    let mut device_control = device.read_register(I8254_REG_CTRL);
    device_control |= I8254_CTRL_RESET;
    device.write_register(I8254_REG_CTRL, device_control);

    while device.read_register(I8254_REG_CTRL) & I8254_CTRL_RESET != 0 {
        hlt();
    }

    let mut device_control = device.read_register(I8254_REG_CTRL);
    device_control |= I8254_CTRL_ASDE | I8254_CTRL_SLU;
    device.write_register(I8254_REG_CTRL, device_control);

    let b0 = read_eeprom(device, 0);
    let b1 = read_eeprom(device, 1);
    let b2 = read_eeprom(device, 2);

    let hwaddr = EthernetAddress::from_u16(b0, b1, b2);
    kprintln!("{hwaddr}");

    device.write_register(I8254_REG_RAL, (b1 as u32) << 16 | (b0 as u32));
    device.write_register(I8254_REG_RAH, b2 as u32 | /* Address valid */ (1 << 31));

    hwaddr
}

fn send_arp_request(
    device: &I82540EMEthernetController,
    mapper: &MemoryMapper,
    hwaddr: EthernetAddress,
) {
    kprintln!("ARP Request:");

    let mut packet = [0; ETHERNET_HEADER + ARP_PACKET];
    let mut frame = EthernetFrame::new(&mut packet).unwrap();

    frame
        .set_destination(EthernetAddress::BROADCAST)
        .set_source(hwaddr)
        .set_ethertype(EtherType::ARP);
    kprintln!("{}", frame);

    let mut arp = ARPPacket::new(frame.payload_mut()).unwrap();
    arp.set_hardware_type(HardwareType::Ethernet)
        .set_protocol_type(ProtocolType::IPv4)
        .set_hardware_length(EthernetAddress::SIZE as u8)
        .set_protocol_length(IPv4Address::SIZE as u8)
        .set_operation(ARPOperation::Request)
        .set_sender_hardware_address(hwaddr)
        .set_sender_protocol_address(IPv4Address::new(10, 0, 2, 3))
        .set_target_hardware_address(EthernetAddress::BROADCAST)
        .set_target_protocol_address(IPv4Address::new(10, 0, 2, 2));
    kprintln!("{}", arp);

    send_packet(device, mapper, frame.into_inner());
}

fn process_arp_reply(buffer: &[u8]) {
    kprintln!("ARP Reply:");
    let frame = EthernetFrame::new(buffer).unwrap();
    kprintln!("{}", frame);
    let arp = ARPPacket::new(frame.payload()).unwrap();
    kprintln!("{}", arp);
}
