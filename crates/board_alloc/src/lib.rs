#![no_std]

use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

use embedded_alloc::LlffHeap as Heap;

pub const HEAP_SIZE: usize = 2048;

#[global_allocator]
static HEAP: Heap = Heap::empty();
static INIT_DONE: AtomicBool = AtomicBool::new(false);
static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];

pub fn init() {
    if INIT_DONE.swap(true, Ordering::AcqRel) {
        return;
    }

    let heap_start = core::ptr::addr_of_mut!(HEAP_MEM) as *mut MaybeUninit<u8> as usize;
    unsafe {
        HEAP.init(heap_start, HEAP_SIZE);
    }
}
