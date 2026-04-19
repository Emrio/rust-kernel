use crate::bits::Split;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthernetAddress([u8; 6]);

impl EthernetAddress {
    pub const BROADCAST: EthernetAddress = EthernetAddress([0xff; 6]);
    pub const SIZE: usize = core::mem::size_of::<EthernetAddress>();

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut address = [0; 6];
        address.copy_from_slice(bytes);
        Self(address)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn from_u16(b0: u16, b1: u16, b2: u16) -> Self {
        let (a1, a0) = b0.split();
        let (a3, a2) = b1.split();
        let (a5, a4) = b2.split();
        Self([a0, a1, a2, a3, a4, a5])
    }

    pub fn is_broadcast(&self) -> bool {
        *self == EthernetAddress::BROADCAST
    }
}

impl core::fmt::Display for EthernetAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        ))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::Write;

    use super::EthernetAddress;

    // FIXME: string allocation
    struct StringBuffer<const N: usize> {
        buf: [u8; N],
        pos: usize,
    }

    impl<const N: usize> StringBuffer<N> {
        fn new() -> Self {
            Self {
                buf: [0u8; N],
                pos: 0,
            }
        }

        fn as_str(&self) -> &str {
            core::str::from_utf8(&self.buf[..self.pos]).unwrap()
        }
    }

    impl<const N: usize> core::fmt::Write for StringBuffer<N> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let bytes = s.as_bytes();
            self.buf[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
            self.pos += bytes.len();
            Ok(())
        }
    }

    #[test_case]
    fn test_hw_addr_formatter() {
        let addr = EthernetAddress::from_u16(0x1234, 0x5678, 0x9abc);

        let mut buffer = StringBuffer::<17>::new();
        write!(buffer, "{}", addr).unwrap();
        assert_eq!(buffer.as_str(), "34:12:78:56:bc:9a");
    }

    #[test_case]
    fn test_hw_addr_low_digits() {
        let addr = EthernetAddress::from_u16(0x0102, 0x120a, 0x0fff);

        let mut buffer = StringBuffer::<17>::new();
        write!(buffer, "{}", addr).unwrap();
        assert_eq!(buffer.as_str(), "02:01:0a:12:ff:0f");
    }

    #[test_case]
    fn test_hw_addr_broadcast() {
        let addr = EthernetAddress::BROADCAST;

        let mut buffer = StringBuffer::<17>::new();
        write!(buffer, "{}", addr).unwrap();

        assert_eq!(addr.is_broadcast(), true);
        assert_eq!(buffer.as_str(), "ff:ff:ff:ff:ff:ff");
    }
}
