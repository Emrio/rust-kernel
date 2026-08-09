extern crate alloc;

use alloc::vec::Vec;

use crate::net::ethernet::address::EthernetAddress;

pub trait NetworkDevice {
    fn send_packet(&self, buffer: &[u8]);
    fn poll_packet(&self) -> Option<Vec<u8>>;
    fn hardware_address(&self) -> EthernetAddress;
}
