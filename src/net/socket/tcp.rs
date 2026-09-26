extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;
use core::task::Poll;
use futures_util::task::AtomicWaker;
use lazy_static::lazy_static;

use crate::net::STATE_MACHINE;
use crate::net::handle::Handle;
use crate::net::ipv4::IPv4Packet;
use crate::net::ipv4::address::IPv4Address;
use crate::net::ipv4::protocol::Protocol;
use crate::net::socket::listen::Listen;
use crate::net::tcp::TCPPacket;
use crate::net::tcp::protocol::AcceptResult;
use crate::net::tcp::protocol::{Id, TransmissionControlBlock, generate_rst};
use crate::net::tx;
use crate::net::tx::L3;
use crate::net::tx::L4;
use crate::net::tx::NetworkError;
use crate::print::colors::Colorable;

pub struct Socket;

impl Socket {
    pub fn listen(listen: impl Into<Listen>) -> BoundSocket {
        let listen = listen.into();
        let handle = Arc::new(Handle::new(16));
        STATE_MACHINE.lock().tcp.listen(listen, handle.clone());
        klog!("tcp", "Listening on ", listen);
        BoundSocket { listen, handle }
    }

    pub fn connect(_address: IPv4Address, _port: u16) -> Connection {
        unimplemented!()
    }
}

pub struct ConnectionAccept {
    handle: Arc<Handle<Connection>>,
}

impl Future for ConnectionAccept {
    type Output = Connection;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        if let Some(message) = self.handle.queue.pop() {
            return Poll::Ready(message);
        }

        self.handle.waker.register(cx.waker());

        match self.handle.queue.pop() {
            Some(message) => Poll::Ready(message),
            None => Poll::Pending,
        }
    }
}

pub struct BoundSocket {
    listen: Listen,
    handle: Arc<Handle<Connection>>,
}

impl BoundSocket {
    pub fn accept(&self) -> ConnectionAccept {
        ConnectionAccept {
            handle: self.handle.clone(),
        }
    }
}

impl Drop for BoundSocket {
    fn drop(&mut self) {
        STATE_MACHINE.lock().tcp.remove_listen(&self.listen);
    }
}

struct ByteStream {
    buffer: spin::Mutex<VecDeque<u8>>,
    waker: AtomicWaker,
    closed: AtomicBool,
    peer_closed: AtomicBool,
}

pub struct Connection {
    id: Id,
    stream: Arc<ByteStream>,
}

pub struct Receive {
    stream: Arc<ByteStream>,
}

impl Receive {
    fn try_receive(&self) -> Option<Result<Vec<u8>, Error>> {
        if self.stream.closed.load(Ordering::Acquire) {
            return Some(Err(Error::ConnectionClosed));
        }

        let mut buffer = self.stream.buffer.lock();
        if !buffer.is_empty() {
            return Some(Ok(buffer.drain(..).collect()));
        }
        drop(buffer);

        if self.stream.peer_closed.load(Ordering::Acquire) {
            return Some(Ok(Vec::new()));
        }

        None
    }
}

#[derive(Debug)]
pub enum Error {
    ConnectionClosed,
}

impl Future for Receive {
    type Output = Result<Vec<u8>, Error>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        if let Some(result) = self.try_receive() {
            return Poll::Ready(result);
        }

        self.stream.waker.register(cx.waker());

        self.try_receive().map(Poll::Ready).unwrap_or(Poll::Pending)
    }
}

impl Connection {
    fn new(id: Id) -> Self {
        Self {
            id,
            stream: Arc::new(ByteStream {
                buffer: spin::Mutex::new(VecDeque::with_capacity(4096)),
                waker: AtomicWaker::new(),
                closed: AtomicBool::new(false),
                peer_closed: AtomicBool::new(false),
            }),
        }
    }

    pub fn remote_address(&self) -> IPv4Address {
        self.id.2
    }

    pub fn remote_port(&self) -> u16 {
        self.id.3
    }

    pub async fn send(&self, buffer: &[u8]) -> Result<(), NetworkError> {
        let segment = {
            let mut state = STATE_MACHINE.lock();
            let connection = state
                .tcp
                .connections
                .get_mut(&self.id)
                .ok_or(NetworkError::Tcp(Error::ConnectionClosed))?;
            connection
                .tcb_mut()
                .send_data(buffer)
                .expect("buffer should not be too small")
        };

        tx::send_l3(tx::L3::IPv4 {
            source: self.id.0,
            destination: self.id.2,
            protocol: Protocol::TCP,
            next: L4::Buffer(segment),
        })
        .await
    }

    pub fn receive(&self) -> Receive {
        Receive {
            stream: self.stream.clone(),
        }
    }

    pub async fn close(&self) -> Result<(), NetworkError> {
        close(self.id).await
    }
}

async fn close(id: Id) -> Result<(), NetworkError> {
    let segment = {
        let mut state = STATE_MACHINE.lock();
        let Some(connection) = state.tcp.connections.get_mut(&id) else {
            return Ok(());
        };
        let Some(segment) = connection
            .tcb_mut()
            .close()
            .expect("buffer should not be too small")
        else {
            return Ok(());
        };
        segment
    };

    tx::send_l3(tx::L3::IPv4 {
        source: id.0,
        destination: id.2,
        protocol: Protocol::TCP,
        next: segment,
    })
    .await
}

impl Drop for Connection {
    fn drop(&mut self) {
        let pending_closes = PENDING_CLOSES.lock();
        let _ = pending_closes.queue.push(self.id);
        pending_closes.waker.wake();
    }
}

enum ConnectionStatus {
    HalfOpen(TransmissionControlBlock, Arc<Handle<Connection>>),
    Established(TransmissionControlBlock, Arc<ByteStream>),
}

impl ConnectionStatus {
    fn tcb(&self) -> &TransmissionControlBlock {
        match self {
            ConnectionStatus::HalfOpen(tcb, _) | ConnectionStatus::Established(tcb, _) => tcb,
        }
    }

    fn tcb_mut(&mut self) -> &mut TransmissionControlBlock {
        match self {
            ConnectionStatus::HalfOpen(tcb, _) | ConnectionStatus::Established(tcb, _) => tcb,
        }
    }
}

#[derive(Default)]
pub struct ConnectionPool {
    connections: BTreeMap<Id, ConnectionStatus>,
    listeners: BTreeMap<Listen, Arc<Handle<Connection>>>,
}

impl ConnectionPool {
    pub const fn new() -> Self {
        Self {
            connections: BTreeMap::new(),
            listeners: BTreeMap::new(),
        }
    }

    fn listen(&mut self, listen: Listen, handle: Arc<Handle<Connection>>) {
        self.listeners.insert(listen, handle);
    }

    fn remove_listen(&mut self, listen: &Listen) {
        self.listeners.remove(listen);
    }

    fn get_connection(
        &mut self,
        ip: &IPv4Packet<&[u8]>,
        tcp: &TCPPacket<&[u8]>,
    ) -> Option<&mut ConnectionStatus> {
        let id = Id(
            ip.destination(),
            tcp.destination(),
            ip.source(),
            tcp.source(),
        );
        if !self.connections.contains_key(&id) && tcp.syn() {
            let specific_listen = Listen::SpecificAddress(ip.destination(), tcp.destination());
            let any_listen = Listen::AnyAddress(tcp.destination());

            let handle = self
                .listeners
                .get(&specific_listen)
                .or_else(|| self.listeners.get(&any_listen))?;

            let connection = ConnectionStatus::HalfOpen(id.into(), handle.clone());
            self.connections.insert(id, connection);
        }

        self.connections.get_mut(&id)
    }

    fn process_result(&mut self, id: Id, result: AcceptResult) -> Option<L4> {
        let Some(connection) = self.connections.remove(&id) else {
            // this is bad news
            return None;
        };

        if result.destroyed {
            if let ConnectionStatus::Established(_, stream) = connection {
                stream.closed.store(true, Ordering::Release);
                stream.waker.wake();
            }

            return result.response;
        }

        let new_connection = match connection {
            ConnectionStatus::HalfOpen(tcb, handle) if result.established => {
                let connection = Connection::new(id);
                let stream = connection.stream.clone();

                if result.peer_closed {
                    connection.stream.peer_closed.store(true, Ordering::Release);
                }

                if handle.queue.push(connection).is_err() {
                    klog!("tcp", "Queue full!".red())
                }
                handle.waker.wake();

                ConnectionStatus::Established(tcb, stream)
            }
            ConnectionStatus::Established(tcb, stream) => {
                if let Some(received) = result.received {
                    stream.buffer.lock().extend(received);
                    stream.waker.wake();
                }

                if result.peer_closed {
                    stream.peer_closed.store(true, Ordering::Release);
                    stream.waker.wake();
                }

                ConnectionStatus::Established(tcb, stream)
            }
            connection => connection,
        };

        self.connections.insert(id, new_connection);

        result.response
    }

    pub(crate) fn accept(&mut self, ip: &IPv4Packet<&[u8]>, tcp: &TCPPacket<&[u8]>) -> Option<L3> {
        let Some(connection) = self.get_connection(ip, tcp) else {
            klog!(
                format_args!(
                    "tcp/{}/{}/{}/{}",
                    ip.destination(),
                    tcp.destination(),
                    ip.source(),
                    tcp.source()
                ),
                "Sending ",
                "RST".red()
            );
            return generate_rst(ip, tcp);
        };

        let result = connection
            .tcb_mut()
            .accept(tcp)
            .expect("buffer should not be too small");

        let id = connection.tcb().id();
        self.process_result(id, result).map(|next| L3::IPv4 {
            source: ip.destination(),
            destination: ip.source(),
            protocol: Protocol::TCP,
            next,
        })
    }
}

lazy_static! {
    static ref PENDING_CLOSES: spin::Mutex<Handle<Id>> = spin::Mutex::new(Handle::new(42));
}

struct PendingCloseWatcher;

impl Future for PendingCloseWatcher {
    type Output = ();

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let pending_closes = PENDING_CLOSES.lock();
        if !pending_closes.queue.is_empty() {
            return Poll::Ready(());
        }
        pending_closes.waker.register(cx.waker());
        drop(pending_closes);
        let pending_closes = PENDING_CLOSES.lock();
        if !pending_closes.queue.is_empty() {
            return Poll::Ready(());
        }
        Poll::Pending
    }
}

pub async fn handle_pending_closes() {
    loop {
        PendingCloseWatcher.await;

        let pending_closes = PENDING_CLOSES.lock();
        while let Some(id) = pending_closes.queue.pop() {
            let _ = close(id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::ipv4::IPV4_PACKET;
    use crate::net::ipv4::ttl::TimeToLive;
    use crate::net::tcp::TCP_HEADER;
    use crate::net::tcp::sequence::Sequence;
    use alloc::vec;

    fn local() -> IPv4Address {
        IPv4Address::new(10, 0, 0, 1)
    }
    const LOCAL_PORT: u16 = 4242;
    fn remote() -> IPv4Address {
        IPv4Address::new(10, 0, 0, 2)
    }
    const REMOTE_PORT: u16 = 1234;

    /// Builds a raw incoming TCP segment (as the remote peer would send it).
    #[allow(clippy::too_many_arguments)]
    fn segment(
        seq: u32,
        ack: u32,
        syn: bool,
        ack_flag: bool,
        fin: bool,
        rst: bool,
        payload: &[u8],
    ) -> Vec<u8> {
        let mut buffer = vec![0u8; TCP_HEADER + payload.len()];
        {
            let mut packet = TCPPacket::new(buffer.as_mut_slice()).unwrap();
            packet
                .set_source(REMOTE_PORT)
                .set_destination(LOCAL_PORT)
                .set_sequence(seq.into())
                .set_acknowledgment(ack.into())
                .set_syn(syn)
                .set_ack(ack_flag)
                .set_fin(fin)
                .set_rst(rst)
                .set_data_offset_and_reserved();
            packet.payload_mut().copy_from_slice(payload);
        }
        buffer
    }

    /// Wraps a raw TCP segment in an IPv4 header, as needed by `ConnectionPool::accept`.
    fn ip_frame(tcp_bytes: &[u8]) -> Vec<u8> {
        let mut buffer = vec![0u8; IPV4_PACKET + tcp_bytes.len()];
        {
            let mut ip = IPv4Packet::new(buffer.as_mut_slice()).unwrap();
            ip.set_version_and_length()
                .set_packet_length(IPV4_PACKET + tcp_bytes.len())
                .set_protocol(Protocol::TCP)
                .set_destination(local())
                .set_source(remote())
                .set_ttl(TimeToLive::max())
                .compute_checksum();
            ip.payload_mut().copy_from_slice(tcp_bytes);
        }
        buffer
    }

    fn listening_pool() -> ConnectionPool {
        let mut pool = ConnectionPool::default();
        pool.listen(Listen::AnyAddress(LOCAL_PORT), Arc::new(Handle::new(16)));
        pool
    }

    #[test_case]
    fn stray_non_syn_segment_does_not_create_a_zombie_connection() {
        let mut pool = listening_pool();

        // A stray ACK for a connection we never saw a SYN for (e.g. a late
        // retransmission from a connection that predates us listening).
        let tcp_bytes = segment(1000, 5000, false, true, false, false, &[]);
        let ip_bytes = ip_frame(&tcp_bytes);
        let ip = IPv4Packet::new(ip_bytes.as_slice()).unwrap();
        let tcp = TCPPacket::new(ip.payload()).unwrap();

        pool.accept(&ip, &tcp);

        assert!(
            pool.connections.is_empty(),
            "a non-SYN segment for an unknown connection must not spawn a zombie TCB"
        );
    }

    #[test_case]
    fn rst_reply_to_ack_segment_uses_incoming_ack_as_sequence() {
        let mut pool = listening_pool();

        // Exactly like the stray retransmitted ACK+PSH+FIN observed in the wild:
        // no SYN was ever seen for this connection.
        let tcp_bytes = segment(1000, 5000, false, true, true, false, b"stray!!");
        let ip_bytes = ip_frame(&tcp_bytes);
        let ip = IPv4Packet::new(ip_bytes.as_slice()).unwrap();
        let tcp = TCPPacket::new(ip.payload()).unwrap();

        let response = pool.accept(&ip, &tcp).expect("expected a RST");
        let L3::IPv4 { next, .. } = response else {
            panic!("expected an IPv4 response")
        };
        let L4::Tcp { rst, sequence, .. } = next else {
            panic!("expected a TCP segment")
        };

        assert!(rst);
        assert_eq!(
            sequence,
            Sequence::from(5000),
            "RFC 793: when the offending segment has ACK set, the RST's sequence \
             number must equal that ACK value, or real stacks treat the RST as \
             out-of-window and silently ignore it"
        );
    }

    #[test_case]
    fn rst_reply_to_segment_without_ack_computes_sequence_and_ack() {
        let mut pool = listening_pool();

        // A bare FIN with no ACK and no matching connection: unusual, but
        // covered by RFC 793's reset-generation rules.
        let payload: &[u8] = b"abc";
        let tcp_bytes = segment(2000, 0, false, false, true, false, payload);
        let ip_bytes = ip_frame(&tcp_bytes);
        let ip = IPv4Packet::new(ip_bytes.as_slice()).unwrap();
        let tcp = TCPPacket::new(ip.payload()).unwrap();

        let response = pool.accept(&ip, &tcp).expect("expected a RST");
        let L3::IPv4 { next, .. } = response else {
            panic!("expected an IPv4 response")
        };
        let L4::Tcp {
            rst,
            ack,
            sequence,
            acknowledgment,
            ..
        } = next
        else {
            panic!("expected a TCP segment")
        };

        assert!(rst);
        assert!(ack);
        assert_eq!(sequence, Sequence::from(0));
        assert_eq!(acknowledgment, Sequence::from(2000 + payload.len() as u32));
    }

    #[test_case]
    fn no_rst_sent_in_reply_to_an_incoming_rst() {
        let mut pool = listening_pool();

        let tcp_bytes = segment(3000, 0, false, false, false, true, &[]);
        let ip_bytes = ip_frame(&tcp_bytes);
        let ip = IPv4Packet::new(ip_bytes.as_slice()).unwrap();
        let tcp = TCPPacket::new(ip.payload()).unwrap();

        let response = pool.accept(&ip, &tcp);

        assert!(
            response.is_none(),
            "replying to an unmatched RST with another RST risks a reset storm \
             between two confused peers"
        );
    }

    #[test_case]
    fn syn_to_unlistened_port_is_met_with_rst() {
        let mut pool = ConnectionPool::default();
        // No listener registered at all.

        let tcp_bytes = segment(1000, 0, true, false, false, false, &[]);
        let ip_bytes = ip_frame(&tcp_bytes);
        let ip = IPv4Packet::new(ip_bytes.as_slice()).unwrap();
        let tcp = TCPPacket::new(ip.payload()).unwrap();

        let response = pool.accept(&ip, &tcp).expect("expected a RST");
        let L3::IPv4 { next, .. } = response else {
            panic!("expected an IPv4 response")
        };
        let L4::Tcp { rst, .. } = next else {
            panic!("expected a TCP segment")
        };

        assert!(rst);
        assert!(pool.connections.is_empty());
    }

    #[test_case]
    fn syn_with_matching_listener_creates_half_open_connection_and_replies_syn_ack() {
        let mut pool = listening_pool();

        let tcp_bytes = segment(1000, 0, true, false, false, false, &[]);
        let ip_bytes = ip_frame(&tcp_bytes);
        let ip = IPv4Packet::new(ip_bytes.as_slice()).unwrap();
        let tcp = TCPPacket::new(ip.payload()).unwrap();

        let response = pool.accept(&ip, &tcp).expect("expected a SYN-ACK");
        let L3::IPv4 { next, .. } = response else {
            panic!("expected an IPv4 response")
        };
        let L4::Tcp {
            syn,
            ack,
            acknowledgment,
            ..
        } = next
        else {
            panic!("expected a TCP segment")
        };

        assert!(syn);
        assert!(ack);
        assert_eq!(acknowledgment, Sequence::from(1001));
        assert_eq!(pool.connections.len(), 1);
    }
}
