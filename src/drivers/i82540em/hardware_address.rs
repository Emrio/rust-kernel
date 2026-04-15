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
