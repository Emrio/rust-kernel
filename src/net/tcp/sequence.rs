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

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn ordering_is_trivial_when_not_wrapping() {
        assert!(Sequence::from(100) < Sequence::from(200));
        assert!(Sequence::from(200) > Sequence::from(100));
        assert_eq!(Sequence::from(100), Sequence::from(100));
    }

    #[test_case]
    fn ordering_respects_wraparound() {
        // u32::MAX comes right before 0 on the circular sequence space,
        // even though as plain integers MAX > 0.
        let just_before_wrap = Sequence::from(u32::MAX);
        let just_after_wrap = Sequence::from(0);

        assert!(just_before_wrap < just_after_wrap);
        assert!(just_after_wrap > just_before_wrap);
    }

    #[test_case]
    fn ordering_still_works_a_bit_further_past_the_wrap() {
        let before = Sequence::from(u32::MAX - 10);
        let after = Sequence::from(5);

        assert!(before < after);
        assert!(after > before);
    }

    #[test_case]
    fn add_wraps_around() {
        let seq = Sequence::from(u32::MAX);
        assert_eq!(seq + 1, Sequence::from(0));
        assert_eq!(seq + 11, Sequence::from(10));
    }

    #[test_case]
    fn add_assign_wraps_around() {
        let mut seq = Sequence::from(u32::MAX - 2);
        seq += 5;
        assert_eq!(seq, Sequence::from(2));
    }
}
