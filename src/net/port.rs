#[derive(Debug)]
pub struct InvalidSize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Port(u16);

impl Port {
    pub const fn new(port: u16) -> Self {
        Self(port)
    }

    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, InvalidSize> {
        Ok(Port::new(u16::from_be_bytes(
            bytes.try_into().map_err(|_| InvalidSize)?,
        )))
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::try_from_bytes(bytes).expect("port bytes to be 2 bytes")
    }

    pub fn to_bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }
}

impl core::fmt::Display for Port {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl core::str::FromStr for Port {
    type Err = core::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse::<u16>()?))
    }
}

impl From<u16> for Port {
    fn from(value: u16) -> Self {
        Self(value)
    }
}
