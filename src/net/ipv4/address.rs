extern crate alloc;

use alloc::vec::Vec;

use crate::net::error::BufferTooSmall;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, PartialOrd, Ord)]
pub struct IPv4Address([u8; 4]);

impl IPv4Address {
    pub const BROADCAST: IPv4Address = IPv4Address([0xff; _]);
    pub const SIZE: usize = core::mem::size_of::<IPv4Address>();

    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self([a, b, c, d])
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut address = [0; _];
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

    pub fn is_zero(&self) -> bool {
        *self == IPv4Address::default()
    }
}

impl core::fmt::Display for IPv4Address {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let [a, b, c, d] = self.0;
        f.write_fmt(format_args!("{a}.{b}.{c}.{d}"))
    }
}

impl TryInto<IPv4Address> for &[u8] {
    type Error = BufferTooSmall;

    fn try_into(self) -> Result<IPv4Address, Self::Error> {
        if self.len() == 4 {
            Ok(IPv4Address::from_bytes(self))
        } else {
            Err(BufferTooSmall)
        }
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

pub enum IPv4AddressParseError {
    MissingThreeDot,
    ParseU8Error,
}

impl core::str::FromStr for IPv4Address {
    type Err = IPv4AddressParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let digits: Vec<&str> = s.split(".").collect();

        let [a, b, c, d] = digits.as_slice() else {
            Err(IPv4AddressParseError::MissingThreeDot)?
        };

        let a = a.parse().map_err(|_| IPv4AddressParseError::ParseU8Error)?;
        let b = b.parse().map_err(|_| IPv4AddressParseError::ParseU8Error)?;
        let c = c.parse().map_err(|_| IPv4AddressParseError::ParseU8Error)?;
        let d = d.parse().map_err(|_| IPv4AddressParseError::ParseU8Error)?;

        Ok(IPv4Address::new(a, b, c, d))
    }
}

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
