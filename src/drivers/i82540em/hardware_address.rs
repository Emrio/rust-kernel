use crate::bits::Split;

pub struct HardwareAddress(u8, u8, u8, u8, u8, u8);

impl HardwareAddress {
    pub fn from(b0: u16, b1: u16, b2: u16) -> Self {
        let (a1, a0) = b0.split();
        let (a3, a2) = b1.split();
        let (a5, a4) = b2.split();
        Self(a0, a1, a2, a3, a4, a5)
    }
}

impl core::fmt::Display for HardwareAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 < 0x10 {
            f.write_str("0")?
        }
        core::fmt::LowerHex::fmt(&self.0, f)?;
        f.write_str(":")?;
        if self.1 < 0x10 {
            f.write_str("0")?
        }
        core::fmt::LowerHex::fmt(&self.1, f)?;
        f.write_str(":")?;
        if self.2 < 0x10 {
            f.write_str("0")?
        }
        core::fmt::LowerHex::fmt(&self.2, f)?;
        f.write_str(":")?;
        if self.3 < 0x10 {
            f.write_str("0")?
        }
        core::fmt::LowerHex::fmt(&self.3, f)?;
        f.write_str(":")?;
        if self.4 < 0x10 {
            f.write_str("0")?
        }
        core::fmt::LowerHex::fmt(&self.4, f)?;
        f.write_str(":")?;
        if self.5 < 0x10 {
            f.write_str("0")?
        }
        core::fmt::LowerHex::fmt(&self.5, f)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::Write;

    use super::HardwareAddress;

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
        let addr = HardwareAddress::from(0x1234, 0x5678, 0x9abc);

        let mut buffer = StringBuffer::<17>::new();
        write!(buffer, "{}", addr).unwrap();
        assert_eq!(buffer.as_str(), "34:12:78:56:bc:9a");
    }

    #[test_case]
    fn test_hw_addr_low_digits() {
        let addr = HardwareAddress::from(0x0102, 0x120a, 0x0fff);

        let mut buffer = StringBuffer::<17>::new();
        write!(buffer, "{}", addr).unwrap();
        assert_eq!(buffer.as_str(), "02:01:0a:12:ff:0f");
    }
}
