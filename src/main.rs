#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![reexport_test_harness_main = "test_main"]
#![test_runner(crate::test_runner)]

#[cfg(test)]
use core::panic::PanicInfo;

use crate::qemu::exit::{QemuExitCode, exit_qemu};

#[macro_use]
mod kprint;
mod panic;
mod qemu;
mod serial;
mod vga;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    kprintln!("Hello World{}", "!");

    #[cfg(test)]
    test_main();

    exit_qemu(QemuExitCode::Success)
}

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        kprint!("{}...\t", core::any::type_name::<T>());
        self();
        kprintln!("[ok]");
    }
}

#[cfg(test)]
pub fn test_runner(tests: &[&dyn Testable]) {
    kprintln!("Running {} tests", tests.len());
    for test in tests {
        test.run(); // new
    }
    exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn trivial_assertion() {
    assert_eq!(1, 1);
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!("[failed]\n");
    kprintln!("Error: {}\n", info);

    exit_qemu(QemuExitCode::Failed);
}
