mod header;
pub mod protocol;
pub mod sequence;

use core::ops::Not;

use crate::net::error::BufferTooSmall;
use crate::net::ipv4::address::IPv4Address;
use crate::net::tcp::header::compute_checksum;
use crate::net::tcp::sequence::Sequence;
use crate::print::colors::Colorable;

mod field {
    pub const SOURCE: core::ops::Range<usize> = 0..2;
    pub const DESTINATION: core::ops::Range<usize> = 2..4;
    pub const SEQ: core::ops::Range<usize> = 4..8;
    pub const ACK: core::ops::Range<usize> = 8..12;
    pub const DATA_OFFSET: usize = 12;
    pub const FLAGS: usize = 13;
    pub const WINDOW: core::ops::Range<usize> = 14..16;
    pub const CHECKSUM: core::ops::Range<usize> = 16..18;
    pub const URGENT: core::ops::Range<usize> = 18..20;
}

mod flags {
    pub const CWR: u8 = 0b1000_0000;
    pub const ECE: u8 = 0b0100_0000;
    pub const URG: u8 = 0b0010_0000;
    pub const ACK: u8 = 0b0001_0000;
    pub const PSH: u8 = 0b0000_1000;
    pub const RST: u8 = 0b0000_0100;
    pub const SYN: u8 = 0b0000_0010;
    pub const FIN: u8 = 0b0000_0001;
}

pub struct TCPPacket<T: AsRef<[u8]>> {
    buffer: T,
}

pub const TCP_HEADER: usize = 20;

impl<T: AsRef<[u8]>> TCPPacket<T> {
    pub fn new_unchecked(buffer: T) -> Self {
        Self { buffer }
    }

    fn check_length(&self) -> Result<(), BufferTooSmall> {
        if self.buffer.as_ref().len() < TCP_HEADER {
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
        u16::from_be_bytes(
            self.buffer.as_ref()[field::SOURCE]
                .try_into()
                .expect("SOURCE to be 2 bytes"),
        )
    }

    pub fn destination(&self) -> u16 {
        u16::from_be_bytes(
            self.buffer.as_ref()[field::DESTINATION]
                .try_into()
                .expect("DESTINATION to be 2 bytes"),
        )
    }

    pub fn sequence(&self) -> Sequence {
        u32::from_be_bytes(
            self.buffer.as_ref()[field::SEQ]
                .try_into()
                .expect("SEQ to be 4 bytes"),
        )
        .into()
    }

    pub fn acknowledgment(&self) -> Sequence {
        u32::from_be_bytes(
            self.buffer.as_ref()[field::ACK]
                .try_into()
                .expect("ACK to be 4 bytes"),
        )
        .into()
    }

    pub fn data_offset_and_reserved(&self) -> u8 {
        self.buffer.as_ref()[field::DATA_OFFSET]
    }

    pub fn cwr(&self) -> bool {
        self.buffer.as_ref()[field::FLAGS] & flags::CWR != 0
    }

    pub fn ece(&self) -> bool {
        self.buffer.as_ref()[field::FLAGS] & flags::ECE != 0
    }

    pub fn urg(&self) -> bool {
        self.buffer.as_ref()[field::FLAGS] & flags::URG != 0
    }

    pub fn ack(&self) -> bool {
        self.buffer.as_ref()[field::FLAGS] & flags::ACK != 0
    }

    pub fn psh(&self) -> bool {
        self.buffer.as_ref()[field::FLAGS] & flags::PSH != 0
    }

    pub fn rst(&self) -> bool {
        self.buffer.as_ref()[field::FLAGS] & flags::RST != 0
    }

    pub fn syn(&self) -> bool {
        self.buffer.as_ref()[field::FLAGS] & flags::SYN != 0
    }

    pub fn fin(&self) -> bool {
        self.buffer.as_ref()[field::FLAGS] & flags::FIN != 0
    }

    pub fn window(&self) -> u16 {
        u16::from_be_bytes(
            self.buffer.as_ref()[field::WINDOW]
                .try_into()
                .expect("WINDOW to be 2 bytes"),
        )
    }

    pub fn checksum(&self) -> u16 {
        u16::from_be_bytes(
            self.buffer.as_ref()[field::CHECKSUM]
                .try_into()
                .expect("CHECKSUM to be 2 bytes"),
        )
    }

    pub fn urgent(&self) -> u16 {
        u16::from_be_bytes(
            self.buffer.as_ref()[field::URGENT]
                .try_into()
                .expect("URGENT to be 2 bytes"),
        )
    }

    pub fn payload(&self) -> &[u8] {
        let payload_begin = (self.data_offset_and_reserved() as usize >> 4) * 4;
        &self.buffer.as_ref()[payload_begin..]
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> TCPPacket<T> {
    pub fn set_source(&mut self, source_port: u16) -> &mut Self {
        self.buffer.as_mut()[field::SOURCE].copy_from_slice(&source_port.to_be_bytes());
        self
    }

    pub fn set_destination(&mut self, destination_port: u16) -> &mut Self {
        self.buffer.as_mut()[field::DESTINATION].copy_from_slice(&destination_port.to_be_bytes());
        self
    }

    pub fn set_sequence(&mut self, seq: Sequence) -> &mut Self {
        self.buffer.as_mut()[field::SEQ].copy_from_slice(&Into::<u32>::into(seq).to_be_bytes());
        self
    }

    pub fn set_acknowledgment(&mut self, ack: Sequence) -> &mut Self {
        self.buffer.as_mut()[field::ACK].copy_from_slice(&Into::<u32>::into(ack).to_be_bytes());
        self
    }

    pub fn set_data_offset_and_reserved(&mut self) -> &mut Self {
        self.buffer.as_mut()[field::DATA_OFFSET] = 5 << 4;
        self
    }

    fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.buffer.as_mut()[field::FLAGS] |= flag;
        } else {
            self.buffer.as_mut()[field::FLAGS] &= flag.not();
        }
    }

    pub fn set_cwr(&mut self, value: bool) -> &mut Self {
        self.set_flag(flags::CWR, value);
        self
    }

    pub fn set_ece(&mut self, value: bool) -> &mut Self {
        self.set_flag(flags::ECE, value);
        self
    }

    pub fn set_urg(&mut self, value: bool) -> &mut Self {
        self.set_flag(flags::URG, value);
        self
    }

    pub fn set_ack(&mut self, value: bool) -> &mut Self {
        self.set_flag(flags::ACK, value);
        self
    }

    pub fn set_psh(&mut self, value: bool) -> &mut Self {
        self.set_flag(flags::PSH, value);
        self
    }

    pub fn set_rst(&mut self, value: bool) -> &mut Self {
        self.set_flag(flags::RST, value);
        self
    }

    pub fn set_syn(&mut self, value: bool) -> &mut Self {
        self.set_flag(flags::SYN, value);
        self
    }

    pub fn set_fin(&mut self, value: bool) -> &mut Self {
        self.set_flag(flags::FIN, value);
        self
    }

    pub fn set_window(&mut self, window: u16) -> &mut Self {
        self.buffer.as_mut()[field::WINDOW].copy_from_slice(&window.to_be_bytes());
        self
    }

    pub fn set_checksum(&mut self, checksum: u16) -> &mut Self {
        self.buffer.as_mut()[field::CHECKSUM].copy_from_slice(&checksum.to_be_bytes());
        self
    }

    pub fn compute_checksum(&mut self, source: IPv4Address, destination: IPv4Address) -> &mut Self {
        self.set_checksum(0);
        let checksum = compute_checksum(source, destination, self);
        self.set_checksum(checksum)
    }

    pub fn set_urgent(&mut self, urgent: u16) -> &mut Self {
        self.buffer.as_mut()[field::URGENT].copy_from_slice(&urgent.to_be_bytes());
        self
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        let payload_begin = (self.data_offset_and_reserved() as usize >> 4) * 4;
        &mut self.buffer.as_mut()[payload_begin..]
    }
}

impl<T: AsRef<[u8]>> core::fmt::Display for TCPPacket<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "TCP(source={}, destination={}, seq={}, ack={}, flags=",
            self.source().blue(),
            self.destination().bright_blue(),
            self.sequence().magenta(),
            self.acknowledgment().bright_magenta(),
        ))?;

        if self.cwr() {
            f.write_str("C")?;
        }
        if self.ece() {
            f.write_str("E")?;
        }
        if self.urg() {
            f.write_str("U")?;
        }
        if self.ack() {
            "A".bright_magenta().fmt(f)?;
        }
        if self.psh() {
            "P".magenta().fmt(f)?;
        }
        if self.rst() {
            "R".red().fmt(f)?;
        }
        if self.syn() {
            "S".green().fmt(f)?;
        }
        if self.fin() {
            "F".bright_red().fmt(f)?;
        }

        f.write_fmt(format_args!(", payload={} bytes)", self.payload().len()))
    }
}
