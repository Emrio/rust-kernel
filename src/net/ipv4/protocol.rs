#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Protocol {
    ICMP = 0x1,
    TCP = 0x6,
    UDP = 0x11,
}

impl Protocol {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut destination = [0; size_of::<Protocol>()];
        destination.copy_from_slice(bytes);
        let value = u8::from_be_bytes(destination);
        unsafe { core::mem::transmute(value) }
    }

    pub fn as_bytes(self) -> [u8; size_of::<Protocol>()] {
        let value = self as u8;
        value.to_be_bytes()
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl core::fmt::Display for Protocol {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.as_u8() {
            x if x == Protocol::ICMP as u8 => f.write_str("ICMP"),
            x if x == Protocol::TCP as u8 => f.write_str("TCP"),
            x if x == Protocol::UDP as u8 => f.write_str("UDP"),
            x => f.write_fmt(format_args!("{x:#x} (unknown)")),
        }
    }
}
