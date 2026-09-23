extern crate alloc;

use alloc::vec::Vec;

use crate::net::socket::UDPSocket;
use crate::print::colors::Colorable;

fn make_pong(input: &[u8]) -> Vec<u8> {
    const PREFIX: &str = "Pong: ";
    let mut response = Vec::with_capacity(PREFIX.len() + input.len());
    response.extend_from_slice(PREFIX.as_bytes());
    response.extend_from_slice(input);
    response
}

pub async fn udp_pong_server(port: u16) -> () {
    let socket = UDPSocket::listen(port);

    loop {
        let message = socket.accept().await;

        klog!(
            "udp_pong_server",
            "Received message from ",
            message.remote_address().green(),
            ":",
            message.remote_port().blue(),
            "!"
        );

        if message.send(&make_pong(message.payload())).is_err() {
            klog!("udp_pong_server", "Pong failed".red());
        }
    }
}
