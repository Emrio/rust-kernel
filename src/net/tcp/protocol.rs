extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;
use alloc::vec::Vec;

use crate::net::error::BufferTooSmall;
use crate::net::ipv4::IPv4Packet;
use crate::net::ipv4::address::IPv4Address;
use crate::net::socket::listen::Listen;
use crate::net::tcp::sequence::Sequence;
use crate::net::tcp::{TCP_HEADER, TCPPacket};
use crate::print::colors::Colorable;

const MY_WINDOW: usize = 4096;

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
struct Id(IPv4Address, u16, IPv4Address, u16);

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

    fn id(&self) -> Id {
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
            .set_window(MY_WINDOW as u16)
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
            .set_ack(true)
            .set_data_offset_and_reserved()
            .set_window(MY_WINDOW as u16)
            .compute_checksum(self.local_address, self.remote_address);

        Ok(buffer)
    }

    fn generate_fin(&self) -> Result<Vec<u8>, BufferTooSmall> {
        let mut buffer = vec![0; TCP_HEADER];
        let mut packet = TCPPacket::new(&mut buffer)?;

        packet
            .set_source(self.local_port)
            .set_destination(self.remote_port)
            .set_sequence(self.snd_nxt)
            .set_acknowledgment(self.rcv_nxt)
            .set_ack(true)
            .set_fin(true)
            .set_data_offset_and_reserved()
            .set_window(MY_WINDOW as u16)
            .compute_checksum(self.local_address, self.remote_address);

        Ok(buffer)
    }

    pub fn accept(&mut self, packet: &TCPPacket<&[u8]>) -> Result<Option<Vec<u8>>, BufferTooSmall> {
        if packet.rst() {
            klog!(
                format_args!(
                    "tcp/{}/{}/{}/{}",
                    self.local_address, self.local_port, self.remote_address, self.remote_port
                ),
                "Received ",
                "RST".red()
            );
            self.state = State::Closed;
            return Ok(None);
        }

        if self.state == State::Listen && packet.syn() {
            klog!(
                format_args!(
                    "tcp/{}/{}/{}/{}",
                    self.local_address, self.local_port, self.remote_address, self.remote_port
                ),
                "Received ",
                "SYN".green()
            );
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
            klog!(
                format_args!(
                    "tcp/{}/{}/{}/{}",
                    self.local_address, self.local_port, self.remote_address, self.remote_port
                ),
                "Connection established"
            );
            self.state = State::Established;
            return Ok(None);
        }

        if self.state == State::LastAck && packet.ack() && packet.acknowledgment() == self.snd_nxt {
            klog!(
                format_args!(
                    "tcp/{}/{}/{}/{}",
                    self.local_address, self.local_port, self.remote_address, self.remote_port
                ),
                "Connection closed"
            );
            self.state = State::Closed;
            return Ok(None);
        }

        if self.state != State::Established {
            return Ok(None);
        }

        if !packet.payload().is_empty() {
            if packet.sequence() == self.rcv_nxt {
                klog!(
                    format_args!(
                        "tcp/{}/{}/{}/{}",
                        self.local_address, self.local_port, self.remote_address, self.remote_port
                    ),
                    "Received ",
                    packet.payload().len().yellow(),
                    " bytes"
                );
                self.rcv_buf.extend_from_slice(packet.payload());
                self.rcv_nxt += packet.payload().len() as u32;
            } else if packet.sequence() < self.rcv_nxt {
                // already acked
            } else {
                // too early
            }
        }

        if packet.fin() && packet.sequence() + packet.payload().len() as u32 == self.rcv_nxt {
            klog!(
                format_args!(
                    "tcp/{}/{}/{}/{}",
                    self.local_address, self.local_port, self.remote_address, self.remote_port
                ),
                "Received ",
                "FIN".bright_red()
            );
            self.state = State::CloseWait;
            self.rcv_nxt += 1;
            // TEMPORARY:
            return self.close();
        }

        self.generate_ack().map(Some)
    }

    pub fn close(&mut self) -> Result<Option<Vec<u8>>, BufferTooSmall> {
        let response = self.generate_fin()?;

        self.snd_nxt += 1;
        self.state = State::LastAck;

        Ok(Some(response))
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
    listening: BTreeSet<Listen>,
}

impl ConnectionPool {
    pub const fn new() -> Self {
        Self {
            active_connections: BTreeMap::new(),
            listening: BTreeSet::new(),
        }
    }

    pub fn listen(&mut self, listen: Listen) {
        match &listen {
            Listen::AnyAddress(port) => klog!("tcp", "Listening on 0.0.0.0:", port),
            Listen::SpecificAddress(address, port) => {
                klog!("tcp", "Listening on ", address, ":", port)
            }
        }

        self.listening.insert(listen);
    }

    fn get_connection(
        &mut self,
        ip: &IPv4Packet<&[u8]>,
        tcp: &TCPPacket<&[u8]>,
    ) -> Option<&mut TransmissionControlBlock> {
        let id = Id(
            ip.destination(),
            tcp.destination(),
            ip.source(),
            tcp.source(),
        );
        if !self.active_connections.contains_key(&id) && tcp.syn() {
            let specific_listen = Listen::SpecificAddress(ip.destination(), tcp.destination());
            let any_listen = Listen::AnyAddress(tcp.destination());
            if !self.listening.contains(&specific_listen) && !self.listening.contains(&any_listen) {
                return None;
            }
            let connection: TransmissionControlBlock = id.into();
            self.active_connections.insert(connection.id(), connection);
        }

        self.active_connections.get_mut(&id)
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
            .accept(tcp)
            .expect("buffer should not be too small");

        let id = connection.id();
        if connection.state == State::Closed {
            self.active_connections.remove(&id);
        }

        result
    }
}

fn generate_rst(ip: &IPv4Packet<&[u8]>, tcp: &TCPPacket<&[u8]>) -> Option<Vec<u8>> {
    if tcp.rst() {
        return None;
    }

    let mut buffer = vec![0; TCP_HEADER];
    let mut packet = TCPPacket::new(&mut buffer).expect("buffer is not too small");

    packet
        .set_source(tcp.destination())
        .set_destination(tcp.source())
        .set_rst(true)
        .set_data_offset_and_reserved();

    if tcp.ack() {
        packet.set_sequence(tcp.acknowledgment());
    } else {
        packet
            .set_sequence(0.into())
            .set_acknowledgment(tcp.sequence() + tcp.payload().len() as u32)
            .set_ack(true);
    }

    packet.compute_checksum(ip.destination(), ip.source());

    Some(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local() -> IPv4Address {
        IPv4Address::new(10, 0, 0, 1)
    }
    const LOCAL_PORT: u16 = 4242;
    fn remote() -> IPv4Address {
        IPv4Address::new(10, 0, 0, 2)
    }
    const REMOTE_PORT: u16 = 1234;

    /// Builds a raw incoming TCP segment (as the remote peer would send it).
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

    fn new_tcb() -> TransmissionControlBlock {
        TransmissionControlBlock::new(local(), LOCAL_PORT, remote(), REMOTE_PORT)
    }

    /// Drives a fresh TCB through the 3-way handshake (client ISS = 1000) and
    /// returns it sitting in `Established`.
    fn established_tcb() -> TransmissionControlBlock {
        let mut tcb = new_tcb();

        let syn = segment(1000, 0, true, false, false, false, &[]);
        tcb.accept(&TCPPacket::new(syn.as_slice()).unwrap())
            .unwrap();

        let ack = segment(1001, tcb.snd_nxt.into(), false, true, false, false, &[]);
        tcb.accept(&TCPPacket::new(ack.as_slice()).unwrap())
            .unwrap();

        assert_eq!(tcb.state, State::Established);
        tcb
    }

    #[test_case]
    fn listen_receiving_syn_sends_syn_ack_and_moves_to_syn_received() {
        let mut tcb = new_tcb();
        let syn = segment(1000, 0, true, false, false, false, &[]);

        let response = tcb
            .accept(&TCPPacket::new(syn.as_slice()).unwrap())
            .unwrap()
            .expect("expected a SYN-ACK");
        let response = TCPPacket::new(response.as_slice()).unwrap();

        assert_eq!(tcb.state, State::SynReceived);
        assert!(response.syn());
        assert!(response.ack());
        assert_eq!(response.acknowledgment(), Sequence::from(1001));
        assert_eq!(tcb.rcv_nxt, Sequence::from(1001));
    }

    #[test_case]
    fn syn_received_with_matching_ack_moves_to_established() {
        let mut tcb = new_tcb();
        let syn = segment(1000, 0, true, false, false, false, &[]);
        tcb.accept(&TCPPacket::new(syn.as_slice()).unwrap())
            .unwrap();

        let ack = segment(1001, tcb.snd_nxt.into(), false, true, false, false, &[]);
        let result = tcb
            .accept(&TCPPacket::new(ack.as_slice()).unwrap())
            .unwrap();

        assert_eq!(tcb.state, State::Established);
        assert!(result.is_none());
    }

    #[test_case]
    fn syn_received_ignores_ack_with_wrong_number() {
        let mut tcb = new_tcb();
        let syn = segment(1000, 0, true, false, false, false, &[]);
        tcb.accept(&TCPPacket::new(syn.as_slice()).unwrap())
            .unwrap();

        // Acknowledges something other than our SYN's sequence number.
        let bogus_ack = segment(1001, 0, false, true, false, false, &[]);
        tcb.accept(&TCPPacket::new(bogus_ack.as_slice()).unwrap())
            .unwrap();

        assert_eq!(
            tcb.state,
            State::SynReceived,
            "a mismatched ACK must not establish the connection"
        );
    }

    #[test_case]
    fn established_receives_in_order_data() {
        let mut tcb = established_tcb();
        let rcv_nxt_before: u32 = tcb.rcv_nxt.into();

        let data = segment(
            rcv_nxt_before,
            tcb.snd_nxt.into(),
            false,
            true,
            false,
            false,
            b"hello",
        );
        let response = tcb
            .accept(&TCPPacket::new(data.as_slice()).unwrap())
            .unwrap()
            .expect("expected an ACK");
        let response = TCPPacket::new(response.as_slice()).unwrap();

        assert_eq!(tcb.rcv_buf, b"hello");
        assert_eq!(tcb.rcv_nxt, Sequence::from(rcv_nxt_before + 5));
        assert_eq!(response.acknowledgment(), tcb.rcv_nxt);
    }

    #[test_case]
    fn established_ignores_duplicate_data_but_still_acks() {
        let mut tcb = established_tcb();
        let rcv_nxt_before: u32 = tcb.rcv_nxt.into();
        let data = segment(
            rcv_nxt_before,
            tcb.snd_nxt.into(),
            false,
            true,
            false,
            false,
            b"hi",
        );

        tcb.accept(&TCPPacket::new(data.as_slice()).unwrap())
            .unwrap();
        let buf_after_first = tcb.rcv_buf.clone();
        let rcv_nxt_after_first = tcb.rcv_nxt;

        // Same segment arrives again (e.g. our first ACK got lost on the wire).
        let response = tcb
            .accept(&TCPPacket::new(data.as_slice()).unwrap())
            .unwrap()
            .expect("a duplicate segment should still be met with a duplicate ACK");
        let response = TCPPacket::new(response.as_slice()).unwrap();

        assert_eq!(
            tcb.rcv_buf, buf_after_first,
            "data must not be appended twice"
        );
        assert_eq!(tcb.rcv_nxt, rcv_nxt_after_first);
        assert_eq!(response.acknowledgment(), tcb.rcv_nxt);
    }

    #[test_case]
    fn established_ignores_out_of_order_data() {
        let mut tcb = established_tcb();
        let rcv_nxt_before = tcb.rcv_nxt;

        // Arrives 100 bytes ahead of what we expect.
        let far_seq: u32 = Into::<u32>::into(rcv_nxt_before) + 100;
        let data = segment(
            far_seq,
            tcb.snd_nxt.into(),
            false,
            true,
            false,
            false,
            b"late",
        );

        tcb.accept(&TCPPacket::new(data.as_slice()).unwrap())
            .unwrap();

        assert_eq!(tcb.rcv_nxt, rcv_nxt_before);
        assert!(tcb.rcv_buf.is_empty());
    }

    #[test_case]
    fn fin_with_trailing_data_advances_past_both() {
        let mut tcb = established_tcb();
        let rcv_nxt_before: u32 = tcb.rcv_nxt.into();
        let payload: &[u8] = b"bye";

        let fin_with_data = segment(
            rcv_nxt_before,
            tcb.snd_nxt.into(),
            false,
            true,
            true,
            false,
            payload,
        );
        let response = tcb
            .accept(&TCPPacket::new(fin_with_data.as_slice()).unwrap())
            .unwrap()
            .expect("expected a response");
        let response = TCPPacket::new(response.as_slice()).unwrap();

        // 3 bytes of data + 1 for the FIN itself.
        assert_eq!(tcb.rcv_nxt, Sequence::from(rcv_nxt_before + 4));
        assert_eq!(tcb.rcv_buf, payload);
        assert_eq!(response.acknowledgment(), tcb.rcv_nxt);

        // `accept` currently auto-closes as soon as a valid FIN is processed
        // (see the `// TEMPORARY` in `accept`), so we land straight in
        // `LastAck` with our own FIN+ACK as the response.
        assert_eq!(tcb.state, State::LastAck);
        assert!(response.fin());
        assert!(response.ack());
    }

    #[test_case]
    fn fin_out_of_order_is_ignored() {
        let mut tcb = established_tcb();
        let rcv_nxt_before = tcb.rcv_nxt;

        // Claims a sequence number far beyond what's actually expected next.
        let far_seq: u32 = Into::<u32>::into(rcv_nxt_before) + 100;
        let fin = segment(far_seq, tcb.snd_nxt.into(), false, true, true, false, &[]);

        tcb.accept(&TCPPacket::new(fin.as_slice()).unwrap())
            .unwrap();

        assert_eq!(
            tcb.state,
            State::Established,
            "an out-of-order FIN must not close the connection"
        );
        assert_eq!(tcb.rcv_nxt, rcv_nxt_before);
    }

    #[test_case]
    fn close_sends_fin_with_ack_and_moves_to_last_ack() {
        let mut tcb = established_tcb();
        tcb.state = State::CloseWait;
        let snd_nxt_before: u32 = tcb.snd_nxt.into();

        let response = tcb.close().unwrap().expect("expected a FIN");
        let response = TCPPacket::new(response.as_slice()).unwrap();

        assert_eq!(tcb.state, State::LastAck);
        assert_eq!(tcb.snd_nxt, Sequence::from(snd_nxt_before + 1));
        assert!(response.fin());
        assert!(
            response.ack(),
            "a FIN without ACK gets silently dropped by real TCP stacks"
        );
        assert_eq!(response.sequence(), Sequence::from(snd_nxt_before));
    }

    #[test_case]
    fn last_ack_with_matching_ack_moves_to_closed() {
        let mut tcb = established_tcb();
        tcb.state = State::CloseWait;
        tcb.close().unwrap();
        assert_eq!(tcb.state, State::LastAck);

        let final_ack = segment(
            tcb.rcv_nxt.into(),
            tcb.snd_nxt.into(),
            false,
            true,
            false,
            false,
            &[],
        );
        let result = tcb
            .accept(&TCPPacket::new(final_ack.as_slice()).unwrap())
            .unwrap();

        assert_eq!(tcb.state, State::Closed);
        assert!(result.is_none());
    }

    #[test_case]
    fn rst_closes_connection_immediately_from_established() {
        let mut tcb = established_tcb();
        let rst = segment(tcb.rcv_nxt.into(), 0, false, false, false, true, &[]);

        let result = tcb
            .accept(&TCPPacket::new(rst.as_slice()).unwrap())
            .unwrap();

        assert_eq!(tcb.state, State::Closed);
        assert!(result.is_none());
    }

    #[test_case]
    fn rst_closes_connection_immediately_from_syn_received() {
        let mut tcb = new_tcb();
        let syn = segment(1000, 0, true, false, false, false, &[]);
        tcb.accept(&TCPPacket::new(syn.as_slice()).unwrap())
            .unwrap();
        assert_eq!(tcb.state, State::SynReceived);

        let rst = segment(1001, 0, false, false, false, true, &[]);
        tcb.accept(&TCPPacket::new(rst.as_slice()).unwrap())
            .unwrap();

        assert_eq!(tcb.state, State::Closed);
    }

    /// Wraps a raw TCP segment (as produced by `segment`) in an IPv4 header,
    /// so it can go through `ConnectionPool::accept`, which needs the IPv4
    /// addresses (not just the TCP ports).
    fn ip_frame(tcp_bytes: &[u8]) -> Vec<u8> {
        use crate::net::ipv4::IPV4_PACKET;
        use crate::net::ipv4::protocol::Protocol;
        use crate::net::ipv4::ttl::TimeToLive;

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

    #[test_case]
    fn stray_non_syn_segment_does_not_create_a_zombie_connection() {
        let mut pool = ConnectionPool::default();
        pool.listen(Listen::AnyAddress(LOCAL_PORT));

        // A stray ACK for a connection we never saw a SYN for (e.g. a late
        // retransmission from a connection that predates us listening).
        let tcp_bytes = segment(1000, 5000, false, true, false, false, &[]);
        let ip_bytes = ip_frame(&tcp_bytes);
        let ip = IPv4Packet::new(ip_bytes.as_slice()).unwrap();
        let tcp = TCPPacket::new(ip.payload()).unwrap();

        pool.accept(&ip, &tcp);

        assert!(
            pool.active_connections.is_empty(),
            "a non-SYN segment for an unknown connection must not spawn a zombie TCB"
        );
    }

    #[test_case]
    fn rst_reply_to_ack_segment_uses_incoming_ack_as_sequence() {
        let mut pool = ConnectionPool::default();
        pool.listen(Listen::AnyAddress(LOCAL_PORT));

        // Exactly like the stray retransmitted ACK+PSH+FIN observed in the wild:
        // no SYN was ever seen for this connection.
        let tcp_bytes = segment(1000, 5000, false, true, true, false, b"stray!!");
        let ip_bytes = ip_frame(&tcp_bytes);
        let ip = IPv4Packet::new(ip_bytes.as_slice()).unwrap();
        let tcp = TCPPacket::new(ip.payload()).unwrap();

        let response = pool.accept(&ip, &tcp).expect("expected a RST");
        let response = TCPPacket::new(response.as_slice()).unwrap();

        assert!(response.rst());
        assert_eq!(
            response.sequence(),
            Sequence::from(5000),
            "RFC 793: when the offending segment has ACK set, the RST's sequence \
             number must equal that ACK value, or real stacks treat the RST as \
             out-of-window and silently ignore it"
        );
    }

    #[test_case]
    fn rst_reply_to_segment_without_ack_computes_sequence_and_ack() {
        let mut pool = ConnectionPool::default();
        pool.listen(Listen::AnyAddress(LOCAL_PORT));

        // A bare FIN with no ACK and no matching connection: unusual, but
        // covered by RFC 793's reset-generation rules.
        let payload: &[u8] = b"abc";
        let tcp_bytes = segment(2000, 0, false, false, true, false, payload);
        let ip_bytes = ip_frame(&tcp_bytes);
        let ip = IPv4Packet::new(ip_bytes.as_slice()).unwrap();
        let tcp = TCPPacket::new(ip.payload()).unwrap();

        let response = pool.accept(&ip, &tcp).expect("expected a RST");
        let response = TCPPacket::new(response.as_slice()).unwrap();

        assert!(response.rst());
        assert!(response.ack());
        assert_eq!(response.sequence(), Sequence::from(0));
        assert_eq!(
            response.acknowledgment(),
            Sequence::from(2000 + payload.len() as u32)
        );
    }

    #[test_case]
    fn no_rst_sent_in_reply_to_an_incoming_rst() {
        let mut pool = ConnectionPool::default();
        pool.listen(Listen::AnyAddress(LOCAL_PORT));

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
}
