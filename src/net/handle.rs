use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;

pub struct Handle<T> {
    pub queue: ArrayQueue<T>,
    pub waker: AtomicWaker,
    _priv: (),
}

impl<T> Handle<T> {
    pub fn new(size: usize) -> Self {
        Self {
            queue: ArrayQueue::new(size),
            waker: AtomicWaker::new(),
            _priv: (),
        }
    }
}
