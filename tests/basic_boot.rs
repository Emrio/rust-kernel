#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

use rust_kernel::{hlt_loop, kprintln};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    test_main();

    hlt_loop()
}

#[allow(dead_code)]
fn test_runner(_tests: &[&dyn Fn()]) {
    unimplemented!();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::tests::test_panic_handler(info)
}

#[test_case]
fn test_println() {
    kprintln!("test_println output");
}
