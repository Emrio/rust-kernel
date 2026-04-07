#[macro_export]
macro_rules! kprintln {
    () => (kprint!("\n"));
    ($($arg:tt)*) => (kprint!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ($crate::kprintln::_print(format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use crate::vga::WRITER;
    use core::fmt::Write;
    WRITER.lock().write_fmt(args).unwrap();
}
