pub mod address;
pub mod protocol;
pub mod ttl;

use crate::net::checksum::checksum;
use crate::net::error::BufferTooSmall;
use crate::net::ipv4::{address::IPv4Address, protocol::Protocol, ttl::TimeToLive};

pub struct IPv4Packet<T: AsRef<[u8]>> {
    buffer: T,
}

mod field {
    pub const VERSION: usize = 0; // first 4 bits
    pub const IHL: usize = 0; // last 4 bits
    // pub const TOS: usize = 1;
    pub const TOTAL_LENGTH: core::ops::Range<usize> = 2..4;
    // pub const ID: core::ops::Range<usize> = 4..6;
    // pub const FLAGS: usize = 6; // first 3 bits
    // pub const FRAGMENT_OFFSET: core::ops::Range<usize> = 6..8; // last 13 bits
    pub const TTL: usize = 8;
    pub const PROTOCOL: core::ops::Range<usize> = 9..10;
    pub const CHECKSUM: core::ops::Range<usize> = 10..12;
    pub const SOURCE: core::ops::Range<usize> = 12..16;
    pub const DESTINATION: core::ops::Range<usize> = 16..20;
}

pub const IPV4_PACKET: usize = 20;

impl<T: AsRef<[u8]>> IPv4Packet<T> {
    pub fn new_unchecked(buffer: T) -> Self {
        Self { buffer }
    }

    fn check_length(&self) -> Result<(), BufferTooSmall> {
        if self.buffer.as_ref().len() < IPV4_PACKET {
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

    pub fn version(&self) -> u8 {
        self.buffer.as_ref()[field::VERSION] >> 4
    }

    pub fn header_length(&self) -> u8 {
        self.buffer.as_ref()[field::IHL] & 0x0f
    }

    pub fn header_length_bytes(&self) -> usize {
        self.header_length() as usize * 4
    }

    pub fn packet_length(&self) -> usize {
        let mut destination = [0; size_of::<u16>()];
        destination.copy_from_slice(&self.buffer.as_ref()[field::TOTAL_LENGTH]);
        let size = u16::from_be_bytes(destination);
        size as usize
    }

    pub fn protocol(&self) -> Protocol {
        Protocol::from_bytes(&self.buffer.as_ref()[field::PROTOCOL])
    }

    pub fn ttl(&self) -> TimeToLive {
        TimeToLive::new(self.buffer.as_ref()[field::TTL])
    }

    pub fn destination(&self) -> IPv4Address {
        IPv4Address::from_bytes(&self.buffer.as_ref()[field::DESTINATION])
    }

    pub fn source(&self) -> IPv4Address {
        IPv4Address::from_bytes(&self.buffer.as_ref()[field::SOURCE])
    }

    pub fn payload(&self) -> &[u8] {
        let range = self.header_length_bytes()..self.packet_length();
        &self.buffer.as_ref()[range]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> IPv4Packet<T> {
    pub fn set_version_and_length(&mut self) -> &mut Self {
        self.buffer.as_mut()[field::VERSION] = (4 << 4) + 5;
        self
    }

    pub fn set_packet_length(&mut self, length: usize) -> &mut Self {
        assert!(
            length <= u16::MAX as usize,
            "IPv4 packet max length is 65 535 bytes."
        );
        let length = (length as u16).to_be_bytes();
        self.buffer.as_mut()[field::TOTAL_LENGTH].copy_from_slice(&length);
        self
    }

    pub fn set_protocol(&mut self, protcol: Protocol) -> &mut Self {
        self.buffer.as_mut()[field::PROTOCOL].copy_from_slice(&protcol.as_bytes());
        self
    }

    pub fn set_destination(&mut self, address: IPv4Address) -> &mut Self {
        self.buffer.as_mut()[field::DESTINATION].copy_from_slice(address.as_bytes());
        self
    }

    pub fn set_source(&mut self, address: IPv4Address) -> &mut Self {
        self.buffer.as_mut()[field::SOURCE].copy_from_slice(address.as_bytes());
        self
    }

    pub fn set_ttl(&mut self, ttl: TimeToLive) -> &mut Self {
        self.buffer.as_mut()[field::TTL] = ttl.as_u8();
        self
    }

    pub fn compute_checksum(&mut self) -> &mut Self {
        self.buffer.as_mut()[field::CHECKSUM].copy_from_slice(&[0, 0]);
        let csum = checksum(&self.buffer.as_ref()[..self.header_length_bytes()]);
        self.buffer.as_mut()[field::CHECKSUM].copy_from_slice(&csum.to_be_bytes());
        self
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        let range = self.header_length_bytes()..self.packet_length();
        &mut self.buffer.as_mut()[range]
    }
}

impl<T: AsRef<[u8]>> core::fmt::Display for IPv4Packet<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "IPv4(source={}, destination={}, protocol={}, ttl={})",
            self.source(),
            self.destination(),
            self.protocol(),
            self.ttl()
        ))
    }
}
