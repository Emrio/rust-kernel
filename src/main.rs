#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader::{BootInfo, entry_point};
use core::panic::PanicInfo;
use rust_kernel::{hlt_loop, init, kprintln};

entry_point!(kmain);

fn kmain(boot_info: &'static BootInfo) -> ! {
    init();

    kprintln!("Hello World{}", "!");

    rust_kernel::drivers::i82540em::find_and_setup_ethernet_controller(
        boot_info.physical_memory_offset,
    );

    #[cfg(test)]
    test_main();

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
