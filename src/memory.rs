use bootloader::bootinfo::{MemoryMap, MemoryRegionType};
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{FrameAllocator, PageTable, PhysFrame, Size4KiB};
use x86_64::structures::paging::{OffsetPageTable, Translate, mapper::TranslateResult};
use x86_64::{PhysAddr, VirtAddr};

unsafe fn get_active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let physical_address = level_4_table_frame.start_address();
    let virtual_address = physical_memory_offset + physical_address.as_u64();
    let page_table_ptr = virtual_address.as_mut_ptr();

    unsafe { &mut *page_table_ptr }
}

pub unsafe fn init(phys_offset: VirtAddr) -> OffsetPageTable<'static> {
    let l4_table = unsafe { get_active_level_4_table(phys_offset) };

    unsafe { OffsetPageTable::new(l4_table, phys_offset) }
}

pub trait MemoryMapper {
    fn to_physical<T>(&self, virt_address: *const T) -> u64;
    fn to_virt<T>(&self, phys_address: PhysAddr) -> *const T;
    fn to_virt_mut<T>(&self, phys_address: PhysAddr) -> *mut T;
}

impl<'a> MemoryMapper for OffsetPageTable<'a> {
    fn to_physical<T>(&self, virt_address: *const T) -> u64 {
        match self.translate(VirtAddr::from_ptr(virt_address)) {
            TranslateResult::Mapped { frame, offset, .. } => {
                frame.start_address().as_u64() + offset
            }
            _ => panic!("Virtual address could not be mapped to physical address"),
        }
    }

    fn to_virt<T>(&self, phys_address: PhysAddr) -> *const T {
        (self.phys_offset() + phys_address.as_u64()).as_ptr()
    }

    fn to_virt_mut<T>(&self, phys_address: PhysAddr) -> *mut T {
        (self.phys_offset() + phys_address.as_u64()).as_mut_ptr()
    }
}

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        self.memory_map
            .iter()
            .filter(|region| region.region_type == MemoryRegionType::Usable)
            .map(|region| region.range.start_addr()..region.range.end_addr())
            .flat_map(|range| range.step_by(4096))
            .map(|address| PhysFrame::containing_address(PhysAddr::new(address)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
