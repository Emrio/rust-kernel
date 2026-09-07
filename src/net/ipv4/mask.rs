use crate::net::error::BufferTooSmall;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct IPv4Mask([u8; 4]);

impl IPv4Mask {
    pub const ALL: IPv4Mask = IPv4Mask([0xff; 4]);
    pub const EMPTY: usize = core::mem::size_of::<IPv4Mask>();

    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut address = [0; 4];
        address.copy_from_slice(bytes);
        Self(address)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn as_u32(self) -> u32 {
        u32::from_be_bytes(self.0)
    }
}

impl core::fmt::Display for IPv4Mask {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let [a, b, c, d] = self.0;
        f.write_fmt(format_args!("{a}.{b}.{c}.{d}"))
    }
}

impl TryInto<IPv4Mask> for &[u8] {
    type Error = BufferTooSmall;

    fn try_into(self) -> Result<IPv4Mask, Self::Error> {
        if self.len() == 4 {
            Ok(IPv4Mask::from_bytes(self))
        } else {
            Err(BufferTooSmall)
        }
    }
}
