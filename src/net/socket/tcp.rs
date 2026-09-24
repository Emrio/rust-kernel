extern crate alloc;

use core::task::Poll;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;
use alloc::vec::Vec;

use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;

use crate::net::STATE_MACHINE;
use crate::net::ipv4::IPv4Packet;
use crate::net::ipv4::address::IPv4Address;
use crate::net::socket::listen::Listen;
use crate::net::tcp::TCPPacket;
use crate::net::tcp::protocol::{Id, TransmissionControlBlock, generate_rst};
use crate::print::colors::Colorable;

struct Handle {
    queue: ArrayQueue<Connection>,
    waker: AtomicWaker,
}

pub struct Socket;

impl Socket {
    pub fn listen(listen: impl Into<Listen>) -> BoundSocket {
        let listen = listen.into();
        let handle = Arc::new(Handle {
            queue: ArrayQueue::new(32),
            waker: AtomicWaker::new(),
        });
        STATE_MACHINE.lock().tcp.listen(listen, handle.clone());
        klog!("tcp", "Listening on ", listen);
        BoundSocket { listen, handle }
    }

    pub fn connect(address: IPv4Address, port: u16) -> Connection {
        unimplemented!()
    }
}

pub struct ConnectionAccept {
    handle: Arc<Handle>,
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
    handle: Arc<Handle>,
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

pub struct Connection {}

impl Connection {
    pub fn remote_address(&self) -> IPv4Address {
        todo!()
    }

    pub fn remote_port(&self) -> u16 {
        todo!()
    }

    pub fn send(&self, buffer: &[u8]) {
        unimplemented!()
    }

    pub async fn receive(&self) -> &[u8] {
        todo!()
    }

    pub fn close(&self) {
        todo!()
    }
}

enum ConnectionStatus {
    HalfOpen(TransmissionControlBlock, Arc<Handle>),
    Established(TransmissionControlBlock),
}

impl ConnectionStatus {
    fn tcb(&self) -> &TransmissionControlBlock {
        match self {
            ConnectionStatus::HalfOpen(tcb, _) => tcb,
            ConnectionStatus::Established(tcb) => tcb,
        }
    }

    fn tcb_mut(&mut self) -> &mut TransmissionControlBlock {
        match self {
            ConnectionStatus::HalfOpen(tcb, _) => tcb,
            ConnectionStatus::Established(tcb) => tcb,
        }
    }
}

// TODO handle close, handle send
#[derive(Default)]
pub struct ConnectionPool {
    connections: BTreeMap<Id, ConnectionStatus>,
    listeners: BTreeMap<Listen, Arc<Handle>>,
}

impl ConnectionPool {
    pub const fn new() -> Self {
        Self {
            connections: BTreeMap::new(),
            listeners: BTreeMap::new(),
        }
    }

    fn listen(&mut self, listen: Listen, handle: Arc<Handle>) {
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

    fn establish(&mut self, id: Id) -> bool {
        let Some(connection) = self.connections.remove(&id) else {
            return false;
        };

        match connection {
            ConnectionStatus::Established(tcb) => {
                self.connections
                    .insert(id, ConnectionStatus::Established(tcb));

                false
            }

            ConnectionStatus::HalfOpen(tcb, handle) => {
                let connection = Connection {};

                if handle.queue.push(connection).is_err() {
                    klog!("udp", "Queue full!".red())
                }
                handle.waker.wake();

                self.connections
                    .insert(id, ConnectionStatus::Established(tcb));

                true
            }
        }
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
        if result.established && !self.establish(id) {
            // TODO destroy connection, internal assertion error
        }

        if result.destroyed {
            self.connections.remove(&id);
        }

        result.response
    }
}
