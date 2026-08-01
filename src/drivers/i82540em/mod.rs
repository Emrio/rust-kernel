// https://www.intel.com/content/dam/doc/manual/pci-pci-x-family-gbe-controllers-software-dev-manual.pdf
// https://wiki.osdev.org/Intel_8254x

mod constants;
mod device;
mod rx;
mod tx;

pub use device::Device as I82540EMEthernetController;
use x86_64::instructions::hlt;
use x86_64::structures::paging::OffsetPageTable;

use crate::drivers::i82540em::rx::{RX_BUFFERS, RX_DESCS, setup_rx};
use crate::drivers::i82540em::tx::setup_tx;
use crate::net::device::NetworkDevice;
use crate::net::send_arp_request;
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

pub fn find_and_setup_ethernet_controller(mapper: &OffsetPageTable<'static>) {
    let Some((bus, device)) = find_device(ID) else {
        return;
    };

    setup_device(mapper, bus, device);
}

fn setup_device(mapper: &OffsetPageTable<'static>, bus: u8, device: u8) {
    let bar0 = config_read_u32(bus, device, 0, 0x10);

    let mut eth_device = I82540EMEthernetController::from(mapper, bar0);

    eth_device.reset_nic_and_fetch_hw_address();

    let command = config_read_u32(bus, device, 0, 0x04);
    config_write_u32(bus, device, 0, 0x04, command | PCI_COMMAND_BME);

    setup_rx(&eth_device, mapper);
    setup_tx(&eth_device, mapper);

    eth_device.setup_handling();

    send_arp_request(&eth_device);

    for _ in 0..5 {
        hlt();
        hlt();
        hlt();
        hlt();
        hlt();
        unsafe {
            // kprintln!("{:#?}", *&raw const RX_DESCS);
            // kprintln!("{:?}", *&raw const RX_BUFFERS);
            if RX_DESCS[0].length != 0
                && let Some(rx_handler) = eth_device.rx_handler()
            {
                rx_handler(&RX_BUFFERS[0]);
            }
        }
    }

    // TODO: setup interrupts
    // TODO: send and receive packets
}
