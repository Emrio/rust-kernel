pub mod address;
pub mod ethertype;

use address::EthernetAddress;
use ethertype::EtherType;

use super::error::BufferTooSmall;

pub struct EthernetFrame<T: AsRef<[u8]>> {
    buffer: T,
}

mod field {
    pub const DESTINATION: core::ops::Range<usize> = 0..6;
    pub const SOURCE: core::ops::Range<usize> = 6..12;
    pub const ETHERTYPE: core::ops::Range<usize> = 12..14;
    pub const PAYLOAD: core::ops::RangeFrom<usize> = 14..;
}
pub const ETHERNET_HEADER: usize = 14;

impl<T: AsRef<[u8]>> EthernetFrame<T> {
    pub fn new_unchecked(buffer: T) -> Self {
        Self { buffer }
    }

    fn check_length(&self) -> Result<(), BufferTooSmall> {
        if self.buffer.as_ref().len() < ETHERNET_HEADER {
            Err(BufferTooSmall)
        } else {
            Ok(())
        }
    }

    pub fn new(buffer: T) -> Result<Self, BufferTooSmall> {
        let frame = Self::new_unchecked(buffer);
        frame.check_length()?;
        Ok(frame)
    }

    pub fn into_inner(self) -> T {
        self.buffer
    }

    pub fn destination(&self) -> EthernetAddress {
        EthernetAddress::from_bytes(&self.buffer.as_ref()[field::DESTINATION])
    }

    pub fn source(&self) -> EthernetAddress {
        EthernetAddress::from_bytes(&self.buffer.as_ref()[field::SOURCE])
    }

    pub fn ethertype(&self) -> EtherType {
        EtherType::from_bytes(&self.buffer.as_ref()[field::ETHERTYPE])
    }

    pub fn payload(&self) -> &[u8] {
        &self.buffer.as_ref()[field::PAYLOAD]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> EthernetFrame<T> {
    pub fn set_destination(&mut self, destination: EthernetAddress) -> &mut Self {
        self.buffer.as_mut()[field::DESTINATION].copy_from_slice(destination.as_bytes());
        self
    }

    pub fn set_source(&mut self, source: EthernetAddress) -> &mut Self {
        self.buffer.as_mut()[field::SOURCE].copy_from_slice(source.as_bytes());
        self
    }

    pub fn set_ethertype(&mut self, ethertype: EtherType) -> &mut Self {
        self.buffer.as_mut()[field::ETHERTYPE].copy_from_slice(&ethertype.as_bytes());
        self
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.buffer.as_mut()[field::PAYLOAD]
    }
}

impl<T: AsRef<[u8]>> core::fmt::Display for EthernetFrame<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "Ethernet(destination={}, source={}, ethertype={})",
            self.destination(),
            self.source(),
            self.ethertype()
        ))
    }
}

#[cfg(test)]
mod test {
    #[test_case]
    fn test_create_ethernet_frame() {
        // TODO:
    }

    #[test_case]
    fn test_process_ethernet_frame() {
        // TODO:
    }
}
