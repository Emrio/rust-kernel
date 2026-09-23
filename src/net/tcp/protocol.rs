extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;
use alloc::vec::Vec;

use crate::net::error::BufferTooSmall;
use crate::net::ipv4::IPv4Packet;
use crate::net::ipv4::address::IPv4Address;
use crate::net::tcp::sequence::Sequence;
use crate::net::tcp::{TCP_HEADER, TCPPacket};

pub struct TransmissionControlBlock {
    local_address: IPv4Address,
    local_port: u16,
    remote_address: IPv4Address,
    remote_port: u16,

    state: State,

    snd_una: Sequence,
    snd_nxt: Sequence,
    snd_wnd: u32,

    rcv_nxt: Sequence,
    rcv_wnd: u32,

    iss: Sequence,
    irs: Sequence,

    // TODO: queue + waker, maybe impl Stream?
    snd_buf: Vec<u8>,
    rcv_buf: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Id(IPv4Address, u16, IPv4Address, u16);

impl Id {
    pub fn from(ip: &IPv4Packet<&[u8]>, tcp: &TCPPacket<&[u8]>) -> Self {
        Self(
            ip.destination(),
            tcp.destination(),
            ip.source(),
            tcp.source(),
        )
    }

    pub fn without_remote(&self) -> Self {
        Self(self.0, self.1, IPv4Address::default(), 0)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    Listen,
    SynReceived,
    Established,
    CloseWait,
    LastAck,
    Closed,
}

impl TransmissionControlBlock {
    pub fn new(
        local_address: IPv4Address,
        local_port: u16,
        remote_address: IPv4Address,
        remote_port: u16,
    ) -> Self {
        Self {
            local_address,
            local_port,
            remote_address,
            remote_port,
            state: State::Listen,
            snd_una: 0.into(),
            snd_nxt: 0.into(),
            snd_wnd: 0,
            rcv_nxt: 0.into(),
            rcv_wnd: 0,
            iss: 0.into(),
            irs: 0.into(),
            snd_buf: Vec::new(),
            rcv_buf: Vec::new(),
        }
    }

    pub fn id(&self) -> Id {
        Id(
            self.local_address,
            self.local_port,
            self.remote_address,
            self.remote_port,
        )
    }

    fn generate_syn_ack(&self) -> Result<Vec<u8>, BufferTooSmall> {
        let mut buffer = vec![0; TCP_HEADER];
        let mut packet = TCPPacket::new(&mut buffer)?;

        packet
            .set_source(self.local_port)
            .set_destination(self.remote_port)
            .set_sequence(self.iss)
            .set_acknowledgment(self.rcv_nxt)
            .set_syn(true)
            .set_ack(true)
            .set_data_offset_and_reserved()
            .compute_checksum(self.local_address, self.remote_address);

        Ok(buffer)
    }

    fn generate_ack(&self) -> Result<Vec<u8>, BufferTooSmall> {
        let mut buffer = vec![0; TCP_HEADER];
        let mut packet = TCPPacket::new(&mut buffer)?;

        packet
            .set_source(self.local_port)
            .set_destination(self.remote_port)
            .set_sequence(self.snd_nxt)
            .set_acknowledgment(self.rcv_nxt)
            .set_syn(false)
            .set_ack(true)
            .set_data_offset_and_reserved()
            .compute_checksum(self.local_address, self.remote_address);

        Ok(buffer)
    }

    pub fn accept(&mut self, packet: TCPPacket<&[u8]>) -> Result<Option<Vec<u8>>, BufferTooSmall> {
        if packet.rst() {
            self.state = State::Closed;
            return Ok(None);
        }

        if self.state == State::Listen && packet.syn() {
            self.state = State::SynReceived;
            self.irs = packet.sequence();
            self.iss = Sequence::random();
            self.rcv_nxt = self.irs + 1;
            self.snd_una = self.iss;
            self.snd_nxt = self.iss + 1;
            return self.generate_syn_ack().map(Some);
        }

        if self.state == State::SynReceived
            && packet.ack()
            && packet.acknowledgment() == self.snd_nxt
        {
            self.state = State::Established;
            return Ok(None);
        }

        if self.state == State::LastAck && packet.ack() && packet.acknowledgment() == self.snd_nxt {
            self.state = State::Closed;
            return Ok(None);
        }

        if self.state != State::Established {
            return Ok(None);
        }

        if packet.fin() {
            self.state = State::CloseWait;

            if packet.payload().is_empty() {
                self.rcv_nxt += 1;
                return self.generate_ack().map(Some);
            }
        }

        if !packet.payload().is_empty() {
            if packet.sequence() == self.rcv_nxt {
                self.rcv_buf.extend_from_slice(packet.payload());
                self.rcv_nxt += packet.payload().len() as u32;
                return self.generate_ack().map(Some);
            } else if packet.sequence() < self.rcv_nxt {
                // already acked
                return self.generate_ack().map(Some);
            } else {
                // too early
                return Ok(None);
            }
        }

        Ok(None)
    }
}

impl From<Id> for TransmissionControlBlock {
    fn from(value: Id) -> Self {
        Self::new(value.0, value.1, value.2, value.3)
    }
}

// TODO: delegate to task that awaits for new data?
#[derive(Default)]
pub struct ConnectionPool {
    active_connections: BTreeMap<Id, TransmissionControlBlock>,
    listening: BTreeSet<Id>,
}

impl ConnectionPool {
    fn get_connection(
        &mut self,
        ip: &IPv4Packet<&[u8]>,
        tcp: &TCPPacket<&[u8]>,
    ) -> Option<&mut TransmissionControlBlock> {
        let id = Id::from(ip, tcp);

        if !self.active_connections.contains_key(&id) {
            if !self.listening.contains(&id.without_remote()) {
                return None;
            }
            let connection: TransmissionControlBlock = id.into();
            self.active_connections.insert(connection.id(), connection);
        }

        self.active_connections.get_mut(&id)
    }

    pub fn accept(&mut self, ip: IPv4Packet<&[u8]>, tcp: TCPPacket<&[u8]>) -> Option<Vec<u8>> {
        let connection = self.get_connection(&ip, &tcp)?;

        let result = connection
            .accept(tcp)
            .expect("buffer should not be too small");

        let id = connection.id();
        if connection.state == State::Closed {
            self.active_connections.remove(&id);
        }

        result
    }
}
