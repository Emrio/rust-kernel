extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::task::Poll;
use futures_util::task::AtomicWaker;

use crate::net::STATE_MACHINE;
use crate::net::handle::Handle;
use crate::net::ipv4::IPv4Packet;
use crate::net::ipv4::address::IPv4Address;
use crate::net::socket::listen::Listen;
use crate::net::tcp::TCPPacket;
use crate::net::tcp::protocol::AcceptResult;
use crate::net::tcp::protocol::{Id, TransmissionControlBlock, generate_rst};
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
}

pub struct Connection {
    id: Id,
    stream: Arc<ByteStream>,
}

pub struct Receive {
    stream: Arc<ByteStream>,
}

impl Future for Receive {
    type Output = Vec<u8>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let mut buffer = self.stream.buffer.lock();
        if !buffer.is_empty() {
            return Poll::Ready(buffer.drain(..).collect());
        }
        drop(buffer);

        self.stream.waker.register(cx.waker());

        let mut buffer = self.stream.buffer.lock();
        if !buffer.is_empty() {
            return Poll::Ready(buffer.drain(..).collect());
        }
        Poll::Pending
    }
}

impl Connection {
    fn new(id: Id) -> Self {
        Self {
            id,
            stream: Arc::new(ByteStream {
                buffer: spin::Mutex::new(VecDeque::with_capacity(4096)),
                waker: AtomicWaker::new(),
            }),
        }
    }

    pub fn remote_address(&self) -> IPv4Address {
        self.id.2
    }

    pub fn remote_port(&self) -> u16 {
        self.id.3
    }

    pub fn send(&self, _buffer: &[u8]) {
        unimplemented!()
    }

    pub fn receive(&self) -> Receive {
        Receive {
            stream: self.stream.clone(),
        }
    }

    pub fn close(&self) {
        todo!()
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        todo!()
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

// TODO handle close, handle send
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

    fn process_result(&mut self, id: Id, result: AcceptResult) -> Option<Vec<u8>> {
        let Some(connection) = self.connections.remove(&id) else {
            // this is bad news
            return None;
        };

        let new_connection = match connection {
            ConnectionStatus::HalfOpen(tcb, handle) if result.established => {
                let connection = Connection::new(id);
                let stream = connection.stream.clone();

                if handle.queue.push(connection).is_err() {
                    klog!("tcp", "Queue full!".red())
                }
                handle.waker.wake();

                ConnectionStatus::Established(tcb, stream)
            }
            ConnectionStatus::Established(tcb, stream) if let Some(received) = result.received => {
                stream.buffer.lock().extend(received);
                stream.waker.wake();

                ConnectionStatus::Established(tcb, stream)
            }
            connection => connection,
        };

        self.connections.insert(id, new_connection);

        if result.destroyed {
            self.connections.remove(&id);
        }

        result.response
    }

    pub fn accept(&mut self, ip: &IPv4Packet<&[u8]>, tcp: &TCPPacket<&[u8]>) -> Option<Vec<u8>> {
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
        self.process_result(id, result)
    }
}
