pub mod icmp_type;

use crate::net::{checksum::checksum, error::BufferTooSmall, icmp::icmp_type::IcmpType};

pub struct ICMPPacket<T: AsRef<[u8]>> {
    buffer: T,
}

mod field {
    pub const TYPE: core::ops::Range<usize> = 0..1;
    pub const CODE: usize = 1;
    pub const CHECKSUM: core::ops::Range<usize> = 2..4;
    pub const ECHO_IDENTIFIER: core::ops::Range<usize> = 4..6;
    pub const ECHO_SEQUENCE: core::ops::Range<usize> = 6..8;
}

pub const ICMP_PACKET: usize = 4;
pub const ECHO_PACKET: usize = 8;

impl<T: AsRef<[u8]>> ICMPPacket<T> {
    pub fn new_unchecked(buffer: T) -> Self {
        Self { buffer }
    }

    fn check_length(&self) -> Result<(), BufferTooSmall> {
        if self.buffer.as_ref().len() < ICMP_PACKET {
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

    pub fn icmp_type(&self) -> IcmpType {
        IcmpType::from_bytes(&self.buffer.as_ref()[field::TYPE])
    }

    pub fn code(&self) -> u8 {
        self.buffer.as_ref()[field::CODE]
    }

    pub fn checksum(&self) -> u16 {
        let mut destination = [0; size_of::<u16>()];
        destination.copy_from_slice(&self.buffer.as_ref()[field::CHECKSUM]);
        u16::from_be_bytes(destination)
    }

    pub fn is_echo_request(&self) -> bool {
        self.buffer.as_ref().len() >= ECHO_PACKET
            && self.icmp_type() == IcmpType::EchoRequest
            && self.code() == 0
    }

    pub fn is_echo_reply(&self) -> bool {
        self.buffer.as_ref().len() >= ECHO_PACKET
            && self.icmp_type() == IcmpType::EchoReply
            && self.code() == 0
    }

    pub fn echo_identifier(&self) -> u16 {
        let mut destination = [0; size_of::<u16>()];
        destination.copy_from_slice(&self.buffer.as_ref()[field::ECHO_IDENTIFIER]);
        u16::from_be_bytes(destination)
    }

    pub fn echo_sequence(&self) -> u16 {
        let mut destination = [0; size_of::<u16>()];
        destination.copy_from_slice(&self.buffer.as_ref()[field::ECHO_SEQUENCE]);
        u16::from_be_bytes(destination)
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> ICMPPacket<T> {
    pub fn set_icmp_type(&mut self, icmp_type: IcmpType) -> &mut Self {
        self.buffer.as_mut()[field::TYPE].copy_from_slice(&icmp_type.as_bytes());
        self
    }

    pub fn set_code(&mut self, code: u8) -> &mut Self {
        self.buffer.as_mut()[field::CODE] = code;
        self
    }

    pub fn compute_checksum(&mut self) -> &mut Self {
        self.buffer.as_mut()[field::CHECKSUM].copy_from_slice(&[0, 0]);
        let csum = checksum(self.buffer.as_ref());
        self.buffer.as_mut()[field::CHECKSUM].copy_from_slice(&csum.to_be_bytes());
        self
    }

    // pub fn set_checksum(&mut self, checksum: u16) -> &mut Self {
    //     self.buffer.as_mut()[field::CHECKSUM].copy_from_slice(&checksum.to_be_bytes());
    //     self
    // }

    pub fn set_echo_identifier(&mut self, identifier: u16) -> &mut Self {
        self.buffer.as_mut()[field::ECHO_IDENTIFIER].copy_from_slice(&identifier.to_be_bytes());
        self
    }

    pub fn set_echo_sequence(&mut self, sequence: u16) -> &mut Self {
        self.buffer.as_mut()[field::ECHO_SEQUENCE].copy_from_slice(&sequence.to_be_bytes());
        self
    }
}
