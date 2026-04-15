// https://www.intel.com/content/dam/doc/manual/pci-pci-x-family-gbe-controllers-software-dev-manual.pdf
// https://wiki.osdev.org/Intel_8254x

mod device;
mod hardware_address;

pub use device::Device as I82540EMEthernetController;
use x86_64::instructions::hlt;

use crate::pci::{config_read_u32, find_device};

use hardware_address::HardwareAddress;

const ID: u32 = 0x100e_8086;
const I8254_REG_CTRL: usize = 0x0;
const I8254_CTRL_ASDE: u32 = 1 << 5;
const I8254_CTRL_SLU: u32 = 1 << 6;
const I8254_CTRL_RESET: u32 = 1 << 26;
const I8254_REG_EERD: usize = 0x14;
const I8254_EERD_DONE: u32 = 1 << 4;
const I8254_REG_RAL: usize = 0x5400;
const I8254_REG_RAH: usize = 0x5404;

pub fn find_and_setup_ethernet_controller(memory_offset: u64) {
    let Some((bus, device)) = find_device(ID) else {
        return;
    };

    setup_device(memory_offset, bus, device);
}

fn setup_device(memory_offset: u64, bus: u8, device: u8) {
    let bar0 = config_read_u32(bus, device, 0, 0x10);

    let device = I82540EMEthernetController::from(memory_offset, bar0);

    reset_nic(&device);

    // TODO: setup token rings
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

fn reset_nic(device: &I82540EMEthernetController) {
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

    let hwaddr = HardwareAddress::from(b0, b1, b2);
    kprintln!("{hwaddr}");

    device.write_register(I8254_REG_RAL, (b1 as u32) << 16 | (b0 as u32));
    device.write_register(I8254_REG_RAH, b2 as u32 | /* Address valid */ (1 << 31));
}
