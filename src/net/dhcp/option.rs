extern crate alloc;

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use crate::net::error::BufferTooSmall;
use crate::net::ipv4::address::IPv4Address;
use crate::net::ipv4::mask::IPv4Mask;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum MessageType {
    Discover = 1,
    Offer = 2,
    Request = 3,
    Decline = 4,
    Ack = 5,
    Nak = 6,
    Release = 7,
    Inform = 8,
    Unknown(u8),
}

impl From<u8> for MessageType {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Discover,
            2 => Self::Offer,
            3 => Self::Request,
            4 => Self::Decline,
            5 => Self::Ack,
            6 => Self::Nak,
            7 => Self::Release,
            8 => Self::Inform,
            code => Self::Unknown(code),
        }
    }
}

impl TryInto<MessageType> for &[u8] {
    type Error = DHCPOptionError;

    fn try_into(self) -> Result<MessageType, Self::Error> {
        match self.first() {
            Some(code) if self.len() == 1 => Ok((*code).into()),
            _ => Err(DHCPOptionError::InvalidSize),
        }
    }
}

impl From<MessageType> for u8 {
    fn from(value: MessageType) -> Self {
        match value {
            MessageType::Discover => 1,
            MessageType::Offer => 2,
            MessageType::Request => 3,
            MessageType::Decline => 4,
            MessageType::Ack => 5,
            MessageType::Nak => 6,
            MessageType::Release => 7,
            MessageType::Inform => 8,
            MessageType::Unknown(code) => code,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum ParameterRequest {
    SubnetMask = 1,
    Router = 3,
    DomainNameServer = 6,
    DomainName = 15,
    DomainSearch = 119,
    Unknown(u8),
}

impl From<ParameterRequest> for u8 {
    fn from(value: ParameterRequest) -> Self {
        match value {
            ParameterRequest::SubnetMask => 1,
            ParameterRequest::Router => 3,
            ParameterRequest::DomainNameServer => 6,
            ParameterRequest::DomainName => 15,
            ParameterRequest::DomainSearch => 119,
            ParameterRequest::Unknown(parameter) => parameter,
        }
    }
}

#[derive(Debug)]
pub enum DHCPOptionError {
    InvalidSize,
    NotAnOption,
    Utf8Error(),
}

#[derive(Debug)]
pub enum DHCPOption {
    Mask(IPv4Mask),
    Router(IPv4Address),
    Dns(IPv4Address),
    Hostname(String),
    ParameterRequestList(Vec<ParameterRequest>),
    RequestedAddress(IPv4Address),
    LeaseTime(Duration),
    MessageType(MessageType),
    ServerIdentifier(IPv4Address),
    End,
    Unknown(u8, Vec<u8>),
}

impl DHCPOption {
    pub fn new(code: u8, content: &[u8]) -> Result<DHCPOption, DHCPOptionError> {
        match code {
            0 => Err(DHCPOptionError::NotAnOption),
            1 => Ok(DHCPOption::Mask(
                content
                    .try_into()
                    .map_err(|_| DHCPOptionError::InvalidSize)?,
            )),
            3 => Ok(DHCPOption::Router(
                content
                    .try_into()
                    .map_err(|_| DHCPOptionError::InvalidSize)?,
            )),
            6 => Ok(DHCPOption::Dns(
                content
                    .try_into()
                    .map_err(|_| DHCPOptionError::InvalidSize)?,
            )),
            12 => Ok(DHCPOption::Hostname(
                String::from_utf8_lossy(content).to_string(),
            )),
            51 => content
                .try_into()
                .map(|bytes| u32::from_be_bytes(bytes) as u64)
                .map(|seconds| DHCPOption::LeaseTime(Duration::from_secs(seconds)))
                .map_err(|_| DHCPOptionError::InvalidSize),
            53 => Ok(DHCPOption::MessageType(content.try_into()?)),
            54 => Ok(DHCPOption::ServerIdentifier(
                content
                    .try_into()
                    .map_err(|_| DHCPOptionError::InvalidSize)?,
            )),
            255 => Ok(DHCPOption::End),
            _ => Ok(DHCPOption::Unknown(code, Vec::from(content))),
        }
    }

    pub fn as_bytes(&self) -> Vec<u8> {
        match self {
            DHCPOption::MessageType(message_type) => vec![53, 1, (*message_type).into()],
            DHCPOption::Hostname(hostname) => {
                let mut result = vec![12, hostname.len() as u8];
                result.extend_from_slice(hostname.as_bytes());
                result
            }
            DHCPOption::ServerIdentifier(id) => {
                let mut result = vec![54, id.as_bytes().len() as u8];
                result.extend_from_slice(id.as_bytes());
                result
            }
            DHCPOption::ParameterRequestList(parameters) => {
                let mut result = vec![55, parameters.len() as u8];
                result.extend_from_slice(
                    &parameters
                        .iter()
                        .map(|parameter| (*parameter).into())
                        .collect::<Vec<u8>>(),
                );
                result
            }
            DHCPOption::RequestedAddress(address) => {
                let mut result = vec![50, address.as_bytes().len() as u8];
                result.extend_from_slice(address.as_bytes());
                result
            }

            DHCPOption::End => vec![255],
            DHCPOption::Unknown(code, content) => {
                let mut result = vec![*code, content.len() as u8];
                result.extend_from_slice(content);
                result
            }
            _ => unimplemented!(),
        }
    }
}

pub struct Options<T: AsRef<[u8]>> {
    buffer: T,
    cursor: usize,
}

impl<T: AsRef<[u8]>> Options<T> {
    pub fn new(buffer: T) -> Self {
        Self { buffer, cursor: 0 }
    }

    pub fn inner(self) -> T {
        self.buffer
    }

    pub fn get_mask(mut self) -> Option<IPv4Mask> {
        self.find_map(|option| match option {
            DHCPOption::Mask(mask) => Some(mask),
            _ => None,
        })
    }

    pub fn get_message_type(mut self) -> Option<MessageType> {
        self.find_map(|option| match option {
            DHCPOption::MessageType(message_type) => Some(message_type),
            _ => None,
        })
    }

    pub fn get_lease_time(mut self) -> Option<Duration> {
        self.find_map(|option| match option {
            DHCPOption::LeaseTime(duration) => Some(duration),
            _ => None,
        })
    }

    pub fn get_router(mut self) -> Option<IPv4Address> {
        self.find_map(|option| match option {
            DHCPOption::Router(router) => Some(router),
            _ => None,
        })
    }

    pub fn get_dns(mut self) -> Option<IPv4Address> {
        self.find_map(|option| match option {
            DHCPOption::Dns(dns) => Some(dns),
            _ => None,
        })
    }

    pub fn get_server_identifier(mut self) -> Option<IPv4Address> {
        self.find_map(|option| match option {
            DHCPOption::ServerIdentifier(id) => Some(id),
            _ => None,
        })
    }
}

impl<T: AsRef<[u8]>> Iterator for Options<T> {
    type Item = DHCPOption;

    fn next(&mut self) -> Option<Self::Item> {
        let buffer = self.buffer.as_ref();

        buffer.get(self.cursor + 1)?;

        let code = buffer[self.cursor];
        let length = buffer[self.cursor + 1] as usize;

        buffer.get(self.cursor + length)?;

        let content = &buffer[self.cursor + 2..self.cursor + 2 + length];

        self.cursor += 2 + length;

        DHCPOption::new(code, content).ok()
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> Options<T> {
    pub fn append(&mut self, option: DHCPOption) -> Result<&mut Self, BufferTooSmall> {
        let bytes = option.as_bytes();
        if self.cursor + bytes.len() > self.buffer.as_ref().len() {
            Err(BufferTooSmall)?
        }

        self.buffer.as_mut()[self.cursor..self.cursor + bytes.len()].copy_from_slice(&bytes);
        self.cursor += bytes.len();
        Ok(self)
    }
}

pub fn options_to_vec(options: &[DHCPOption]) -> Vec<u8> {
    options
        .iter()
        .flat_map(|option| option.as_bytes())
        .collect()
}
