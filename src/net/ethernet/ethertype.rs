#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum EtherType {
    // IPv4 = 0x0800,
    ARP = 0x0806,
}

impl EtherType {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut destination = [0; 2];
        destination.copy_from_slice(bytes);
        let value = u16::from_be_bytes(destination);
        unsafe { core::mem::transmute(value) }
    }

    pub fn as_bytes(self) -> [u8; 2] {
        let value = self as u16;
        let bytes = value.to_be_bytes();
        bytes
    }

    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

impl core::fmt::Display for EtherType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.as_u16() {
            x if x == EtherType::ARP as u16 => f.write_str("ARP"),
            x => f.write_fmt(format_args!("{x:#x} (unknown)")),
        }
    }
}
