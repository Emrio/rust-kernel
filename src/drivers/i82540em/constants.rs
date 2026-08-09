pub const REG_STATUS: usize = 0x8;
/// Link Up?
pub const STATUS_LU: u32 = 1 << 1;
pub const STATUS_SPEED_0: u32 = 0;
pub const STATUS_SPEED_100: u32 = 1 << 6;
pub const STATUS_SPEED: u32 = 0b11 << 6;
pub const REG_RCTL: usize = 0x100;
/// Receiver Enable
pub const RCTL_EN: u32 = 1 << 1;
/// Unicast Promiscuous Mode
pub const RCTL_UPE: u32 = 1 << 3;
/// Broadcast Accept Mode
pub const RCTL_BAM: u32 = 1 << 15;
/// Receive Buffer Size full (4096 bytes when BSEX = 1)
pub const RCTL_BSIZE_FULL: u32 = 0b11 << 16;
/// Buffer Size Extension
pub const RCTL_BSEX: u32 = 1 << 25;

pub const REG_RDBAL: usize = 0x2800;

pub const REG_RDBAH: usize = 0x2804;

pub const REG_RDLEN: usize = 0x2808;

// pub const REG_RDH: usize = 0x2810;

pub const REG_RDH: usize = 0x2810;
pub const REG_RDT: usize = 0x2818;

/// Interrupt Mask Set/Read: enable interrupts
pub const REG_IMS: usize = 0xd0;
/// Receiver Timer Interrupt
pub const IMS_RXT0: u32 = 1 << 7;
