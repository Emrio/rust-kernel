#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum IcmpType {
    EchoReply = 0,
    EchoRequest = 8,
}

impl IcmpType {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut destination = [0; size_of::<IcmpType>()];
        destination.copy_from_slice(bytes);
        let value = u8::from_be_bytes(destination);
        unsafe { core::mem::transmute(value) }
    }

    pub fn as_bytes(self) -> [u8; size_of::<IcmpType>()] {
        let value = self as u8;
        value.to_be_bytes()
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

impl core::fmt::Display for IcmpType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.as_u8() {
            x if x == IcmpType::EchoReply as u8 => f.write_str("Echo Reply"),
            x if x == IcmpType::EchoRequest as u8 => f.write_str("Echo Request"),
            x => f.write_fmt(format_args!("{x:#x} (unknown)")),
        }
    }
}
