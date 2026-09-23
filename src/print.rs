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
    x86_64::instructions::interrupts::without_interrupts(|| {
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    })
}

#[doc(hidden)]
pub fn _print_vga(args: core::fmt::Arguments) {
    use crate::vga::WRITER;
    use core::fmt::Write;
    x86_64::instructions::interrupts::without_interrupts(|| {
        WRITER
            .lock()
            .write_fmt(args)
            .expect("Printing to vga failed");
    })
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

#[macro_export]
macro_rules! dbg {
    ($val:expr) => {{
        let val = $val;
        kprintln!("{}:{} {} = {:?}", file!(), line!(), stringify!($val), val);
        val
    }};
}

pub mod colors {
    enum AnsiColor {
        Black,
        Red,
        Green,
        Yellow,
        Blue,
        Magenta,
        Cyan,
        White,
        BrightBlack,
        BrightRed,
        BrightGreen,
        BrightYellow,
        BrightBlue,
        BrightMagenta,
        BrightCyan,
        BrightWhite,
        Reset,
    }

    impl AnsiColor {
        fn code(&self) -> &'static str {
            match self {
                Self::Black => "\x1b[30m",
                Self::Red => "\x1b[31m",
                Self::Green => "\x1b[32m",
                Self::Yellow => "\x1b[33m",
                Self::Blue => "\x1b[34m",
                Self::Magenta => "\x1b[35m",
                Self::Cyan => "\x1b[36m",
                Self::White => "\x1b[37m",
                Self::BrightBlack => "\x1b[90m",
                Self::BrightRed => "\x1b[91m",
                Self::BrightGreen => "\x1b[92m",
                Self::BrightYellow => "\x1b[93m",
                Self::BrightBlue => "\x1b[94m",
                Self::BrightMagenta => "\x1b[95m",
                Self::BrightCyan => "\x1b[96m",
                Self::BrightWhite => "\x1b[97m",
                Self::Reset => "\x1b[0m",
            }
        }
    }

    pub struct Colored<T: core::fmt::Display> {
        color: AnsiColor,
        value: T,
    }

    impl<T: core::fmt::Display> core::fmt::Display for Colored<T> {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.write_fmt(format_args!(
                "{}{}{}",
                self.color.code(),
                self.value,
                AnsiColor::Reset.code()
            ))
        }
    }

    macro_rules! impl_color {
        ($f:ident, $color:expr) => {
            fn $f(self) -> Colored<Self> {
                Colored {
                    color: $color,
                    value: self,
                }
            }
        };
    }

    pub trait Colorable
    where
        Self: core::fmt::Display + Sized,
    {
        impl_color!(black, AnsiColor::Black);
        impl_color!(red, AnsiColor::Red);
        impl_color!(green, AnsiColor::Green);
        impl_color!(yellow, AnsiColor::Yellow);
        impl_color!(blue, AnsiColor::Blue);
        impl_color!(magenta, AnsiColor::Magenta);
        impl_color!(cyan, AnsiColor::Cyan);
        impl_color!(white, AnsiColor::White);
        impl_color!(gray, AnsiColor::BrightBlack);
        impl_color!(bright_red, AnsiColor::BrightRed);
        impl_color!(bright_green, AnsiColor::BrightGreen);
        impl_color!(bright_yellow, AnsiColor::BrightYellow);
        impl_color!(bright_blue, AnsiColor::BrightBlue);
        impl_color!(bright_magenta, AnsiColor::BrightMagenta);
        impl_color!(bright_cyan, AnsiColor::BrightCyan);
        impl_color!(bright_white, AnsiColor::BrightWhite);
    }

    impl<T: core::fmt::Display> Colorable for T {}
}

#[macro_export]
macro_rules! klog {
    ($service:expr, $($arg:expr),* $(,)?) => {{
        $crate::kprint!("[{}] ", $crate::print::colors::Colorable::gray($service));
        $( $crate::kprint!("{}", $arg); )*
        $crate::kprint!("\n");
    }};
}
