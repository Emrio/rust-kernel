use core::time::Duration;

use crate::drivers::i82540em::DEVICE;
use crate::net::arp::ARPCache;
use crate::net::ipv4::mask::IPv4Mask;
use crate::net::socket::TCPConnectionPool;
use crate::net::socket::UDPListenerPool;
use crate::net::tx::NetworkError;
use crate::net::tx::send_l3;
use crate::time::{Instant, sleep};
use ipv4::address::IPv4Address;

// utils
pub mod checksum;
pub mod device;
pub mod error;
mod handle;
pub mod pong;
pub mod rx;
pub mod socket;
mod tx;

// protocols
pub mod arp;
pub mod dhcp;
pub mod ethernet;
pub mod icmp;
pub mod ipv4;
pub mod tcp;
pub mod udp;

#[cfg(test)]
mod tests;

pub use rx::rx_loop;

#[derive(Clone, Copy)]
pub struct DHCPConfiguration {
    invalid_at: Instant,
    ipv4: IPv4Address,
    netmask: Option<IPv4Mask>,
    router: Option<IPv4Address>,
    dns: Option<IPv4Address>,
}

enum DHCPStateMachine {
    Unconfigured(Instant),
    Offered(Instant, Instant, u32, DHCPConfiguration),
    Assigned(DHCPConfiguration),
}

pub struct StateMachine {
    dhcp: DHCPStateMachine,
    tcp: TCPConnectionPool,
    udp: UDPListenerPool,
    arp: ARPCache,
}

impl StateMachine {
    pub fn configuration(&self) -> Option<&DHCPConfiguration> {
        match &self.dhcp {
            DHCPStateMachine::Assigned(configuration) => Some(configuration),
            _ => None,
        }
    }

    pub fn tcp_pool(&mut self) -> &mut TCPConnectionPool {
        &mut self.tcp
    }
}

pub static STATE_MACHINE: spin::Mutex<StateMachine> = spin::Mutex::new(StateMachine {
    dhcp: DHCPStateMachine::Unconfigured(Instant::zero()),
    tcp: TCPConnectionPool::new(),
    udp: UDPListenerPool::new(),
    arp: ARPCache::new(),
});

async fn net_loop_logic() -> Result<(), NetworkError> {
    let Some(device) = DEVICE.get() else {
        // network not ready
        return Ok(());
    };

    let mut state_machine = STATE_MACHINE.lock();

    match state_machine.dhcp {
        DHCPStateMachine::Unconfigured(last_request)
            if last_request.from_now() > Duration::from_secs(1) =>
        {
            // No DHCP configuration
            state_machine.dhcp = DHCPStateMachine::Unconfigured(Instant::now());

            let context = rx::NetContext::from_device_and_state(device, &state_machine);
            let message = tx::generate_dhcp_discover(&context);
            klog!("dhcp", "Unconfigured: Sending DISCOVER");
            send_l3(message).await?;
        }

        DHCPStateMachine::Offered(offered_time, last_retry, xid, configuration)
            if last_retry.from_now() > Duration::from_secs(2) =>
        {
            let context = rx::NetContext::from_device_and_state(device, &state_machine);
            let buffer =
                tx::generate_dhcp_request_with_configuration(&context, xid, &configuration);
            klog!("dhcp", "Re-requesting offer");
            send_l3(buffer).await?;
            state_machine.dhcp =
                DHCPStateMachine::Offered(offered_time, Instant::now(), xid, configuration);
        }

        DHCPStateMachine::Offered(offered_time, _, _, _)
            if offered_time.from_now() > Duration::from_secs(9) =>
        {
            // DHCP offer was not met with ack, retrying...
            state_machine.dhcp = DHCPStateMachine::Unconfigured(Instant::now());
            klog!("dhcp", "Offered -> Unconfigured");
        }

        DHCPStateMachine::Assigned(DHCPConfiguration { invalid_at, .. })
            if invalid_at < Instant::now() =>
        {
            // DHCP lease expired
            state_machine.dhcp = DHCPStateMachine::Unconfigured(Instant::now());
            state_machine.arp.identity = None;

            let context = rx::NetContext::from_device_and_state(device, &state_machine);
            let buffer = tx::generate_dhcp_discover(&context);
            klog!("dhcp", "Assigned: Lease expired, sending DISCOVER");
            klog!("dhcp", "Assigned -> Unconfigured");
            send_l3(buffer).await?;
        }

        _ => {}
    }

    Ok(())
}

pub async fn net_loop() {
    loop {
        if let Err(err) = net_loop_logic().await {
            kprintln!("Network error: {err:?}");
        }
        sleep(Duration::from_millis(200)).await;
    }
}
