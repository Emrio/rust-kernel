extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::task::Poll;

use crate::drivers::i82540em::DEVICE;
use crate::net::handle::Handle;
use crate::net::ipv4::IPv4Packet;
use crate::net::ipv4::address::IPv4Address;
use crate::net::ipv4::protocol::Protocol;
use crate::net::rx::NetContext;
use crate::net::socket::listen::Listen;
use crate::net::tx::{L3, L4, L7, NetworkError};
use crate::net::udp::UDPPacket;
use crate::net::{STATE_MACHINE, tx};
use crate::print::colors::Colorable;

pub struct MessageAccept {
    handle: Arc<Handle<Message>>,
}

impl Future for MessageAccept {
    type Output = Message;

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

pub struct Socket {
    listen: Listen,
    handle: Arc<Handle<Message>>,
}

impl Socket {
    pub fn listen(listen: impl Into<Listen>) -> Self {
        let listen = listen.into();
        let handle = Arc::new(Handle::new(32));

        STATE_MACHINE.lock().udp.add(listen, handle.clone());
        klog!("udp", "Listening on ", listen);

        Self { listen, handle }
    }

    pub fn accept(&self) -> MessageAccept {
        MessageAccept {
            handle: self.handle.clone(),
        }
    }

    pub async fn send(
        &self,
        address: IPv4Address,
        port: u16,
        payload: Vec<u8>,
    ) -> Result<(), SocketError> {
        tx::send_l3(L3::IPv4 {
            source: self
                .listen
                .address()
                .or_else(get_my_ipv4_address)
                .ok_or(SocketError::DHCPNotReady)?,
            destination: address,
            protocol: Protocol::UDP,
            next: L4::Udp {
                source: self.listen.port(),
                destination: port,
                next: L7::Buffer(payload),
            },
        })
        .await
        .map_err(SocketError::NetworkError)
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        STATE_MACHINE.lock().udp.remove(&self.listen);
    }
}

pub struct Message {
    local_port: u16,
    remote_address: IPv4Address,
    remote_port: u16,
    payload: Vec<u8>,
}

#[derive(Debug)]
pub enum SocketError {
    DHCPNotReady,
    NetworkError(NetworkError),
}

impl Message {
    pub fn new(ipv4: &IPv4Packet<&[u8]>, udp: &UDPPacket<&[u8]>) -> Self {
        Self {
            remote_address: ipv4.source(),
            remote_port: udp.source(),
            local_port: udp.destination(),
            payload: udp.payload().to_vec(),
        }
    }

    pub fn remote_address(&self) -> IPv4Address {
        self.remote_address
    }

    pub fn remote_port(&self) -> u16 {
        self.remote_port
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub async fn send(&self, buffer: &[u8]) -> Result<(), SocketError> {
        tx::send_l3(L3::IPv4 {
            source: get_my_ipv4_address().ok_or(SocketError::DHCPNotReady)?,
            destination: self.remote_address,
            protocol: Protocol::UDP,
            next: L4::Udp {
                source: self.local_port,
                destination: self.remote_port,
                next: L7::Buffer(buffer.to_vec()),
            },
        })
        .await
        .map_err(SocketError::NetworkError)
    }
}

#[derive(Default)]
pub struct ListenerPool {
    listeners: BTreeMap<Listen, Arc<Handle<Message>>>,
}

impl ListenerPool {
    pub const fn new() -> Self {
        Self {
            listeners: BTreeMap::new(),
        }
    }

    fn add(&mut self, listen: Listen, handle: Arc<Handle<Message>>) {
        self.listeners.insert(listen, handle);
    }

    fn remove(&mut self, listen: &Listen) {
        self.listeners.remove(listen);
    }

    pub fn accept(&self, message: Message) {
        let local_port = message.local_port;
        let Some(handle) = self.listeners.get(&Listen::AnyAddress(local_port)) else {
            klog!("udp", "Cannot find handle for port ", local_port.yellow());
            return;
        };

        if handle.queue.push(message).is_err() {
            klog!("udp", "Queue full for port ", local_port.yellow())
        }
        handle.waker.wake();
    }
}

fn get_my_ipv4_address() -> Option<IPv4Address> {
    let state = STATE_MACHINE.lock();
    let device = DEVICE.get().expect("device to be ready");
    let context = NetContext::from_device_and_state(device, &state);
    context.ipv4_address()
}
