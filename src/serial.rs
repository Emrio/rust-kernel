extern crate alloc;
use alloc::vec::Vec;
use core::{
    fmt::Write,
    pin::Pin,
    task::{Context, Poll},
};

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use futures_util::{Stream, StreamExt, task::AtomicWaker};
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

impl SerialPort {
    pub fn try_receive_byte(&mut self) -> Option<u8> {
        self.0.try_receive_byte().ok()
    }
}

static SERIAL_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

pub(crate) fn add_serial_byte(byte: u8) {
    let Ok(queue) = SERIAL_QUEUE.try_get() else {
        kprintln!("WARNING: byte queue uninitialized");
        return;
    };

    if queue.push(byte).is_err() {
        kprintln!("WARNING: byte queue full; dropping keyboard input");
        return;
    }

    WAKER.wake();
}

pub struct SerialStream {
    _private: (),
}

impl SerialStream {
    fn new() -> Self {
        SERIAL_QUEUE
            .try_init_once(|| ArrayQueue::new(100))
            .expect("SerialStream::new should only be called once");
        Self { _private: () }
    }
}

impl Stream for SerialStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        let queue = SERIAL_QUEUE.try_get().expect("byte queue not initialized");

        if let Some(byte) = queue.pop() {
            return Poll::Ready(Some(byte));
        }

        WAKER.register(cx.waker());
        match queue.pop() {
            Some(byte) => {
                WAKER.take();
                Poll::Ready(Some(byte))
            }
            None => Poll::Pending,
        }
    }
}

pub async fn print_keypresses() {
    let mut bytes = SerialStream::new();
    let mut buffer = Vec::new();

    while let Some(byte) = bytes.next().await {
        match byte {
            b'\r' | b'\n' => {
                kprint!("\r\n");
                buffer.clear();
            }
            0x7f | 0x08 => {
                if buffer.pop().is_some() {
                    kprint!("\x08 \x08");
                }
            }
            byte => {
                buffer.push(byte);
                kprint!("{}", byte as char);
            }
        }
    }
}
