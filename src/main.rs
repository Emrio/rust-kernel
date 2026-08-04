#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
extern crate alloc;

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use rust_kernel::allocator::init_heap;
use rust_kernel::executor::block_on;
use rust_kernel::keyboard;
use rust_kernel::memory::{self, BootInfoFrameAllocator};
use rust_kernel::sleep::init_sleep;
use rust_kernel::{hlt_loop, init, kprintln};
use x86_64::VirtAddr;

entry_point!(kmain);

fn kmain(boot_info: &'static BootInfo) -> ! {
    init();

    kprintln!("Hello World{}", "!");

    let mut mapper = unsafe { memory::init(VirtAddr::new(boot_info.physical_memory_offset)) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    #[cfg(test)]
    test_main();

    block_on(init_sleep());

    // rust_kernel::drivers::i82540em::find_and_setup_ethernet_controller(&mapper);

    block_on(keyboard::print_keypresses());

    hlt_loop()
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use rust_kernel::qemu::exit::{QemuExitCode, exit_qemu};
    kprintln!("{}", info);
    exit_qemu(QemuExitCode::Failed)
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::tests::test_panic_handler(info)
}
