#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![feature(future_join)]
#![test_runner(rust_kernel::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::arch::x86_64::_rdtsc;
use core::future::join;
use core::panic::PanicInfo;
use core::time::Duration;

use bootloader::{BootInfo, entry_point};
use rust_kernel::allocator;
use rust_kernel::executor::block_on;
use rust_kernel::memory::{self, BootInfoFrameAllocator};
use rust_kernel::sleep::{calibrate, init_sleep, sleep};
use x86_64::VirtAddr;

entry_point!(kmain);

fn kmain(boot_info: &'static BootInfo) -> ! {
    rust_kernel::init();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    test_main();
    rust_kernel::hlt_loop()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::tests::test_panic_handler(info)
}

fn cycles_to_nanos(tsc_cycles: u64, frequency_hz: u64) -> u64 {
    (tsc_cycles as u128 * 1_000_000_000 / frequency_hz as u128) as u64
}

async fn sleep_elapsed_ns(delay: Duration) -> u64 {
    init_sleep().await;
    let frequency = calibrate().await;

    let before = unsafe { _rdtsc() };
    sleep(delay).await;
    let after = unsafe { _rdtsc() };

    cycles_to_nanos(after - before, frequency)
}

#[test_case]
fn sleep_waits_approximately_the_requested_duration() {
    let delay = Duration::from_millis(150);
    let target_ns = delay.as_nanos() as u64;

    let elapsed_ns = block_on(sleep_elapsed_ns(delay));

    assert!(
        elapsed_ns >= target_ns,
        "sleep returned early: waited {}ns, requested {}ns",
        elapsed_ns,
        target_ns,
    );
    // 20% tolerance
    assert!(
        elapsed_ns < target_ns * 6 / 5,
        "sleep overshot: waited {}ns, requested {}ns",
        elapsed_ns,
        target_ns,
    );
}

async fn sleep_short() -> u64 {
    sleep(Duration::from_millis(30)).await;
    unsafe { _rdtsc() }
}

async fn sleep_long() -> u64 {
    sleep(Duration::from_millis(150)).await;
    unsafe { _rdtsc() }
}

async fn sleep_short_long() -> (u64, u64) {
    init_sleep().await;

    join!(sleep_short(), sleep_long()).await
}

#[test_case]
fn longer_sleep_waits_longer() {
    let (short_time, long_time) = block_on(sleep_short_long());

    assert!(
        long_time > short_time,
        "expected sleeping longer to take more TSC cycles: short={}, long={}",
        short_time,
        long_time,
    );
}
