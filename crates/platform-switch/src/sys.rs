//! Raw FFI declarations: the C shim (`c/slynx_shim.h`) and the few libc functions the allocator needs.
//! Everything here is `unsafe` to call; the safe wrappers live in the other modules.

use crate::input_map::PadReading;
use core::ffi::{c_char, c_int, c_void};
use platform_api::Vertex;

pub const SLYNX_OK: c_int = 0;
pub const MAX_PLAYERS: usize = 4;

/// Mirrors `SlynxAppletState`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AppletState {
    pub focus: u32,
    pub docked: bool,
    pub resumed: bool,
}

/// Mirrors `SlynxDisplayHandles`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RawDisplayHandles {
    pub device: *mut c_void,
    pub queue: *mut c_void,
    pub swapchain: *mut c_void,
    pub images: *const c_void,
    pub image_count: u32,
    pub image_stride: u32,
    pub fb_width: u32,
    pub fb_height: u32,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub docked: bool,
}

unsafe extern "C" {
    // c/slynx_platform.c
    pub fn slynx_platform_init();
    pub fn slynx_platform_exit();
    pub fn slynx_applet_pump(out: *mut AppletState) -> bool;
    pub fn slynx_input_read(out: *mut PadReading);
    pub fn slynx_ticks_ns() -> u64;
    pub fn slynx_set_cpu_boost(on: bool) -> c_int;
    pub fn slynx_debug_log(msg: *const c_char, len: usize);
    pub fn slynx_abort() -> !;

    pub fn slynx_thread_start(
        entry: extern "C" fn(*mut c_void),
        arg: *mut c_void,
        stack_size: usize,
        prio: c_int,
        core: c_int,
        rc_out: *mut u32,
    ) -> *mut c_void;
    pub fn slynx_thread_join(thread: *mut c_void);
    pub fn slynx_thread_pin_current(core: c_int) -> bool;
    pub fn slynx_current_core() -> c_int;

    pub fn slynx_audio_open(frames: u32, buf_count: u32) -> u32;
    pub fn slynx_audio_acquire(timeout_ns: u64) -> *mut i16;
    pub fn slynx_audio_submit(samples: *mut i16) -> u32;
    pub fn slynx_audio_close();

    pub fn slynx_romfs_available() -> bool;
    pub fn slynx_fs_open_read(path: *const c_char, fd_out: *mut c_int, size_out: *mut u64) -> c_int;
    pub fn slynx_fs_read_at(fd: c_int, offset: u64, buf: *mut c_void, len: usize) -> i64;
    pub fn slynx_fs_close(fd: c_int);
    pub fn slynx_fs_exists(path: *const c_char) -> bool;
    pub fn slynx_fs_mkdirs(dir: *const c_char) -> c_int;
    pub fn slynx_fs_write_replace(path: *const c_char, tmp: *const c_char, data: *const c_void, len: usize) -> c_int;
    pub fn slynx_fs_remove(path: *const c_char) -> c_int;

    // c/slynx_display.c
    pub fn slynx_display_init(device_flags: u32, image_count: u32, docked: bool) -> c_int;
    pub fn slynx_display_set_docked(docked: bool);
    pub fn slynx_display_acquire() -> c_int;
    pub fn slynx_display_present(index: c_int);
    pub fn slynx_display_wait_idle();
    pub fn slynx_display_exit();
    pub fn slynx_display_handles(out: *mut RawDisplayHandles) -> bool;
    pub fn slynx_display_add_exit_hook(hook: unsafe extern "C" fn()) -> c_int;
    pub fn slynx_display_remove_exit_hook(hook: unsafe extern "C" fn());

    // c/slynx_debugdraw.c
    pub fn slynx_dd_init(max_vertices: u32) -> c_int;
    pub fn slynx_dd_frame(clear_rgba: *const f32, vertices: *const Vertex, count: u32) -> c_int;
    pub fn slynx_dd_exit();

    // newlib (linked through libnx's switch.specs)
    pub fn malloc(size: usize) -> *mut c_void;
    pub fn memalign(align: usize, size: usize) -> *mut c_void;
    pub fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void;
    pub fn free(ptr: *mut c_void);
}
