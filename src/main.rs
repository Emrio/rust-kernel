#![no_std]
#![no_main]

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

    exit_qemu(QemuExitCode::Success)
}
