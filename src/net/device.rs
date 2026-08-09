use crate::net::ethernet::address::EthernetAddress;

pub trait NetworkDevice {
    fn send_packet(&self, buffer: &[u8]);
    fn hardware_address(&self) -> EthernetAddress;
}
