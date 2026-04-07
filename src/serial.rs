use core::fmt::Write;

use lazy_static::lazy_static;
use spin::Mutex;
use uart_16550::{self, Config, Uart16550, backend::PioBackend};

pub struct SerialPort(Uart16550<PioBackend>);

impl Write for SerialPort {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.send_bytes_exact(s.as_bytes());
        Ok(())
    }
}

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { Uart16550::new_port(0x3f8).unwrap() };
        serial_port.init(Config::default()).unwrap();
        Mutex::new(SerialPort(serial_port))
    };
}
