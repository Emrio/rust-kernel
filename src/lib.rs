#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

#[macro_use]
pub mod print;
pub mod interrupts;
pub mod qemu;
pub mod serial;
pub mod tests;
pub mod vga;

pub fn init() {
    interrupts::init_idt();
}
