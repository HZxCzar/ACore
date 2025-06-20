mod address;
pub mod linked_list;
mod buddy_allocator;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;
pub use address::{PhysAddr, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
use address::VPNRange;
pub use frame_allocator::{FrameTracker, frame_alloc, frame_dealloc};
use heap_allocator::heap_test;
pub use memory_set::remap_test;
pub use memory_set::{KERNEL_SPACE, MapPermission, MemorySet, kernel_token};
use page_table::PTEFlags;
pub use page_table::{PageTable, PageTableEntry, UserBuffer, UserBufferIterator, translated_byte_buffer,
    translated_ref, translated_refmut, translated_str};
/// initiate heap allocator, frame allocator and kernel space
pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activate();
}
