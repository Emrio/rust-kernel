use crate::bits::Split;
use crate::drivers::i82540em::constants::{
    RCTL_BAM, RCTL_BSEX, RCTL_BSIZE_FULL, RCTL_EN, RCTL_UPE, REG_RCTL, REG_RDBAH, REG_RDBAL,
    REG_RDLEN, REG_RDT,
};
use crate::drivers::i82540em::device::Device;
use crate::memory::MemoryMapper;

#[derive(Default, Clone, Copy, Debug)]
#[repr(C, align(16))]
pub struct RxDescriptor {
    buffer_address: u64,
    pub(super) length: u16,
    checksum: u16,
    /// DD = Descriptor Done
    /// EOF = End Of Packet
    /// IXSM = Ignore Checksum Indication
    /// VP = Packet is 802.1Q
    /// --
    /// TCPCS = TCP Checksum Calculated on Packet
    /// IPCS = IP Checksum Calculated on Packet
    /// PIF = Passed in-exact Filter
    status: u8,
    error: u8,
    special: u16,
}

impl RxDescriptor {
    const fn new() -> Self {
        Self {
            buffer_address: 0,
            special: 0,
            error: 0,
            status: 0,
            checksum: 0,
            length: 0,
        }
    }
}

pub const RX_SIZE: usize = 8;
pub const PACKET_SIZE: usize = 4096;
pub static mut RX_DESCS: [RxDescriptor; RX_SIZE] = [RxDescriptor::new(); RX_SIZE];
pub static mut RX_BUFFERS: [[u8; PACKET_SIZE]; RX_SIZE] = [[0u8; PACKET_SIZE]; RX_SIZE];

pub fn setup_rx(device: &Device, mapper: &MemoryMapper) {
    for index in 0..RX_SIZE {
        unsafe {
            RX_DESCS[index].buffer_address = mapper.to_physical(&raw const RX_BUFFERS[index])
        };
    }

    let rx_desc_address = mapper.to_physical(&raw mut RX_DESCS);
    let (base_address_high, base_address_low) = rx_desc_address.split();
    device.write_register(REG_RDBAL, base_address_low);
    device.write_register(REG_RDBAH, base_address_high);
    device.write_register(
        REG_RDLEN,
        (RX_SIZE * core::mem::size_of::<RxDescriptor>()) as u32,
    );
    device.write_register(REG_RDT, RX_SIZE as u32 - 1);

    device.write_register(
        REG_RCTL,
        RCTL_EN | RCTL_UPE | RCTL_BAM | RCTL_BSIZE_FULL | RCTL_BSEX,
    );
}
