use core::arch::x86_64::_rdtsc;
use core::pin::Pin;
use core::sync::atomic::{AtomicU64, Ordering};
use core::task::{Context, Poll};
use core::time::Duration;

use conquer_once::spin::OnceCell;
use x86_64::instructions::port::Port;

fn tsc_now() -> u64 {
    unsafe { _rdtsc() }
}

static TSC_FREQUENCY: OnceCell<u64> = OnceCell::uninit();

/**
 * Wait CALIBRATION_TICK_COUNT ticks from the programmable interval timer
 */
#[must_use = "TSC clock calibrator must be awaited"]
pub struct ClockCalibration {
    tsc_begin: u64,
    pit_begin: u64,
}

const CALIBRATION_TICK_COUNT: u64 = 300;

/**
 * I would have preferred something like 1 kHz but the VM interrupt delivery starts to be unreliable at this frequency.
 */
const TARGET_PIT_FREQUENCY: u32 = 600; // Hz
const PIT_DIVISOR: u16 = (1_193_182 / TARGET_PIT_FREQUENCY) as u16;
const PIT_FREQUENCY: u64 = 1_193_182_000_000 / (PIT_DIVISOR as u64); // µHz

static CALIBRATION_TICKER: AtomicU64 = AtomicU64::new(0);
pub(crate) fn tick_calibrator() {
    CALIBRATION_TICKER.fetch_add(1, Ordering::Release);
}

impl Future for ClockCalibration {
    type Output = u64; // TSC's frequency

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pit_now = CALIBRATION_TICKER.load(Ordering::Acquire);

        if pit_now - self.pit_begin < CALIBRATION_TICK_COUNT {
            return Poll::Pending;
        }

        let tsc_delta = tsc_now() - self.tsc_begin;
        let tsc_frequency = (tsc_delta * PIT_FREQUENCY) / CALIBRATION_TICK_COUNT / 1_000_000;

        Poll::Ready(tsc_frequency)
    }
}

pub fn calibrate() -> ClockCalibration {
    let tsc_begin = tsc_now();
    let pit_begin = CALIBRATION_TICKER.load(Ordering::Acquire);

    ClockCalibration {
        tsc_begin,
        pit_begin,
    }
}

fn set_pit_frequency_to_target() {
    let mut cmd: Port<u8> = Port::new(0x43);
    let mut data: Port<u8> = Port::new(0x40);

    unsafe {
        cmd.write(0x36);
        data.write((PIT_DIVISOR & 0xFF) as u8);
        data.write((PIT_DIVISOR >> 8) as u8);
    }
}

/// This function enables the use of the `sleep()` function and other time utilities.
/// WARNING: It also increases the PIT interrupt frequency from 18.2 Hz to 600 Hz!
pub async fn init_time() {
    set_pit_frequency_to_target();
    let tsc_frequency = calibrate().await;
    TSC_FREQUENCY.init_once(|| tsc_frequency);
}

// Everything beyond this point assumes TSC_FREQUENCY is set

trait TickSupport {
    fn from_ticks(ticks: u64) -> Self;
    fn to_ticks(self) -> u64;
}

// t = ns * freq / 1_000_000_000
// t * 1_000_000_000 / freq = ns
impl TickSupport for Duration {
    fn from_ticks(ticks: u64) -> Self {
        let frequency = TSC_FREQUENCY
            .get()
            .expect("must run init_sleep() before using time utilities");

        Self::from_nanos_u128((ticks as u128 * 1_000_000_000) / *frequency as u128)
    }

    fn to_ticks(self) -> u64 {
        let frequency = TSC_FREQUENCY
            .get()
            .expect("must run init_sleep() before using time utilities");

        ((self.as_nanos() * *frequency as u128) / 1_000_000_000) as u64
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Instant(u64);

impl Instant {
    pub fn now() -> Self {
        Self(tsc_now())
    }
}

impl core::ops::Add<u64> for Instant {
    type Output = Instant;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl core::ops::Add<Duration> for Instant {
    type Output = Instant;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0 + rhs.to_ticks())
    }
}

impl core::ops::Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        let Some(delta) = self.0.checked_sub(rhs.0) else {
            panic!(
                "Trying to subtract two Instants a - b where a < b. Negative durations are not supported."
            );
        };

        Duration::from_ticks(delta)
    }
}

#[must_use = "Sleep must be awaited"]
pub struct Sleep {
    target_tsc_tick: Instant,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = Instant::now();

        if now < self.target_tsc_tick {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

/// WARNING: This function cannot be called before `init_sleep()`!
pub fn sleep(delay: Duration) -> Sleep {
    Sleep {
        target_tsc_tick: Instant::now() + delay,
    }
}
