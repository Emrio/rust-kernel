use x86_64::instructions::port::Port;

// https://wiki.osdev.org/PCI#Configuration_Space_Access_Mechanism_#1
pub fn config_read_u32(bus: u8, device: u8, func: u8, offset: u8) -> u32 {
    assert!((0..32).contains(&device));
    assert!((0..8).contains(&func));

    let address = 0x80000000u32
        | (bus as u32) << 16
        | (device as u32) << 11
        | (func as u32) << 8
        | (offset & 0xFC) as u32;

    let mut port1 = Port::new(0xCF8);
    let mut port2 = Port::new(0xCFC);

    unsafe {
        port1.write(address);
        port2.read()
    }
}

pub fn config_write_u32(bus: u8, slot: u8, func: u8, offset: u8, value: u32) {
    assert!((0..32).contains(&slot));
    assert!((0..8).contains(&func));

    let address = 0x80000000u32
        | (bus as u32) << 16
        | (slot as u32) << 11
        | (func as u32) << 8
        | (offset & 0xFC) as u32;

    let mut port1 = Port::new(0xCF8);
    let mut port2 = Port::new(0xCFC);

    unsafe {
        port1.write(address);
        port2.write(value)
    }
}

pub fn find_device(target_id: u32) -> Option<(u8, u8)> {
    for bus in 0..=255 {
        for device in 0..32 {
            if config_read_u32(bus, device, 0, 0) == target_id {
                return Some((bus, device));
            }
        }
    }

    None
}
