#[derive(Debug, Clone, Copy)]
pub struct MmioPtr<T>(*mut T);

impl<T> core::fmt::Display for MmioPtr<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:#x}", self.0 as u64))
    }
}

impl<T> MmioPtr<T> {
    pub fn new(ptr: *mut T) -> Self {
        Self(ptr)
    }

    pub unsafe fn byte_add(self, count: usize) -> Self {
        Self(unsafe { self.0.byte_add(count) })
    }

    pub unsafe fn write_volatile(&self, val: T) {
        unsafe { self.0.write_volatile(val) }
    }

    pub unsafe fn read_volatile(&self) -> T {
        unsafe { self.0.read_volatile() }
    }
}

unsafe impl<T> Sync for MmioPtr<T> {}
unsafe impl<T> Send for MmioPtr<T> {}
