#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[macro_use]
pub mod print;
pub mod qemu;
pub mod serial;
pub mod tests;
pub mod vga;
