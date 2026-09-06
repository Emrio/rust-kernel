use crate::net::error::BufferTooSmall;

mod field {
    pub const SOURCE: core::ops::Range<usize> = 0..2;
    pub const DESTINATION: core::ops::Range<usize> = 2..4;
    pub const LENGTH: core::ops::Range<usize> = 4..6;
    // pub const CHECKSUM: core::ops::Range<usize> = 6..8;
}

pub struct UDPPacket<T: AsRef<[u8]>> {
    buffer: T,
}

pub const UDP_HEADER: usize = 8;

impl<T: AsRef<[u8]>> UDPPacket<T> {
    pub fn new_unchecked(buffer: T) -> Self {
        Self { buffer }
    }

    fn check_length(&self) -> Result<(), BufferTooSmall> {
        if self.buffer.as_ref().len() < UDP_HEADER {
            Err(BufferTooSmall)
        } else {
            Ok(())
        }
    }

    pub fn new(buffer: T) -> Result<Self, BufferTooSmall> {
        let packet = Self::new_unchecked(buffer);
        packet.check_length()?;
        Ok(packet)
    }

    pub fn into_inner(self) -> T {
        self.buffer
    }

    pub fn source(&self) -> u16 {
        let mut destination = [0; size_of::<u16>()];
        destination.copy_from_slice(&self.buffer.as_ref()[field::SOURCE]);
        u16::from_be_bytes(destination)
    }

    pub fn destination(&self) -> u16 {
        let mut destination = [0; size_of::<u16>()];
        destination.copy_from_slice(&self.buffer.as_ref()[field::DESTINATION]);
        u16::from_be_bytes(destination)
    }

    pub fn packet_length(&self) -> usize {
        let mut destination = [0; size_of::<u16>()];
        destination.copy_from_slice(&self.buffer.as_ref()[field::LENGTH]);
        let size = u16::from_be_bytes(destination);
        size as usize
    }

    pub fn payload(&self) -> &[u8] {
        let range = UDP_HEADER..self.packet_length();
        &self.buffer.as_ref()[range]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> UDPPacket<T> {
    pub fn set_source(&mut self, source_port: u16) -> &mut Self {
        self.buffer.as_mut()[field::SOURCE].copy_from_slice(&source_port.to_be_bytes());
        self
    }

    pub fn set_destination(&mut self, destination_port: u16) -> &mut Self {
        self.buffer.as_mut()[field::DESTINATION].copy_from_slice(&destination_port.to_be_bytes());
        self
    }

    pub fn set_length(&mut self, length: usize) -> &mut Self {
        assert!(
            length < u16::MAX as usize,
            "UDP packet max length is 65 535 bytes."
        );
        self.buffer.as_mut()[field::LENGTH].copy_from_slice(&(length as u16).to_be_bytes());
        self
    }

    pub fn set_payload(&mut self, payload: &[u8]) -> &mut Self {
        let range = UDP_HEADER..self.packet_length();
        self.buffer.as_mut()[range].copy_from_slice(payload);
        self
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        let range = UDP_HEADER..self.packet_length();
        &mut self.buffer.as_mut()[range]
    }
}

impl<T: AsRef<[u8]>> core::fmt::Display for UDPPacket<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "UDP(source={}, destination={}, payload={} bytes)",
            self.source(),
            self.destination(),
            self.payload().len(),
        ))
    }
}
