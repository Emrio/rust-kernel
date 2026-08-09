use lazy_static::lazy_static;
use pic8259::ChainedPics;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::{drivers::i82540em::DEVICE, gdt, net};

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

#[derive(Debug, Clone, Copy)]
struct InterruptIndex(u8);

impl InterruptIndex {
    const fn new(irq_line: u8) -> Self {
        Self(irq_line)
    }

    const fn into_index(self) -> u8 {
        if self.0 < 8 {
            PIC_1_OFFSET + self.0
        } else {
            PIC_2_OFFSET + self.0 - 8
        }
    }

    const TIMER: Self = Self::new(0);
    const KEYBOARD: Self = Self::new(1);
    const ETHERNET_RX: Self = Self::new(11);
}

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt[InterruptIndex::TIMER.into_index()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::KEYBOARD.into_index()].set_handler_fn(keyboard_interrupt_handler);
        idt[InterruptIndex::ETHERNET_RX.into_index()].set_handler_fn(handle_ethernet_frame);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    kprintln!("Exception: breakpoint\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("Exception: double fault\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // WARNING: kprint!ing here hangs rust_kernel::vga::tests::test_println_many but I haven't figured out why...

    crate::time::tick_calibrator();

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::TIMER.into_index());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::keyboard::add_scancode(scancode);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::KEYBOARD.into_index());
    }
}

extern "x86-interrupt" fn handle_ethernet_frame(_stack_frame: InterruptStackFrame) {
    net::rx::WAKER.wake();

    if let Some(device) = DEVICE.get() {
        device.on_rx_interrupt()
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::ETHERNET_RX.into_index());
    }
}

#[cfg(test)]
mod tests {
    #[test_case]
    fn test_breakpoint_exception() {
        x86_64::instructions::interrupts::int3();
    }
}
