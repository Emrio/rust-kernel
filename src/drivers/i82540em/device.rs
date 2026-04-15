pub struct Device {
    base_address: *mut u32,
}

impl Device {
    pub(super) fn from(memory_offset: u64, bar0: u32) -> Self {
        let base_address = memory_offset + (bar0 & 0xfffffff8u32) as u64;

        Self {
            base_address: base_address as *mut u32,
        }
    }

    pub(super) fn write_register(&self, register: usize, value: u32) {
        unsafe { self.base_address.byte_add(register).write_volatile(value) }
    }

    pub(super) fn read_register(&self, register: usize) -> u32 {
        unsafe { self.base_address.byte_add(register).read_volatile() }
    }
}
