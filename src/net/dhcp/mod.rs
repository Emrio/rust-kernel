pub mod operation;
pub mod option;
#[cfg(test)]
mod tests;

use crate::net::arp::HardwareType;
use crate::net::dhcp::operation::Operation;
use crate::net::dhcp::option::Options;
use crate::net::error::BufferTooSmall;
use crate::net::ethernet::address::{ADDRESS_SIZE, EthernetAddress};
use crate::net::ipv4::address::IPv4Address;
use crate::print::colors::Colorable;

mod field {
    pub const OPERATION: usize = 0;
    pub const HW_TYPE: usize = 1;
    pub const HW_LENGTH: usize = 2;
    pub const HOPS: usize = 3;
    pub const XID: core::ops::Range<usize> = 4..8;
    pub const SECS: core::ops::Range<usize> = 8..10;
    pub const FLAGS: core::ops::Range<usize> = 10..12;
    pub const CIADDR: core::ops::Range<usize> = 12..16;
    pub const YIADDR: core::ops::Range<usize> = 16..20;
    pub const SIADDR: core::ops::Range<usize> = 20..24;
    pub const GIADDR: core::ops::Range<usize> = 24..28;
    pub const CHADDR: core::ops::Range<usize> = 28..44;
    pub const SNAME: core::ops::Range<usize> = 44..108;
    pub const FILE_NAME: core::ops::Range<usize> = 108..236;
    pub const MAGIC_COOKIE: core::ops::Range<usize> = 236..240;
    pub const OPTIONS: core::ops::RangeFrom<usize> = 240..;
}

pub mod ports {
    pub const SERVER: u16 = 67;
    pub const CLIENT: u16 = 68;
}

pub struct DHCPPacket<T: AsRef<[u8]>> {
    buffer: T,
}

pub const DHCP_HEADER: usize = 240;

const MAGIC_COOKIE: u32 = 0x63825363;

impl<T: AsRef<[u8]>> DHCPPacket<T> {
    pub fn new_unchecked(buffer: T) -> Self {
        Self { buffer }
    }

    fn check_length(&self) -> Result<(), BufferTooSmall> {
        if self.buffer.as_ref().len() < DHCP_HEADER {
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

    pub fn operation(&self) -> Operation {
        Operation::from_bytes(&[self.buffer.as_ref()[field::OPERATION]])
    }

    pub fn hardware_type(&self) -> HardwareType {
        HardwareType::from_bytes(&[0, self.buffer.as_ref()[field::HW_TYPE]])
    }

    pub fn hardware_length(&self) -> usize {
        self.buffer.as_ref()[field::HW_LENGTH] as usize
    }

    pub fn xid(&self) -> u32 {
        match self.buffer.as_ref()[field::XID].try_into() {
            Ok(bytes) => u32::from_be_bytes(bytes),
            Err(_) => unreachable!("dhcp::XID size should be 4 bytes"),
        }
    }

    pub fn secs(&self) -> u16 {
        match self.buffer.as_ref()[field::SECS].try_into() {
            Ok(bytes) => u16::from_be_bytes(bytes),
            Err(_) => unreachable!("dhcp::SECS size should be 2 bytes"),
        }
    }

    pub fn flags(&self) -> u16 {
        match self.buffer.as_ref()[field::FLAGS].try_into() {
            Ok(bytes) => u16::from_be_bytes(bytes),
            Err(_) => unreachable!("dhcp::FLAGS size should be 2 bytes"),
        }
    }

    pub fn client_address(&self) -> IPv4Address {
        IPv4Address::from_bytes(&self.buffer.as_ref()[field::CIADDR])
    }

    pub fn your_address(&self) -> IPv4Address {
        IPv4Address::from_bytes(&self.buffer.as_ref()[field::YIADDR])
    }

    pub fn server_address(&self) -> IPv4Address {
        IPv4Address::from_bytes(&self.buffer.as_ref()[field::SIADDR])
    }

    pub fn relay_address(&self) -> IPv4Address {
        IPv4Address::from_bytes(&self.buffer.as_ref()[field::GIADDR])
    }

    pub fn hardware_address(&self) -> EthernetAddress {
        assert_eq!(
            self.hardware_type(),
            HardwareType::Ethernet,
            "This implementation only supports Ethernet addresses"
        );
        assert_eq!(self.hardware_length(), ADDRESS_SIZE);
        EthernetAddress::from_bytes(
            &self.buffer.as_ref()[field::CHADDR.start..ADDRESS_SIZE + field::CHADDR.start],
        )
    }

    pub fn server_name(&self) -> Result<&str, core::str::Utf8Error> {
        let buffer = &self.buffer.as_ref()[field::SNAME];
        let nul_range_end = buffer
            .iter()
            .position(|&c| c == b'\0')
            .unwrap_or(buffer.len());

        core::str::from_utf8(&buffer[..nul_range_end])
    }

    pub fn file_name(&self) -> Result<&str, core::str::Utf8Error> {
        let buffer = &self.buffer.as_ref()[field::FILE_NAME];
        let nul_range_end = buffer
            .iter()
            .position(|&c| c == b'\0')
            .unwrap_or(buffer.len());

        core::str::from_utf8(&buffer[..nul_range_end])
    }

    pub fn is_dhcp(&self) -> bool {
        self.buffer.as_ref()[field::MAGIC_COOKIE] == MAGIC_COOKIE.to_be_bytes()
    }

    pub fn options(&self) -> Options<&[u8]> {
        Options::new(&self.buffer.as_ref()[field::OPTIONS])
    }
}

impl<T: AsRef<[u8]>> core::fmt::Display for DHCPPacket<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Some(message_type) = self.options().get_message_type() else {
            return f.write_str("BOOT()");
        };

        f.write_fmt(format_args!(
            "DHCP({}, xid=0x{:02x}",
            message_type.blue(),
            self.xid()
        ))?;

        if !self.your_address().is_zero() {
            f.write_fmt(format_args!(
                ", yaddr={}",
                self.your_address().bright_green()
            ))?;
        }

        for option in self.options() {
            match option {
                option::DHCPOption::Mask(mask) => {
                    f.write_fmt(format_args!(", mask={}", mask.green()))?
                }
                option::DHCPOption::Router(ipv4_address) => {
                    f.write_fmt(format_args!(", router={}", ipv4_address.green()))?
                }
                option::DHCPOption::Dns(ipv4_address) => {
                    f.write_fmt(format_args!(", dns={}", ipv4_address.green()))?
                }
                option::DHCPOption::Hostname(hostname) => {
                    f.write_fmt(format_args!(", hostname={}", hostname.bright_cyan()))?
                }
                option::DHCPOption::DomainName(domain) => {
                    f.write_fmt(format_args!(", domain={}", domain.cyan()))?
                }
                option::DHCPOption::ParameterRequestList(parameter_requests) => {
                    f.write_fmt(format_args!(", request_params={parameter_requests:?}"))?
                }
                option::DHCPOption::RequestedAddress(address) => {
                    f.write_fmt(format_args!(", request_address={}", address.bright_green()))?
                }
                option::DHCPOption::LeaseTime(duration) => {
                    f.write_fmt(format_args!(", lease_time={duration:?}"))?
                }
                option::DHCPOption::MessageType(_) => {}
                option::DHCPOption::ServerIdentifier(ipv4_address) => {
                    f.write_fmt(format_args!(", sid={}", ipv4_address.green()))?
                }
                option::DHCPOption::End => {}
                option::DHCPOption::Unknown(code, items) => {
                    f.write_fmt(format_args!(", unknown({})={items:?}", code.red()))?
                }
            }
        }

        f.write_str(")")
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> DHCPPacket<T> {
    pub fn set_operation(&mut self, operation: Operation) -> &mut Self {
        self.buffer.as_mut()[field::OPERATION] = operation.as_u8();
        self
    }

    pub fn set_hardware_type(&mut self, hardware_type: HardwareType) -> &mut Self {
        let [upper, lower] = hardware_type.as_bytes();
        assert_eq!(upper, 0);
        self.buffer.as_mut()[field::HW_TYPE] = lower;
        self
    }

    pub fn set_hardware_length(&mut self, length: usize) -> &mut Self {
        assert!(
            length <= u8::MAX as usize,
            "Hardware length max size is 255 bytes."
        );
        self.buffer.as_mut()[field::HW_LENGTH] = length as u8;
        self
    }

    pub fn set_hops(&mut self, hops: u8) -> &mut Self {
        self.buffer.as_mut()[field::HOPS] = hops;
        self
    }

    pub fn set_xid(&mut self, xid: u32) -> &mut Self {
        self.buffer.as_mut()[field::XID].copy_from_slice(&xid.to_be_bytes());
        self
    }

    pub fn set_secs(&mut self, secs: u16) -> &mut Self {
        self.buffer.as_mut()[field::SECS].copy_from_slice(&secs.to_be_bytes());
        self
    }

    pub fn set_flags(&mut self, flags: u16) -> &mut Self {
        self.buffer.as_mut()[field::FLAGS].copy_from_slice(&flags.to_be_bytes());
        self
    }

    pub fn set_hardware_address(&mut self, address: EthernetAddress) -> &mut Self {
        self.buffer.as_mut()[field::CHADDR.start..ADDRESS_SIZE + field::CHADDR.start]
            .copy_from_slice(address.as_bytes());
        self
    }

    pub fn set_server_name(&mut self, name: &str) -> &mut Self {
        assert!(name.len() <= field::SNAME.len());

        let name_range = field::SNAME.start..field::SNAME.start + name.len();
        let remaining_range = field::SNAME.start + name.len()..field::SNAME.end;

        self.buffer.as_mut()[name_range].copy_from_slice(name.as_bytes());
        self.buffer.as_mut()[remaining_range].fill(0);

        self
    }

    pub fn set_file_name(&mut self, name: &str) -> &mut Self {
        assert!(name.len() <= field::FILE_NAME.len());

        let name_range = field::FILE_NAME.start..field::FILE_NAME.start + name.len();
        let remaining_range = field::FILE_NAME.start + name.len()..field::FILE_NAME.end;

        self.buffer.as_mut()[name_range].copy_from_slice(name.as_bytes());
        self.buffer.as_mut()[remaining_range].fill(0);

        self
    }

    pub fn set_magic_cookie(&mut self) -> &mut Self {
        self.buffer.as_mut()[field::MAGIC_COOKIE].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
        self
    }

    pub fn options_mut(&mut self) -> Options<&mut [u8]> {
        Options::new(&mut self.buffer.as_mut()[field::OPTIONS])
    }
}
