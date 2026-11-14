//! Raw FFI declarations: the C shim (`c/slynx_shim.h`) and the few libc functions the allocator needs.
//! Everything here is `unsafe` to call; the safe wrappers live in `switch.rs`, `heap.rs` and `panic.rs`.

use crate::types::Vertex;
use core::ffi::{c_char, c_int, c_void};

pub const SLYNX_OK: c_int = 0;

unsafe extern "C" {
    // c/slynx_shim.c
    pub fn slynx_input_init();
    pub fn slynx_applet_main_loop() -> bool;
    pub fn slynx_input_poll(down: *mut u64, held: *mut u64);
    pub fn slynx_ticks_ns() -> u64;
    pub fn slynx_debug_log(msg: *const c_char, len: usize);
    pub fn slynx_abort() -> !;
    pub fn slynx_gfx_width() -> u32;
    pub fn slynx_gfx_height() -> u32;
    pub fn slynx_gfx_init(max_vertices: u32) -> c_int;
    pub fn slynx_gfx_frame(clear_rgba: *const f32, vertices: *const Vertex, count: u32) -> c_int;
    pub fn slynx_gfx_exit();

    // newlib (linked through libnx's switch.specs)
    pub fn malloc(size: usize) -> *mut c_void;
    pub fn memalign(align: usize, size: usize) -> *mut c_void;
    pub fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    pub fn free(ptr: *mut c_void);
}
