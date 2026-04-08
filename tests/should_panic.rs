#![no_std]
#![no_main]

use core::panic::PanicInfo;

use rust_kernel::qemu::exit::{QemuExitCode, exit_qemu};
use rust_kernel::{test_print, test_println};

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    should_fail();
    test_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
}

fn should_fail() {
    test_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    test_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
}
