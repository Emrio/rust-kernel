#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sequence(u32);

impl Sequence {
    pub fn random() -> Self {
        Self(0x4242) // TODO:
    }
}

impl From<u32> for Sequence {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

impl From<Sequence> for u32 {
    fn from(val: Sequence) -> Self {
        val.0
    }
}

impl core::ops::AddAssign<u32> for Sequence {
    fn add_assign(&mut self, rhs: u32) {
        self.0 = self.0.wrapping_add(rhs)
    }
}

impl core::ops::Add<u32> for Sequence {
    type Output = Self;

    fn add(self, rhs: u32) -> Self::Output {
        Self(self.0.wrapping_add(rhs))
    }
}

impl core::cmp::PartialOrd for Sequence {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::cmp::Ord for Sequence {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        if self.0 == other.0 {
            core::cmp::Ordering::Equal
        } else if (self.0.wrapping_sub(other.0) as i32) < 0 {
            core::cmp::Ordering::Less
        } else {
            core::cmp::Ordering::Greater
        }
    }
}

impl core::fmt::Display for Sequence {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}
