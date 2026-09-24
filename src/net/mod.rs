use core::time::Duration;

use crate::drivers::i82540em::DEVICE;
use crate::net::device::NetworkDevice;
use crate::net::ipv4::mask::IPv4Mask;
use crate::net::socket::TCPConnectionPool;
use crate::net::socket::UDPListenerPool;
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
});

async fn net_loop_logic() {
    let Some(device) = DEVICE.get() else {
        // network not ready
        return;
    };

    let mut state_machine = STATE_MACHINE.lock();

    match state_machine.dhcp {
        DHCPStateMachine::Unconfigured(last_request)
            if last_request.from_now() > Duration::from_secs(1) =>
        {
            // No DHCP configuration
            state_machine.dhcp = DHCPStateMachine::Unconfigured(Instant::now());

            let context = rx::NetContext::from_device_and_state(device, &state_machine);
            let buffer = tx::generate_dhcp_discover(&context).expect("buffer too small");
            klog!("dhcp", "Unconfigured: Sending DISCOVER");
            device.send_packet(&buffer);
        }

        DHCPStateMachine::Offered(offered_time, last_retry, xid, configuration)
            if last_retry.from_now() > Duration::from_secs(2) =>
        {
            let context = rx::NetContext::from_device_and_state(device, &state_machine);
            let buffer =
                tx::generate_dhcp_request_with_configuration(&context, xid, &configuration)
                    .expect("buffer too small");
            klog!("dhcp", "Re-requesting offer");
            device.send_packet(&buffer);
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

            let context = rx::NetContext::from_device_and_state(device, &state_machine);
            let buffer = tx::generate_dhcp_discover(&context).expect("buffer too small");
            klog!("dhcp", "Assigned: Lease expired, sending DISCOVER");
            klog!("dhcp", "Assigned -> Unconfigured");
            device.send_packet(&buffer);
        }

        _ => {}
    }
}

pub async fn net_loop() {
    loop {
        net_loop_logic().await;
        sleep(Duration::from_millis(200)).await;
    }
}
