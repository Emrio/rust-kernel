use crate::net::checksum::Checksum;
use crate::net::ipv4::address::IPv4Address;
use crate::net::ipv4::protocol::Protocol::TCP;
use crate::net::tcp::TCPPacket;

pub fn compute_checksum<T: AsRef<[u8]>>(
    source: IPv4Address,
    destination: IPv4Address,
    tcp: &TCPPacket<T>,
) -> u16 {
    let mut checksum = Checksum::new();

    // pseudo-header
    checksum.feed(source.as_bytes());
    checksum.feed(destination.as_bytes());
    checksum.feed(&[0, TCP as u8]);
    checksum.feed(&(tcp.buffer.as_ref().len() as u16).to_be_bytes());

    // tcp header + payload
    checksum.feed(tcp.buffer.as_ref());

    checksum.finish()
}
