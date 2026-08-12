use core::time::Duration;

use crate::drivers::i82540em::DEVICE;
use crate::time::{Instant, sleep};
use ipv4::address::IPv4Address;

pub mod arp;
pub mod checksum;
pub mod device;
pub mod error;
pub mod ethernet;
pub mod icmp;
pub mod ipv4;
pub mod rx;
mod tx;

#[cfg(test)]
mod tests;

pub use rx::rx_loop;

pub struct StateMachine {
    ipv4: Option<IPv4Address>,
    last_arp_request: Instant,
}

static STATE_MACHINE: spin::Mutex<StateMachine> = spin::Mutex::new(StateMachine {
    ipv4: None,
    last_arp_request: Instant::zero(),
});

pub async fn net_loop() {
    loop {
        let mut state_machine = STATE_MACHINE.lock();
        if let Some(device) = DEVICE.get()
            && state_machine.ipv4.is_none()
            && Instant::now() - state_machine.last_arp_request > Duration::from_secs(5)
        {
            state_machine.last_arp_request = Instant::now();
            tx::send_arp_request(device);
        }
        drop(state_machine);

        sleep(Duration::from_secs(1)).await;
    }
}
