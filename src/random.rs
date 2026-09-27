use core::sync::atomic::{AtomicU64, Ordering};

static STATE: AtomicU64 = AtomicU64::new(0);

fn random_u64() -> u64 {
    let mut state = STATE.load(Ordering::Relaxed);

    if state == 0 {
        state = unsafe { core::arch::x86_64::_rdtsc() } | 1;
    }

    // xorshift64
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;

    STATE.store(state, Ordering::Relaxed);

    state
}

pub trait Random {
    fn random() -> Self;
}

macro_rules! impl_random {
    ($type:ty) => {
        impl Random for $type {
            fn random() -> Self {
                random_u64() as Self
            }
        }
    };
}

impl_random!(u64);
impl_random!(u32);
impl_random!(u16);
impl_random!(u8);
