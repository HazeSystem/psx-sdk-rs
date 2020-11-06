use core::alloc::{GlobalAlloc, Layout};

use crate::bios;

pub struct BiosAllocator;

impl BiosAllocator {
    pub fn init(addr: usize, size: usize) {
        bios::init_heap(addr, size);
    }
}

unsafe impl GlobalAlloc for BiosAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        bios::malloc(layout.size())
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        bios::free(ptr)
    }
}

#[global_allocator]
pub static HEAP: BiosAllocator = BiosAllocator;

#[alloc_error_handler]
fn on_oom(_layout: Layout) -> ! { loop { } }
