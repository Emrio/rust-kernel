#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use rust_kernel::{
    kprintln,
    qemu::exit::{QemuExitCode, exit_qemu},
};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    kprintln!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    exit_qemu(QemuExitCode::Success)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!("{}", info);
    exit_qemu(QemuExitCode::Failed)
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::tests::test_panic_handler(info)
}
