mod driver;

use lazy_static::lazy_static;
use spin::Mutex;

use crate::vga::driver::{Buffer, Color, Writer};

impl core::fmt::Write for Writer<'static> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

lazy_static! {
    pub static ref WRITER: Mutex<Writer<'static>> =
        Mutex::new(Writer::new(Color::Yellow, Color::Black, unsafe {
            &mut *(0xb8000 as *mut Buffer)
        }));
}

#[cfg(test)]
mod tests;
