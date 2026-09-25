extern crate alloc;

use core::ops::Deref;
use core::task::Poll;
use core::time::Duration;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use futures_util::task::AtomicWaker;

use crate::net::arp::cache::ResolveStart::{Known, Pending};
use crate::net::arp::{ARPOperation, ARPPacket, HardwareType, ProtocolType};
use crate::net::ethernet::address::EthernetAddress;
use crate::net::ethernet::ethertype::EtherType;
use crate::net::ipv4::address::IPv4Address;
use crate::net::tx::{self, L2, L3};
use crate::print::colors::Colorable;
use crate::time::Instant;

pub(crate) struct ARPMessage {
    hardware_type: HardwareType,
    hardware_length: usize,
    protocol_type: ProtocolType,
    protocol_length: usize,
    target_protocol_address: IPv4Address,
    sender_hardware_address: EthernetAddress,
    sender_protocol_address: IPv4Address,
    operation: ARPOperation,
}

impl From<ARPPacket<&[u8]>> for ARPMessage {
    fn from(value: ARPPacket<&[u8]>) -> Self {
        Self {
            hardware_type: value.hardware_type(),
            hardware_length: value.hardware_length() as usize,
            protocol_type: value.protocol_type(),
            protocol_length: value.protocol_length() as usize,
            target_protocol_address: value.target_protocol_address(),
            sender_hardware_address: value.sender_hardware_address(),
            sender_protocol_address: value.sender_protocol_address(),
            operation: value.operation(),
        }
    }
}

struct Entry {
    expiration: Option<Instant>,
    hardware_address: EthernetAddress,
}

struct Request {
    waker: AtomicWaker,
    value: spin::Mutex<Option<EthernetAddress>>,
}

pub(crate) struct Resolution {
    identity: (EthernetAddress, IPv4Address),
    address: IPv4Address,
    request: Arc<Request>,
    initiated_at: Instant,
    last_request: Instant,
}

#[derive(Debug)]
pub enum ResolutionError {
    TimedOut,
    UnconfiguredIdentity,
}

impl Future for Resolution {
    type Output = Result<EthernetAddress, ResolutionError>;

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        if let Some(address) = self.request.value.lock().deref() {
            klog!(
                "arp",
                "Resolved: ",
                self.address.green(),
                " is ",
                address.yellow()
            );
            return Poll::Ready(Ok(*address));
        }

        if Instant::now() - self.initiated_at > Duration::from_secs(5) {
            return Poll::Ready(Err(ResolutionError::TimedOut));
        }

        if Instant::now() - self.last_request > Duration::from_secs(1) {
            self.last_request = Instant::now();

            let _ = tx::send_l2(L2::Ethernet {
                source: self.identity.0,
                destination: EthernetAddress::BROADCAST,
                ethertype: EtherType::ARP,
                next: L3::Arp {
                    operation: ARPOperation::Request,
                    sender_hardware_address: self.identity.0,
                    sender_protocol_address: self.identity.1,
                    target_hardware_address: EthernetAddress::BROADCAST,
                    target_protocol_address: self.address,
                },
            });
        }

        self.request.waker.register(cx.waker());

        match self.request.value.lock().deref() {
            Some(address) => {
                klog!(
                    "arp",
                    "Resolved: ",
                    self.address.green(),
                    " is ",
                    address.yellow()
                );
                Poll::Ready(Ok(*address))
            }
            None => Poll::Pending,
        }
    }
}

pub(crate) enum ResolveStart {
    Known(EthernetAddress),
    Pending(Resolution),
}

#[derive(Default)]
pub struct Cache {
    pub identity: Option<(EthernetAddress, IPv4Address)>,
    entries: BTreeMap<IPv4Address, Entry>,
    requests: BTreeMap<IPv4Address, Arc<Request>>,
}

impl Cache {
    pub const fn new() -> Self {
        Self {
            identity: None,
            entries: BTreeMap::new(),
            requests: BTreeMap::new(),
        }
    }

    fn get_cache(&self, address: IPv4Address) -> Option<EthernetAddress> {
        if let Some((my_ethernet_address, my_ipv4_address)) = self.identity
            && my_ipv4_address == address
        {
            klog!("arp", "Hit on my identity");
            return Some(my_ethernet_address);
        }

        if let Some(entry) = self.entries.get(&address) {
            klog!(
                "arp",
                "Cache hit: ",
                address.green(),
                " is ",
                entry.hardware_address.yellow()
            );
            return Some(entry.hardware_address);
        }

        None
    }

    pub(crate) fn resolve_start(
        &mut self,
        address: IPv4Address,
    ) -> Result<ResolveStart, ResolutionError> {
        if let Some(ethernet_address) = self.get_cache(address) {
            return Ok(Known(ethernet_address));
        }

        let request = Arc::new(Request {
            waker: AtomicWaker::new(),
            value: spin::Mutex::new(None),
        });
        self.requests.insert(address, request.clone());
        let resolution = Resolution {
            request,
            initiated_at: Instant::now(),
            last_request: Instant::zero(),
            address,
            identity: self.identity.ok_or(ResolutionError::UnconfiguredIdentity)?,
        };

        Ok(Pending(resolution))
    }

    pub(crate) fn resolve_finish(&mut self, address: IPv4Address, mac: EthernetAddress) {
        self.requests.remove(&address);
        self.add_entry(address, mac);
    }

    fn add_entry(&mut self, ipv4: IPv4Address, ethernet: EthernetAddress) {
        self.entries.insert(
            ipv4,
            Entry {
                expiration: Some(Instant::now() + Duration::from_secs(30)),
                hardware_address: ethernet,
            },
        );
    }

    pub fn remove_expired(&mut self) {
        let t = Instant::now();
        self.entries
            .retain(|_, entry| entry.expiration.is_none_or(|t_exp| t_exp > t))
    }

    pub(crate) async fn accept(&mut self, message: impl Into<ARPMessage>) {
        let message = message.into();
        if message.hardware_type != HardwareType::Ethernet
            || message.hardware_length != EthernetAddress::SIZE
            || message.protocol_type != ProtocolType::IPv4
            || message.protocol_length != IPv4Address::SIZE
        {
            klog!("arp", "early return");
            return;
        }

        if message.operation == ARPOperation::Request
            && let Some(ethernet_address) = self.get_cache(message.target_protocol_address)
        {
            klog!(
                "arp",
                "Answering to request from ",
                message.sender_hardware_address.yellow(),
                "/",
                message.sender_protocol_address.green(),
                ": ",
                message.target_protocol_address.bright_green(),
                " is ",
                ethernet_address.bright_yellow()
            );
            let _ = tx::send_l3(L3::Arp {
                operation: ARPOperation::Reply,
                sender_hardware_address: ethernet_address,
                sender_protocol_address: message.target_protocol_address,
                target_hardware_address: message.sender_hardware_address,
                target_protocol_address: message.sender_protocol_address,
            })
            .await;
            if message.sender_protocol_address != IPv4Address::BROADCAST {
                self.add_entry(
                    message.sender_protocol_address,
                    message.sender_hardware_address,
                );
            }
            return;
        }

        if message.operation == ARPOperation::Reply
            && let Some(request) = self.requests.get_mut(&message.sender_protocol_address)
        {
            let request = request.clone();
            let mut value = request.value.lock();
            *value = Some(message.sender_hardware_address);
            request.waker.wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::block_on;

    fn me() -> (EthernetAddress, IPv4Address) {
        (
            EthernetAddress::from_bytes(&[1, 2, 3, 4, 5, 6]),
            IPv4Address::new(10, 0, 0, 1),
        )
    }

    fn peer() -> (EthernetAddress, IPv4Address) {
        (
            EthernetAddress::from_bytes(&[7, 8, 9, 10, 11, 12]),
            IPv4Address::new(10, 0, 0, 2),
        )
    }

    /// Builds a synthetic ARP reply, as if `peer` had answered a request for its address.
    fn reply_from(peer: (EthernetAddress, IPv4Address), requester: (EthernetAddress, IPv4Address)) -> ARPMessage {
        ARPMessage {
            hardware_type: HardwareType::Ethernet,
            hardware_length: EthernetAddress::SIZE,
            protocol_type: ProtocolType::IPv4,
            protocol_length: IPv4Address::SIZE,
            // A reply always echoes the original requester back as its target.
            target_protocol_address: requester.1,
            sender_hardware_address: peer.0,
            sender_protocol_address: peer.1,
            operation: ARPOperation::Reply,
        }
    }

    #[test_case]
    fn resolve_start_is_known_for_own_identity() {
        let mut cache = Cache::new();
        cache.identity = Some(me());

        let start = cache.resolve_start(me().1).unwrap();
        assert!(matches!(start, ResolveStart::Known(mac) if mac == me().0));
    }

    #[test_case]
    fn resolve_start_is_known_for_a_cached_entry() {
        let mut cache = Cache::new();
        cache.identity = Some(me());
        cache.add_entry(peer().1, peer().0);

        let start = cache.resolve_start(peer().1).unwrap();
        assert!(matches!(start, ResolveStart::Known(mac) if mac == peer().0));
    }

    #[test_case]
    fn resolve_start_errors_without_identity_configured() {
        let mut cache = Cache::new();

        let result = cache.resolve_start(peer().1);
        assert!(matches!(result, Err(ResolutionError::UnconfiguredIdentity)));
    }

    #[test_case]
    fn resolve_start_registers_a_pending_request() {
        let mut cache = Cache::new();
        cache.identity = Some(me());

        let start = cache.resolve_start(peer().1).unwrap();
        assert!(matches!(start, ResolveStart::Pending(_)));
        assert!(cache.requests.contains_key(&peer().1));
    }

    #[test_case]
    fn accept_reply_resolves_the_matching_pending_request() {
        let mut cache = Cache::new();
        cache.identity = Some(me());

        let start = cache.resolve_start(peer().1).unwrap();
        let ResolveStart::Pending(resolution) = start else {
            panic!("expected a pending resolution")
        };

        block_on(cache.accept(reply_from(peer(), me())));

        let resolved = block_on(resolution).expect("expected the resolution to succeed");
        assert_eq!(
            resolved,
            peer().0,
            "must resolve to the *peer's* MAC, not our own echoed back"
        );
    }

    #[test_case]
    fn accept_reply_for_an_unrequested_address_is_ignored() {
        let mut cache = Cache::new();
        cache.identity = Some(me());

        // No resolve_start() was ever called for `peer` -- nothing pending for it.
        block_on(cache.accept(reply_from(peer(), me())));

        assert!(cache.get_cache(peer().1).is_none());
    }

    #[test_case]
    fn resolve_finish_caches_the_entry_and_clears_the_pending_request() {
        let mut cache = Cache::new();
        cache.identity = Some(me());
        cache.resolve_start(peer().1).unwrap();

        cache.resolve_finish(peer().1, peer().0);

        assert!(!cache.requests.contains_key(&peer().1));
        assert_eq!(cache.get_cache(peer().1), Some(peer().0));
    }
}
