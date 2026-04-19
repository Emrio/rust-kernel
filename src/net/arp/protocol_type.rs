#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ProtocolType {
    IPv4 = 0x0800,
}

impl ProtocolType {
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

impl Default for ProtocolType {
    fn default() -> Self {
        Self::IPv4
    }
}

impl core::fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.as_u16() {
            x if x == ProtocolType::IPv4 as u16 => f.write_str("IPv4"),
            x => f.write_fmt(format_args!("{x:#x} (unknown)")),
        }
    }
}

#[cfg(test)]
mod test {
    use super::ProtocolType;

    #[test_case]
    fn proto_type_default_is_ipv4() {
        let proto_type = ProtocolType::default();
        assert_eq!(proto_type.as_bytes(), 0x0800u16.to_be_bytes())
    }

    // TODO: test that format recognizes IPv4
}
