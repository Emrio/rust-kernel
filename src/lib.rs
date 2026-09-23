#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::tests::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

#[macro_use]
pub mod print;
pub mod allocator;
pub mod bits;
pub mod drivers;
pub mod executor;
pub mod gdt;
pub mod interrupts;
pub mod keyboard;
pub mod memory;
pub(crate) mod mmio;
pub mod net;
pub mod pci;
pub mod qemu;
pub mod serial;
pub mod tests;
pub mod time;
pub mod vga;

pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    unsafe {
        let masks = interrupts::PICS.lock().read_masks();
        interrupts::PICS.lock().write_masks(masks[0] & !(1 << 4), masks[1]);
    }
    x86_64::instructions::interrupts::enable();
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
