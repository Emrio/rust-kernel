#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Operation {
    Request = 0x1,
    Reply = 0x2,
}

impl Operation {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut destination = [0; core::mem::size_of::<Self>()];
        destination.copy_from_slice(bytes);
        let value = u16::from_be_bytes(destination);
        unsafe { core::mem::transmute(value) }
    }

    pub fn as_bytes(self) -> [u8; core::mem::size_of::<Self>()] {
        let value = self as u16;
        let bytes = value.to_be_bytes();
        bytes
    }

    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

impl core::fmt::Display for Operation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.as_u16() {
            x if x == Operation::Request as u16 => f.write_str("Request"),
            x if x == Operation::Reply as u16 => f.write_str("Reply"),
            x => f.write_fmt(format_args!("{x:#x} (unknown)")),
        }
    }
}
