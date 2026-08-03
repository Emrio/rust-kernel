extern crate alloc;

use alloc::sync::Arc;
use alloc::task::Wake;
use core::pin::pin;
use core::task::{Context, Poll};
use x86_64::instructions::interrupts;

struct Waker;

impl Wake for Waker {
    fn wake(self: Arc<Self>) {}
    fn wake_by_ref(self: &Arc<Self>) {}
}

pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    let waker = core::task::Waker::from(Arc::new(Waker));
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        interrupts::disable();
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(result) => {
                interrupts::enable();
                return result;
            }
            Poll::Pending => interrupts::enable_and_hlt(),
        }
    }
}
