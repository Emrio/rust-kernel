use x86_64::instructions::hlt;

use crate::bits::Split;
use crate::drivers::i82540em::device::Device;
use crate::memory::MemoryMapper;

#[derive(Clone, Copy, Debug)]
#[repr(C, align(16))]
pub struct TxDescriptor {
    buffer_address: u64,
    length: u16,
    checksum_offset: u8,
    command: u8,
    status: u8,
    checksum_start: u8,
    special: u16,
}

pub const TX_SIZE: usize = 8;
pub const PACKET_SIZE: usize = 4096;
pub static mut TX_DESCS: [TxDescriptor; TX_SIZE] = [TxDescriptor {
    buffer_address: 0,
    length: 0,
    checksum_offset: 0,
    command: 0,
    status: 0,
    checksum_start: 0,
    special: 0,
}; TX_SIZE];
pub static mut TX_BUFFERS: [[u8; PACKET_SIZE]; TX_SIZE] = [[0u8; PACKET_SIZE]; TX_SIZE];

/// End Of Packet
const CMD_EOP: u8 = 1 << 0;
/// Insert FCS
const CMD_IFCS: u8 = 1 << 1;
/// Report Status
const CMD_RS: u8 = 1 << 3;

/// Descriptor Done
const STA_DD: u8 = 1 << 0;

const REG_TCTL: usize = 0x400;
const REG_TIPG: usize = 0x410;
const REG_TDBAL: usize = 0x3800;
const REG_TDBAH: usize = 0x3804;
const REG_TDLEN: usize = 0x3808;
const REG_TDH: usize = 0x3810;
const REG_TDT: usize = 0x3818;

/// Receiver Enable
const TCTL_EN: u32 = 1 << 1;
/// Pad Short Packets
const TCTL_PSP: u32 = 1 << 3;
/// Collision Threshold
const TCTL_CT: u32 = 0x0f << 4;
/// Collision Distance
const TCTL_COLD: u32 = 0x40 << 12;

pub fn setup_tx(device: &Device, mapper: &MemoryMapper) {
    let tx_desc_address = mapper.to_physical(&raw mut TX_DESCS);
    let (base_address_high, base_address_low) = tx_desc_address.split();
    device.write_register(REG_TDBAL, base_address_low);
    device.write_register(REG_TDBAH, base_address_high);
    device.write_register(
        REG_TDLEN,
        (TX_SIZE * core::mem::size_of::<TxDescriptor>()) as u32,
    );
    device.write_register(REG_TDT, 0);

    // IPGT=10, IPGR1=8, IPGR2=6
    device.write_register(REG_TIPG, 0x60200a);

    device.write_register(REG_TCTL, TCTL_EN | TCTL_PSP | TCTL_CT | TCTL_COLD);
}

pub fn send_packet(device: &Device, mapper: &MemoryMapper, packet: &[u8]) {
    let tail = device.read_register(REG_TDT) as usize;
    let head = device.read_register(REG_TDH) as usize;

    if (tail + 1) % TX_SIZE == head {
        kprintln!("Cannot send packet: buffer full");
    }

    unsafe {
        TX_BUFFERS[tail][0..packet.len()].copy_from_slice(packet);

        let descriptor = &raw mut TX_DESCS[tail];
        descriptor.write_volatile(TxDescriptor {
            buffer_address: mapper.to_physical(&raw const TX_BUFFERS[tail]),
            length: packet.len() as u16,
            checksum_offset: 0,
            command: CMD_EOP | CMD_IFCS | CMD_RS,
            status: 0,
            checksum_start: 0,
            special: 0,
        });
    }

    device.write_register(REG_TDT, ((tail + 1) % TX_SIZE) as u32);

    let status = unsafe { &raw const TX_DESCS[tail].status };

    while unsafe { status.read_volatile() } & STA_DD == 0 {
        hlt();
    }
}
