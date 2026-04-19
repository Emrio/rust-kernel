#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IPv4Address([u8; 4]);

impl IPv4Address {
    pub const BROADCAST: IPv4Address = IPv4Address([0xff; 4]);
    pub const SIZE: usize = core::mem::size_of::<IPv4Address>();

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

    pub fn is_broadcast(&self) -> bool {
        *self == IPv4Address::BROADCAST
    }
}

impl core::fmt::Display for IPv4Address {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let [a, b, c, d] = self.0;
        f.write_fmt(format_args!("{a}.{b}.{c}.{d}"))
    }
}

// impl Into<u32> for IPv4Address {
//     fn into(self) -> u32 {
//         self.0
//     }
// }

// impl From<u32> for IPv4Address {
//     fn from(address: u32) -> Self {
//         Self(address)
//     }
// }

#[cfg(test)]
mod test {
    use super::IPv4Address;

    #[test_case]
    fn parse_4_bytes_localhost() {
        let ip = IPv4Address::new(192, 168, 0, 1);
        assert_eq!(ip.as_u32(), 0xc0_a8_00_01u32);
    }

    #[test_case]
    fn parse_4_bytes_privnet() {
        let ip = IPv4Address::new(10, 0, 2, 1);
        assert_eq!(ip.as_u32(), 0x0a_00_02_01u32);
    }

    // TODO: test formatting order
}
