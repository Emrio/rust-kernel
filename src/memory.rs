use x86_64::VirtAddr;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{OffsetPageTable, Translate, mapper::TranslateResult};

pub struct MemoryMapper<'a> {
    mapper: OffsetPageTable<'a>,
    physical_memory_offset: u64,
}

impl<'a> MemoryMapper<'a> {
    pub unsafe fn new(physical_memory_offset: u64) -> Self {
        let (l4_frame, _) = Cr3::read();
        let l4_phys = l4_frame.start_address();

        let l4_virt = VirtAddr::new(l4_phys.as_u64() + physical_memory_offset);
        let l4_table = unsafe { &mut *l4_virt.as_mut_ptr() };

        let mapper =
            unsafe { OffsetPageTable::new(l4_table, VirtAddr::new(physical_memory_offset)) };

        Self {
            mapper,
            physical_memory_offset,
        }
    }

    pub fn to_physical<T>(&self, virt_address: *const T) -> u64 {
        match self.mapper.translate(VirtAddr::from_ptr(virt_address)) {
            TranslateResult::Mapped { frame, offset, .. } => {
                frame.start_address().as_u64() + offset
            }
            _ => panic!("Virtual address could not be mapped to physical address"),
        }
    }

    pub fn to_virt<T>(&self, phys_address: u64) -> *const T {
        (phys_address + self.physical_memory_offset) as *const T
    }

    pub fn to_virt_mut<T>(&self, phys_address: u64) -> *mut T {
        (phys_address + self.physical_memory_offset) as *mut T
    }
}
