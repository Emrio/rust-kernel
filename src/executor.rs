extern crate alloc;

use alloc::sync::Arc;
use alloc::task::Wake;
use core::pin::pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use x86_64::instructions::interrupts;

/**
 * This waker implementation works fine while all futures rely:
 * - on interrupts (un-hlt the thread);
 * - or update internal logic (caught by the atomic boolean).
 */
struct MyWaker {
    notified: AtomicBool,
}

impl MyWaker {
    fn new() -> Self {
        Self {
            notified: AtomicBool::new(false),
        }
    }

    fn notify(&self) {
        self.notified.store(true, Ordering::Release);
    }
}

impl Wake for MyWaker {
    fn wake(self: Arc<Self>) {
        self.notify();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.notify();
    }
}

pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    let mywaker = Arc::new(MyWaker::new());
    let waker = Waker::from(mywaker.clone());
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        interrupts::disable();
        mywaker.notified.store(false, Ordering::Relaxed);

        if let Poll::Ready(result) = future.as_mut().poll(&mut cx) {
            interrupts::enable();
            return result;
        }

        if !mywaker.notified.load(Ordering::Acquire) {
            interrupts::enable_and_hlt()
        } else {
            interrupts::enable()
        }
    }
}
