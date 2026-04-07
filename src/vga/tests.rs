use core::fmt::Write;

use crate::vga::driver::BUFFER_HEIGHT;

use super::WRITER;

#[test_case]
fn test_println_simple() {
    WRITER.lock().write_string("test_println_simple output");
}

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        WRITER.lock().write_string("test_println_many output");
    }
}

#[test_case]
fn test_println_output() {
    let s = "Some test string that fits on a single line";
    WRITER.lock().write_fmt(format_args!("{}\n", s)).unwrap();
    for (i, c) in s.chars().enumerate() {
        let screen_char = WRITER.lock().buffer.chars[BUFFER_HEIGHT - 2][i];
        assert_eq!(char::from(screen_char.ascii_character), c);
    }
}
