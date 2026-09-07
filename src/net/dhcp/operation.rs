#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Operation {
    BootRequest = 0x1,
    BootReply = 0x2,
}

impl Operation {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut destination = [0; core::mem::size_of::<Self>()];
        destination.copy_from_slice(bytes);
        let value = u8::from_be_bytes(destination);
        unsafe { core::mem::transmute(value) }
    }

    pub fn as_bytes(self) -> [u8; core::mem::size_of::<Self>()] {
        let value = self as u8;
        value.to_be_bytes()
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl core::fmt::Display for Operation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.as_u8() {
            x if x == Operation::BootRequest as u8 => f.write_str("BootRequest"),
            x if x == Operation::BootReply as u8 => f.write_str("BootReply"),
            x => f.write_fmt(format_args!("{x:#x} (unknown)")),
        }
    }
}
