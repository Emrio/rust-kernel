use core::time::Duration;

use crate::drivers::i82540em::DEVICE;
use crate::net::device::NetworkDevice;
use crate::net::ipv4::mask::IPv4Mask;
use crate::time::{Instant, sleep};
use ipv4::address::IPv4Address;

pub mod arp;
pub mod checksum;
pub mod device;
pub mod dhcp;
pub mod error;
pub mod ethernet;
pub mod icmp;
pub mod ipv4;
pub mod rx;
pub mod tcp;
mod tx;
pub mod udp;

#[cfg(test)]
mod tests;

pub use rx::rx_loop;

pub struct DHCPConfiguration {
    invalid_at: Instant,
    ipv4: IPv4Address,
    netmask: Option<IPv4Mask>,
    router: Option<IPv4Address>,
    dns: Option<IPv4Address>,
}

enum DHCPStateMachine {
    Unconfigured(Instant),
    Offered(Instant),
    Assigned(DHCPConfiguration),
}

pub struct StateMachine {
    dhcp: DHCPStateMachine,
}

impl StateMachine {
    pub fn configuration(&self) -> Option<&DHCPConfiguration> {
        match &self.dhcp {
            DHCPStateMachine::Assigned(configuration) => Some(configuration),
            _ => None,
        }
    }
}

static STATE_MACHINE: spin::Mutex<StateMachine> = spin::Mutex::new(StateMachine {
    dhcp: DHCPStateMachine::Unconfigured(Instant::zero()),
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
            device.send_packet(&buffer);
        }

        DHCPStateMachine::Offered(offered_time)
            if offered_time.from_now() > Duration::from_secs(5) =>
        {
            // DHCP offer was not met with ack, retrying...
            state_machine.dhcp = DHCPStateMachine::Unconfigured(Instant::now());
        }

        DHCPStateMachine::Assigned(DHCPConfiguration { invalid_at, .. })
            if invalid_at < Instant::now() =>
        {
            // DHCP lease expired
            state_machine.dhcp = DHCPStateMachine::Unconfigured(Instant::now());

            let context = rx::NetContext::from_device_and_state(device, &state_machine);
            let buffer = tx::generate_dhcp_discover(&context).expect("buffer too small");
            device.send_packet(&buffer);
        }

        _ => {}
    }
}

pub async fn net_loop() {
    loop {
        net_loop_logic().await;
        sleep(Duration::from_secs(1)).await;
    }
}
