use crate::net::ipv4::address::{IPv4Address, IPv4AddressParseError};
use crate::net::port::Port;
use crate::print::colors::Colorable;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Listen {
    AnyAddress(Port),
    SpecificAddress(IPv4Address, Port),
}

impl Listen {
    pub fn port(&self) -> Port {
        match self {
            Listen::AnyAddress(port) | Listen::SpecificAddress(_, port) => *port,
        }
    }

    pub fn address(&self) -> Option<IPv4Address> {
        match self {
            Listen::SpecificAddress(address, _) => Some(*address),
            _ => None,
        }
    }
}

pub enum ListenParseError {
    MissingSemicolon,
    InvalidIPv4Address(IPv4AddressParseError),
    InvalidPort,
}

impl core::str::FromStr for Listen {
    type Err = ListenParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((address, port)) = s.split_once(":") else {
            Err(ListenParseError::MissingSemicolon)?
        };

        let address = address
            .parse::<IPv4Address>()
            .map_err(ListenParseError::InvalidIPv4Address)?;

        let port = port.parse().map_err(|_| ListenParseError::InvalidPort)?;

        if address == IPv4Address::default() {
            Ok(Listen::AnyAddress(port))
        } else {
            Ok(Listen::SpecificAddress(address, port))
        }
    }
}

impl core::fmt::Display for Listen {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let (address, port) = match self {
            Listen::AnyAddress(port) => (&IPv4Address::default(), port),
            Listen::SpecificAddress(address, port) => (address, port),
        };
        f.write_fmt(format_args!("{}:{}", address.green(), port.blue()))
    }
}

impl From<Port> for Listen {
    fn from(port: Port) -> Self {
        Self::AnyAddress(port)
    }
}

impl From<u16> for Listen {
    fn from(port: u16) -> Self {
        Self::from(Into::<Port>::into(port))
    }
}
