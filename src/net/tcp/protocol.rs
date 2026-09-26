extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use crate::net::error::BufferTooSmall;
use crate::net::ipv4::IPv4Packet;
use crate::net::ipv4::address::IPv4Address;
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
    // snd_wnd: u32,
    rcv_nxt: Sequence,
    // rcv_wnd: u32,
    iss: Sequence,
    irs: Sequence,
    // snd_buf: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Id(pub IPv4Address, pub u16, pub IPv4Address, pub u16);

#[derive(Debug, Default)]
pub struct AcceptResult {
    pub established: bool,
    pub destroyed: bool,
    pub peer_closed: bool,
    pub response: Option<Vec<u8>>,
    pub received: Option<Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum State {
    Listen,
    SynReceived,
    Established,
    CloseWait,
    LastAck,
    FinWait1,
    FinWait2,
    Closing,
    Closed,
}

macro_rules! serv {
    ($a:expr) => {
        format_args!(
            "tcp/{}/{}/{}/{}",
            $a.local_address, $a.local_port, $a.remote_address, $a.remote_port
        )
    };
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
            // snd_wnd: 0,
            rcv_nxt: 0.into(),
            // rcv_wnd: 0,
            iss: 0.into(),
            irs: 0.into(),
            // snd_buf: Vec::new(),
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

    pub fn accept(&mut self, packet: &TCPPacket<&[u8]>) -> Result<AcceptResult, BufferTooSmall> {
        if packet.rst() {
            klog!(serv!(self), "Received ", "RST".red());
            self.state = State::Closed;
            return Ok(AcceptResult {
                destroyed: true,
                ..Default::default()
            });
        }

        if self.state == State::Listen && packet.syn() {
            klog!(serv!(self), "Received ", "SYN".green());
            self.state = State::SynReceived;
            self.irs = packet.sequence();
            self.iss = Sequence::random();
            self.rcv_nxt = self.irs + 1;
            self.snd_una = self.iss;
            self.snd_nxt = self.iss + 1;
            return Ok(AcceptResult {
                response: Some(self.generate_syn_ack()?),
                ..Default::default()
            });
        }

        if self.state == State::SynReceived
            && packet.ack()
            && packet.acknowledgment() == self.snd_nxt
        {
            klog!(serv!(self), "Connection established");
            self.state = State::Established;
            return Ok(AcceptResult {
                established: true,
                ..Default::default()
            });
        }

        if self.state == State::LastAck && packet.ack() && packet.acknowledgment() == self.snd_nxt {
            klog!(serv!(self), "Connection closed (LastAck)");
            self.state = State::Closed;
            return Ok(AcceptResult {
                destroyed: true,
                ..Default::default()
            });
        }

        if self.state == State::FinWait1
            && packet.fin()
            && packet.ack()
            && packet.acknowledgment() == self.snd_nxt
        {
            klog!(serv!(self), "Connection closed (FinWait1)");
            self.state = State::Closed;
            self.rcv_nxt += 1;
            return Ok(AcceptResult {
                response: Some(self.generate_ack()?),
                peer_closed: true,
                destroyed: true,
                ..Default::default()
            });
        }

        if self.state == State::FinWait1 && packet.fin() {
            klog!(serv!(self), "FinWait1 -> Closing");
            self.state = State::Closing;
            self.rcv_nxt += 1;
            return Ok(AcceptResult {
                response: Some(self.generate_ack()?),
                peer_closed: true,
                ..Default::default()
            });
        }

        if self.state == State::FinWait1 && packet.ack() && packet.acknowledgment() == self.snd_nxt
        {
            klog!(serv!(self), "FinWait1 -> FinWait2");
            self.state = State::FinWait2;
            return Ok(AcceptResult::default());
        }

        if self.state == State::FinWait2 && packet.fin() {
            klog!(serv!(self), "Connection closed (FinWait2)");
            self.state = State::Closed;
            self.rcv_nxt += 1;
            return Ok(AcceptResult {
                response: Some(self.generate_ack()?),
                peer_closed: true,
                destroyed: true,
                ..Default::default()
            });
        }

        if self.state == State::Closing && packet.ack() && packet.acknowledgment() == self.snd_nxt {
            klog!(serv!(self), "Closing -> Closed");
            self.state = State::Closed;
            return Ok(AcceptResult {
                destroyed: true,
                ..Default::default()
            });
        }

        if self.state != State::Established {
            return Ok(AcceptResult::default());
        }

        let mut received = None;
        let mut generated_ack = false;

        if !packet.payload().is_empty() {
            generated_ack = true;
            if packet.sequence() == self.rcv_nxt {
                klog!(
                    serv!(self),
                    "Received ",
                    packet.payload().len().yellow(),
                    " bytes"
                );
                received = Some(packet.payload().to_vec());
                self.rcv_nxt += packet.payload().len() as u32;
            } else if packet.sequence() < self.rcv_nxt {
                // already acked
            } else {
                // too early
            }
        }

        let peer_closed =
            if packet.fin() && packet.sequence() + packet.payload().len() as u32 == self.rcv_nxt {
                klog!(serv!(self), "Received ", "FIN".bright_red());
                self.state = State::CloseWait;
                self.rcv_nxt += 1;
                true
            } else {
                false
            };

        Ok(AcceptResult {
            response: if generated_ack {
                Some(self.generate_ack()?)
            } else {
                None
            },
            peer_closed,
            received,
            ..Default::default()
        })
    }

    pub fn close(&mut self) -> Result<Option<Vec<u8>>, BufferTooSmall> {
        let response = self.generate_fin()?;
        self.snd_nxt += 1;

        self.state = match self.state {
            State::SynReceived | State::Established => State::FinWait1,
            State::CloseWait => State::LastAck,
            State::Listen | State::Closed => State::Closed,
            state => state,
        };

        Ok(Some(response))
    }

    pub fn send_data(&mut self, payload: &[u8]) -> Result<Vec<u8>, BufferTooSmall> {
        let mut buffer = vec![0; TCP_HEADER + payload.len()];
        let mut packet = TCPPacket::new(&mut buffer)?;

        packet
            .set_source(self.local_port)
            .set_destination(self.remote_port)
            .set_sequence(self.snd_nxt)
            .set_acknowledgment(self.rcv_nxt)
            .set_ack(true)
            .set_psh(true)
            .set_data_offset_and_reserved()
            .set_window(MY_WINDOW as u16);
        packet.payload_mut().copy_from_slice(payload);
        packet.compute_checksum(self.local_address, self.remote_address);

        self.snd_nxt += payload.len() as u32;
        Ok(buffer)
    }
}

impl From<Id> for TransmissionControlBlock {
    fn from(value: Id) -> Self {
        Self::new(value.0, value.1, value.2, value.3)
    }
}

pub fn generate_rst(ip: &IPv4Packet<&[u8]>, tcp: &TCPPacket<&[u8]>) -> Option<Vec<u8>> {
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
            .response
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
        assert!(result.response.is_none());
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
        let result = tcb
            .accept(&TCPPacket::new(data.as_slice()).unwrap())
            .unwrap();
        let response = result.response.clone().expect("expected an ACK");
        let response = TCPPacket::new(response.as_slice()).unwrap();

        assert_eq!(result.received.as_deref(), Some(b"hello".as_slice()));
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

        let first = tcb
            .accept(&TCPPacket::new(data.as_slice()).unwrap())
            .unwrap();
        assert_eq!(first.received.as_deref(), Some(b"hi".as_slice()));
        let rcv_nxt_after_first = tcb.rcv_nxt;

        // Same segment arrives again (e.g. our first ACK got lost on the wire).
        let second = tcb
            .accept(&TCPPacket::new(data.as_slice()).unwrap())
            .unwrap();
        let response = second
            .response
            .clone()
            .expect("a duplicate segment should still be met with a duplicate ACK");
        let response = TCPPacket::new(response.as_slice()).unwrap();

        assert!(
            second.received.is_none(),
            "duplicate data must not be delivered twice to the application"
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

        let result = tcb
            .accept(&TCPPacket::new(data.as_slice()).unwrap())
            .unwrap();

        assert_eq!(tcb.rcv_nxt, rcv_nxt_before);
        assert!(result.received.is_none());
    }

    #[test_case]
    fn fin_with_trailing_data_moves_to_close_wait_and_acks_both() {
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
        let result = tcb
            .accept(&TCPPacket::new(fin_with_data.as_slice()).unwrap())
            .unwrap();
        let response = result.response.clone().expect("expected an ACK");
        let response = TCPPacket::new(response.as_slice()).unwrap();

        // 3 bytes of data + 1 for the FIN itself.
        assert_eq!(tcb.rcv_nxt, Sequence::from(rcv_nxt_before + 4));
        assert_eq!(result.received.as_deref(), Some(payload));
        assert!(result.peer_closed);
        assert_eq!(response.acknowledgment(), tcb.rcv_nxt);

        // Receiving a FIN only closes the read side (half-close): we must
        // NOT send our own FIN yet, only once the application calls
        // `close()` (see `passive_close_full_lifecycle`).
        assert_eq!(tcb.state, State::CloseWait);
        assert!(!response.fin());
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
        assert!(result.response.is_none());
    }

    /// Regression test for the real `nc` session traced by hand: the peer
    /// closes first, we only ACK it (no FIN of our own -- `CloseWait` must
    /// not auto-close, see `fin_with_trailing_data_moves_to_close_wait_and_acks_both`),
    /// the application then calls `close()`, and only then do we land in
    /// `Closed` once the peer ACKs our FIN. This used to get stuck forever
    /// in `CloseWait` because `close()`'s state transition forgot that case.
    #[test_case]
    fn passive_close_full_lifecycle() {
        let mut tcb = established_tcb();

        // Peer closes first.
        let fin = segment(
            tcb.rcv_nxt.into(),
            tcb.snd_nxt.into(),
            false,
            true,
            true,
            false,
            &[],
        );
        let result = tcb
            .accept(&TCPPacket::new(fin.as_slice()).unwrap())
            .unwrap();
        assert_eq!(tcb.state, State::CloseWait);
        assert!(result.peer_closed);
        assert!(
            !result.destroyed,
            "CloseWait must not auto-close the connection"
        );

        // The application only decides to close afterwards.
        let our_fin = tcb.close().unwrap().expect("expected our own FIN");
        let our_fin = TCPPacket::new(our_fin.as_slice()).unwrap();
        assert_eq!(
            tcb.state,
            State::LastAck,
            "closing from CloseWait must move to LastAck, not stay stuck in CloseWait"
        );
        assert!(our_fin.fin());
        assert!(our_fin.ack());

        // Peer ACKs our FIN.
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
        assert!(result.destroyed);
    }

    /// Regression test for two bugs found while tracing a real active-close
    /// sequence: (1) the ACK generated when leaving `FinWait1`/`FinWait2` was
    /// placed in `AcceptResult.received` instead of `.response` (so it was
    /// silently never sent), and (2) `rcv_nxt` was not advanced before
    /// acknowledging the peer's FIN, so the ACK we sent didn't cover the
    /// FIN's own sequence number and the peer kept retransmitting it.
    #[test_case]
    fn active_close_then_peer_fin_full_lifecycle() {
        let mut tcb = established_tcb();

        // We close first.
        let our_fin = tcb.close().unwrap().expect("expected our own FIN");
        let our_fin_seq: u32 = TCPPacket::new(our_fin.as_slice())
            .unwrap()
            .sequence()
            .into();
        assert_eq!(tcb.state, State::FinWait1);

        // Peer ACKs our FIN (no FIN of their own yet).
        let ack = segment(
            tcb.rcv_nxt.into(),
            our_fin_seq + 1,
            false,
            true,
            false,
            false,
            &[],
        );
        let result = tcb
            .accept(&TCPPacket::new(ack.as_slice()).unwrap())
            .unwrap();
        assert_eq!(tcb.state, State::FinWait2);
        assert!(result.response.is_none());

        // Peer now sends its own FIN.
        let rcv_nxt_before: u32 = tcb.rcv_nxt.into();
        let peer_fin = segment(rcv_nxt_before, our_fin_seq + 1, false, true, true, false, &[]);
        let result = tcb
            .accept(&TCPPacket::new(peer_fin.as_slice()).unwrap())
            .unwrap();

        assert_eq!(tcb.state, State::Closed);
        assert!(result.destroyed);
        let response = result
            .response
            .expect("the ACK for the peer's FIN must be sent, not stashed in `received`");
        let response = TCPPacket::new(response.as_slice()).unwrap();
        assert_eq!(
            response.acknowledgment(),
            Sequence::from(rcv_nxt_before + 1),
            "a FIN consumes one sequence number: the ACK must cover seq+1, not seq, \
             or the peer will consider its FIN unacknowledged and retransmit it"
        );
    }

    /// Regression test for the simultaneous-close path (both sides send a
    /// FIN before seeing the other's). As of writing this still fails:
    /// `Closing -> Closed` (see `accept`) doesn't set `destroyed`, unlike
    /// every other terminal transition, so the TCB never actually leaves the
    /// pool once closed this way.
    #[test_case]
    fn simultaneous_close_full_lifecycle() {
        let mut tcb = established_tcb();

        let our_fin = tcb.close().unwrap().expect("expected our own FIN");
        let our_fin_seq: u32 = TCPPacket::new(our_fin.as_slice())
            .unwrap()
            .sequence()
            .into();
        assert_eq!(tcb.state, State::FinWait1);

        // Peer's FIN crosses ours on the wire, before ACKing ours.
        let rcv_nxt_before: u32 = tcb.rcv_nxt.into();
        let peer_fin = segment(rcv_nxt_before, 0, false, false, true, false, &[]);
        let result = tcb
            .accept(&TCPPacket::new(peer_fin.as_slice()).unwrap())
            .unwrap();
        assert_eq!(tcb.state, State::Closing);
        assert!(result.peer_closed);

        // Peer now ACKs our FIN.
        let final_ack = segment(
            tcb.rcv_nxt.into(),
            our_fin_seq + 1,
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
        assert!(
            result.destroyed,
            "Closing -> Closed must mark the connection as destroyed, like every \
             other terminal transition, or the TCB leaks in the pool forever"
        );
    }

    #[test_case]
    fn rst_closes_connection_immediately_from_established() {
        let mut tcb = established_tcb();
        let rst = segment(tcb.rcv_nxt.into(), 0, false, false, false, true, &[]);

        let result = tcb
            .accept(&TCPPacket::new(rst.as_slice()).unwrap())
            .unwrap();

        assert_eq!(tcb.state, State::Closed);
        assert!(result.response.is_none());
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
}
