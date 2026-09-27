pub mod listen;

mod tcp;
mod udp;

pub use tcp::ConnectionPool as TCPConnectionPool;
pub use tcp::Error as TCPError;
pub use tcp::Socket as TCPSocket;
pub use tcp::handle_pending_closes as cleanup_task;
pub use udp::ListenerPool as UDPListenerPool;
pub use udp::Message as UDPMessage;
pub use udp::Socket as UDPSocket;

use crate::net::STATE_MACHINE;
use crate::net::ipv4::address::IPv4Address;

fn get_my_ipv4_address() -> Option<IPv4Address> {
    let state = STATE_MACHINE.lock();
    state.arp.identity.map(|id| id.1)
}
