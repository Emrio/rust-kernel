#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(future_join)]
extern crate alloc;

use bootloader::{BootInfo, entry_point};
use core::future::join;
use core::panic::PanicInfo;
use rust_kernel::executor::block_on;
use rust_kernel::keyboard;
use rust_kernel::memory::init_memory;
use rust_kernel::net;
use rust_kernel::time::init_time;
use rust_kernel::{hlt_loop, init, kprintln};

entry_point!(kmain);

fn kmain(boot_info: &'static BootInfo) -> ! {
    init();

    kprintln!("Hello World{}", "!");

    init_memory(boot_info.physical_memory_offset, &boot_info.memory_map);

    #[cfg(test)]
    test_main();

    block_on(init_time());

    rust_kernel::drivers::i82540em::find_and_setup_ethernet_controller();
    block_on(join!(
        net::rx_loop(),
        net::net_loop(),
        keyboard::print_keypresses()
    ));

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
