#![no_std]
#![no_main]

#[macro_use]
mod kprintln;
mod panic;
mod vga;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    kprintln!("Hello World{}", "!");

    loop {}
}
