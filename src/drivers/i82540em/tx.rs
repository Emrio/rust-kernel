use x86_64::structures::paging::OffsetPageTable;

use crate::bits::Split;
use crate::drivers::i82540em::device::Device;
use crate::memory::MemoryMapper;

#[derive(Clone, Copy, Debug)]
#[repr(C, align(16))]
pub struct TxDescriptor {
    pub(super) buffer_address: u64,
    pub(super) length: u16,
    pub(super) checksum_offset: u8,
    pub(super) command: u8,
    pub(super) status: u8,
    pub(super) checksum_start: u8,
    pub(super) special: u16,
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
pub const CMD_EOP: u8 = 1 << 0;
/// Insert FCS
pub const CMD_IFCS: u8 = 1 << 1;
/// Report Status
pub const CMD_RS: u8 = 1 << 3;

/// Descriptor Done
pub const STA_DD: u8 = 1 << 0;

pub const REG_TCTL: usize = 0x400;
pub const REG_TIPG: usize = 0x410;
pub const REG_TDBAL: usize = 0x3800;
pub const REG_TDBAH: usize = 0x3804;
pub const REG_TDLEN: usize = 0x3808;
pub const REG_TDH: usize = 0x3810;
pub const REG_TDT: usize = 0x3818;

/// Receiver Enable
pub const TCTL_EN: u32 = 1 << 1;
/// Pad Short Packets
pub const TCTL_PSP: u32 = 1 << 3;
/// Collision Threshold
pub const TCTL_CT: u32 = 0x0f << 4;
/// Collision Distance
pub const TCTL_COLD: u32 = 0x40 << 12;

pub fn setup_tx(device: &Device, mapper: &OffsetPageTable<'static>) {
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
