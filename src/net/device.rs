use crate::net::{ethernet::address::EthernetAddress, handle_incoming_ethernet_packet};

pub trait NetworkDevice {
    fn send_packet(&self, buffer: &[u8]);
    fn setup_device_rx(&mut self, handler_fn: &'static dyn Fn(&[u8]));
    fn hardware_address(&self) -> EthernetAddress;

    fn setup_handling(&mut self) {
        self.setup_device_rx(&handle_incoming_ethernet_packet);
    }
}
