//! Global allocator backed by newlib's heap, which libnx sets up from the homebrew loader's heap at startup.

use crate::sys;
use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;

/// newlib's `malloc` on AArch64 returns 16-byte aligned blocks.
const MALLOC_ALIGN: usize = 16;

struct LibcHeap;

// SAFETY: every method forwards to the matching C heap function and honours `Layout`: plain `malloc`/`realloc`
// are used only when the requested alignment is at most what they guarantee, otherwise `memalign`. Blocks from
// either path are released with `free`, which newlib accepts for both.
unsafe impl GlobalAlloc for LibcHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: plain C calls; a null result is passed back as allocation failure.
        unsafe {
            if layout.align() <= MALLOC_ALIGN {
                sys::malloc(layout.size()) as *mut u8
            } else {
                sys::memalign(layout.align(), layout.size()) as *mut u8
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        // SAFETY: `ptr` came from `alloc`/`realloc` above (GlobalAlloc contract).
        unsafe { sys::free(ptr as *mut c_void) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() <= MALLOC_ALIGN {
            // SAFETY: `ptr` is a live block from this allocator; C realloc keeps malloc's alignment.
            return unsafe { sys::realloc(ptr as *mut c_void, new_size) as *mut u8 };
        }
        // Over-aligned: C realloc may lose the alignment, so allocate, copy and free.
        // SAFETY: the new layout is valid (same align, size checked by the caller); copy length is the smaller size.
        unsafe {
            let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
            let new_ptr = self.alloc(new_layout);
            if !new_ptr.is_null() {
                core::ptr::copy_nonoverlapping(ptr, new_ptr, layout.size().min(new_size));
                self.dealloc(ptr, layout);
            }
            new_ptr
        }
    }
}

#[global_allocator]
static HEAP: LibcHeap = LibcHeap;
