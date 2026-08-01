use x86_64::registers::control::Cr3;
use x86_64::structures::paging::PageTable;
use x86_64::structures::paging::{OffsetPageTable, Translate, mapper::TranslateResult};
use x86_64::{PhysAddr, VirtAddr};

pub struct MemoryMapper<'a> {
    mapper: OffsetPageTable<'a>,
    physical_memory_offset: VirtAddr,
}

impl<'a> MemoryMapper<'a> {
    pub unsafe fn new(physical_memory_offset: VirtAddr) -> Self {
        let l4_table = unsafe { get_active_level_4_table(physical_memory_offset) };

        let mapper = unsafe { OffsetPageTable::new(l4_table, physical_memory_offset) };

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

    pub fn to_virt<T>(&self, phys_address: PhysAddr) -> *const T {
        (self.physical_memory_offset + phys_address.as_u64()).as_ptr()
    }

    pub fn to_virt_mut<T>(&self, phys_address: PhysAddr) -> *mut T {
        (self.physical_memory_offset + phys_address.as_u64()).as_mut_ptr()
    }
}

pub unsafe fn get_active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let physical_address = level_4_table_frame.start_address();
    let virtual_address = physical_memory_offset + physical_address.as_u64();
    let page_table_ptr = virtual_address.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}
