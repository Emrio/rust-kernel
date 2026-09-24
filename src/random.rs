use core::sync::atomic::{AtomicU64, Ordering};

static STATE: AtomicU64 = AtomicU64::new(0);

pub fn random_u32() -> u32 {
    let mut state = STATE.load(Ordering::Relaxed);

    if state == 0 {
        state = unsafe { core::arch::x86_64::_rdtsc() } | 1;
    }

    // xorshift64
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;

    STATE.store(state, Ordering::Relaxed);

    state as u32
}
