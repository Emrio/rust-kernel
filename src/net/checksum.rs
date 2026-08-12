use crate::bits::Split;

pub fn checksum(data: &[u8]) -> u16 {
    let mut sum = 0u32;

    let mut chunks = data.chunks_exact(2);
    for chunk in &mut chunks {
        let word = u16::from_be_bytes([chunk[0], chunk[1]]);
        sum = sum.wrapping_add(word as u32);
    }

    if let [last] = chunks.remainder() {
        let word = u16::from_be_bytes([*last, 0]);
        sum = sum.wrapping_add(word as u32);
    }

    let (a, b) = sum.split();

    !(a.wrapping_add(b))
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
