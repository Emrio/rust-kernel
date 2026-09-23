use crate::bits::Split;

#[derive(Debug, Default)]
pub struct Checksum {
    sum: u32,
    rem: Option<u8>,
}

impl Checksum {
    pub fn new() -> Self {
        Self::default()
    }

    fn ingest(&mut self, a: u8, b: u8) {
        let word = u16::from_be_bytes([a, b]);
        self.sum = self.sum.wrapping_add(word as u32);
    }

    pub fn feed(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        let data = if let Some(rem) = self.rem {
            self.ingest(rem, data[0]);
            self.rem = None;
            &data[1..]
        } else {
            data
        };

        if data.is_empty() {
            return;
        }

        let mut chunks = data.chunks_exact(2);
        for chunk in &mut chunks {
            self.ingest(chunk[0], chunk[1]);
        }

        if let [last] = chunks.remainder() {
            self.rem = Some(*last);
        }
    }

    pub fn finish(mut self) -> u16 {
        if let Some(rem) = self.rem {
            self.ingest(rem, 0);
        }

        let (a, b) = self.sum.split();
        !(a.wrapping_add(b))
    }
}

pub fn checksum(data: &[u8]) -> u16 {
    let mut checksum = Checksum::new();
    checksum.feed(data);
    checksum.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn checksum_empty() {
        assert_eq!(checksum(&[]), 0xffff);
    }

    #[test_case]
    fn checksum_with_carry() {
        assert_eq!(checksum(&[0xff, 0xff, 0x00, 0x01]), 0xfffe);
    }

    #[test_case]
    fn checksum_random() {
        assert_eq!(checksum(&[0x12, 0x34, 0x56, 0x78, 0x9a]), 0xfd52);
    }

    #[test_case]
    fn checksum_single_byte() {
        assert_eq!(checksum(&[0xff]), 0x00ff);
    }
}
