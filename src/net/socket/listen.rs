use crate::net::ipv4::address::IPv4Address;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Listen {
    AnyAddress(u16),
    SpecificAddress(IPv4Address, u16),
}
