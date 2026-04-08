use core::panic::PanicInfo;

use crate::qemu::exit::{QemuExitCode, exit_qemu};

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        test_print!("{}...\t", core::any::type_name::<T>());
        self();
        test_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    test_println!("Running {} tests", tests.len());
    kprintln!("test_println output");
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    test_println!("[failed]\n");
    test_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

/// Entry point for `cargo test`
#[cfg(test)]
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    super::init();
    super::test_main();
    loop {}
}
