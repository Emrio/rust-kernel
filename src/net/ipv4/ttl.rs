#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeToLive(u8);

impl TimeToLive {
    pub fn new(ttl: u8) -> Self {
        Self(ttl)
    }

    pub fn max() -> Self {
        Self(u8::MAX)
    }

    pub fn as_u8(&self) -> u8 {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl core::fmt::Display for TimeToLive {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("TTL({})", self.0))
    }
}
