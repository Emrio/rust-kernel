#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum HardwareType {
    Ethernet = 0x1,
}

impl HardwareType {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut destination = [0; 2];
        destination.copy_from_slice(bytes);
        let value = u16::from_be_bytes(destination);
        unsafe { core::mem::transmute(value) }
    }

    pub fn as_bytes(self) -> [u8; 2] {
        let value = self as u16;
        let bytes = value.to_be_bytes();
        bytes
    }

    pub fn as_u16(self) -> u16 {
        self as u16
    }
}

impl Default for HardwareType {
    fn default() -> Self {
        Self::Ethernet
    }
}

impl core::fmt::Display for HardwareType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.as_u16() {
            x if x == HardwareType::Ethernet as u16 => f.write_str("Ethernet"),
            x => f.write_fmt(format_args!("{:#x} (unknown)", x)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::HardwareType;

    #[test_case]
    fn hw_type_default_is_ethernet() {
        let hw_type = HardwareType::default();
        assert_eq!(hw_type.as_bytes(), 1u16.to_be_bytes())
    }

    // TODO: test that format recognizes Ethernet
}
