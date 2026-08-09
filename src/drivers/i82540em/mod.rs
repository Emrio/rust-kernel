// https://www.intel.com/content/dam/doc/manual/pci-pci-x-family-gbe-controllers-software-dev-manual.pdf
// https://wiki.osdev.org/Intel_8254x

mod constants;
mod device;
mod rx;
mod tx;

use conquer_once::spin::OnceCell;

pub use device::Device as I82540EMEthernetController;
use x86_64::instructions::hlt;

use crate::bits::SplitTwice;
use crate::drivers::i82540em::rx::{enable_rx_interrupts, setup_rx};
use crate::drivers::i82540em::tx::setup_tx;
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

pub static DEVICE: OnceCell<I82540EMEthernetController> = OnceCell::uninit();

pub fn find_and_setup_ethernet_controller() {
    let Some((bus, device)) = find_device(ID) else {
        return;
    };

    setup_device(bus, device);
}

fn setup_device(bus: u8, device: u8) {
    let bar0 = config_read_u32(bus, device, 0, 0x10);

    let mut eth_device = I82540EMEthernetController::from(bar0);

    eth_device.reset_nic_and_fetch_hw_address();

    while !eth_device.status().is_link_up() {
        kprintln!("Not ready!");
        hlt();
    }
    kprintln!("Link up, speed: {} Mbit/s", eth_device.status().speed());

    let command = config_read_u32(bus, device, 0, 0x04);
    config_write_u32(bus, device, 0, 0x04, command | PCI_COMMAND_BME);

    setup_rx(&eth_device);
    setup_tx(&eth_device);

    DEVICE.init_once(|| eth_device);

    let eth_device = DEVICE.get().expect("device to be initialized");

    let (_, _, _, interupt_line) = config_read_u32(bus, device, 0, 0x3c).split_twice();
    enable_rx_interrupts(eth_device, interupt_line);
}
