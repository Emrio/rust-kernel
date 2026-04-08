#[macro_export]
macro_rules! kprintln {
    () => ($crate::kprint!("\n"));
    ($($arg:tt)*) => ($crate::kprint!("{}\n", format_args!($($arg)*)));
}

#[cfg(not(feature = "kprint-vga"))]
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ($crate::print::_print_serial(format_args!($($arg)*)));
}

#[cfg(feature = "kprint-vga")]
#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ($crate::print::_print_vga(format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print_serial(args: core::fmt::Arguments) {
    use crate::serial::SERIAL1;
    use core::fmt::Write;
    SERIAL1
        .lock()
        .write_fmt(args)
        .expect("Printing to serial failed");
}

#[doc(hidden)]
pub fn _print_vga(args: core::fmt::Arguments) {
    use crate::vga::WRITER;
    use core::fmt::Write;
    WRITER
        .lock()
        .write_fmt(args)
        .expect("Printing to vga failed");
}

// used by test framework

#[macro_export]
macro_rules! test_println {
    () => ($crate::test_print!("\n"));
    ($($arg:tt)*) => ($crate::test_print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! test_print {
    ($($arg:tt)*) => ($crate::print::_print_serial(format_args!($($arg)*)));
}
