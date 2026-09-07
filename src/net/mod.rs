use core::time::Duration;

use crate::drivers::i82540em::DEVICE;
use crate::net::device::NetworkDevice;
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
mod tx;
pub mod udp;

#[cfg(test)]
mod tests;

pub use rx::rx_loop;

pub struct Configuration {
    invalid_at: Instant,
    ipv4: IPv4Address,
    router: Option<IPv4Address>,
    dns: Option<IPv4Address>,
}

enum DHCPStateMachine {
    Unconfigured(Instant),
    Offered(Instant),
    Assigned(Configuration),
}

pub struct StateMachine {
    dhcp: DHCPStateMachine,
}

impl StateMachine {
    pub fn configuration(&self) -> Option<&Configuration> {
        match &self.dhcp {
            DHCPStateMachine::Assigned(configuration) => Some(configuration),
            _ => None,
        }
    }
}

static STATE_MACHINE: spin::Mutex<StateMachine> = spin::Mutex::new(StateMachine {
    dhcp: DHCPStateMachine::Unconfigured(Instant::zero()),
});

pub async fn net_loop() {
    loop {
        let mut state_machine = STATE_MACHINE.lock();

        if let Some(device) = DEVICE.get()
            && let DHCPStateMachine::Unconfigured(last_request) = state_machine.dhcp
            && Instant::now() - last_request > Duration::from_secs(1)
        {
            state_machine.dhcp = DHCPStateMachine::Unconfigured(Instant::now());
            let context = rx::NetContext::from_device_and_state(device, &state_machine);
            match tx::generate_dhcp_discover(&context) {
                Ok(frame) => device.send_packet(&frame),
                Err(error::BufferTooSmall) => unreachable!("Buffer too small"),
            }
        }
        drop(state_machine);

        sleep(Duration::from_secs(1)).await;
    }
}
