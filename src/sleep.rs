use core::arch::x86_64::_rdtsc;
use core::pin::Pin;
use core::sync::atomic::{AtomicU64, Ordering};
use core::task::{Context, Poll};

use conquer_once::spin::OnceCell;
use x86_64::instructions::port::Port;

static TSC_FREQUENCY: OnceCell<u64> = OnceCell::uninit();

/**
 * Wait CALIBRATION_TICK_COUNT ticks from the programmable interval timer
 */
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

        let tsc_delta = unsafe { _rdtsc() } - self.tsc_begin;
        let tsc_frequency = (tsc_delta * PIT_FREQUENCY) / CALIBRATION_TICK_COUNT / 1_000_000;

        return Poll::Ready(tsc_frequency);
    }
}

pub fn calibrate() -> ClockCalibration {
    let tsc_begin = unsafe { _rdtsc() };
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

/// This function enables the use of the `sleep()` function.
/// WARNING: It also increases the PIT interrupt frequency from 18.2 Hz to 600 Hz!
pub async fn init_sleep() {
    set_pit_frequency_to_target();
    let tsc_frequency = calibrate().await;
    TSC_FREQUENCY.init_once(|| tsc_frequency);
}

pub struct Sleep {
    target_tsc_tick: u64,
}

impl Future for Sleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let now = unsafe { _rdtsc() };

        if now < self.target_tsc_tick {
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}

/// WARNING: This function cannot be called before `init_sleep()`!
pub fn sleep(delay: core::time::Duration) -> Sleep {
    let frequency = TSC_FREQUENCY
        .get()
        .expect("must run init_sleep() before using sleep()");
    let number_of_ticks = ((delay.as_nanos() * *frequency as u128) / 1_000_000_000) as u64;

    let now = unsafe { _rdtsc() };

    Sleep {
        target_tsc_tick: now + number_of_ticks,
    }
}
