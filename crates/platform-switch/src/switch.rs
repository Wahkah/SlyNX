//! Safe wrappers over the C shim. Each `unsafe` block is a direct call into the shim whose preconditions are
//! stated next to it.

use crate::sys;
use crate::types::{Buttons, Vertex};
use core::fmt;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, Ordering};

/// Writes a line to the debug log (`svcOutputDebugString`).
pub fn debug_log(msg: &str) {
    debug_log_bytes(msg.as_bytes());
}

/// Writes raw bytes to the debug log.
pub fn debug_log_bytes(msg: &[u8]) {
    // SAFETY: pointer and length describe a live slice; the shim only reads it.
    unsafe { sys::slynx_debug_log(msg.as_ptr().cast(), msg.len()) }
}

/// Aborts the program (libnx returns to the Homebrew Menu with an error).
pub fn abort() -> ! {
    // SAFETY: no preconditions.
    unsafe { sys::slynx_abort() }
}

/// Nanoseconds since the console booted, from the system tick counter.
pub fn ticks_ns() -> u64 {
    // SAFETY: no preconditions; reads a CPU counter.
    unsafe { sys::slynx_ticks_ns() }
}

/// Runs the applet message loop once. Returns `false` when the system asks the program to exit
/// (for example the user closed it from the HOME menu); the caller should then leave its main loop.
pub fn applet_main_loop() -> bool {
    // SAFETY: no preconditions; libnx's applet service is initialised by its startup code.
    unsafe { sys::slynx_applet_main_loop() }
}

/// Buttons newly pressed this frame and buttons held.
#[derive(Clone, Copy, Debug, Default)]
pub struct PadState {
    pub down: Buttons,
    pub held: Buttons,
}

/// Player 1's controller (also reads handheld mode). Only one may exist at a time.
pub struct Pad {
    _not_send: PhantomData<*mut ()>,
}

static PAD_TAKEN: AtomicBool = AtomicBool::new(false);

impl Pad {
    /// Configures input for one player with the standard controller styles. Returns `None` if a `Pad` already exists.
    pub fn new() -> Option<Pad> {
        if PAD_TAKEN.swap(true, Ordering::AcqRel) {
            return None;
        }
        // SAFETY: called once (guarded above); the shim keeps the pad state in a static.
        unsafe { sys::slynx_input_init() };
        Some(Pad { _not_send: PhantomData })
    }

    /// Reads the controller. Call once per frame.
    pub fn poll(&mut self) -> PadState {
        let (mut down, mut held) = (0u64, 0u64);
        // SAFETY: both pointers are to live locals; `&mut self` serialises access to the shim's pad state.
        unsafe { sys::slynx_input_poll(&mut down, &mut held) };
        PadState { down: Buttons(down), held: Buttons(held) }
    }
}

impl Drop for Pad {
    fn drop(&mut self) {
        PAD_TAKEN.store(false, Ordering::Release);
    }
}

/// Error from the graphics shim (the shim's `SLYNX_ERR_*` code).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GfxError(pub i32);

impl fmt::Display for GfxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.0 {
            1 => "already initialised",
            2 => "not initialised",
            3 => "out of memory",
            4 => "bad argument",
            _ => "unknown error",
        };
        write!(f, "gfx error {} ({what})", self.0)
    }
}

/// The deko3d device, swapchain and built-in colour pipeline. Only one may exist at a time; dropping it waits for
/// the GPU to go idle and frees everything.
pub struct Gfx {
    max_vertices: u32,
    _not_send: PhantomData<*mut ()>,
}

impl Gfx {
    /// Creates the device and a 1280x720 double-buffered swapchain that can draw up to `max_vertices` per frame.
    pub fn new(max_vertices: u32) -> Result<Gfx, GfxError> {
        // SAFETY: the shim refuses a second init itself, so no extra guard is needed here.
        let rc = unsafe { sys::slynx_gfx_init(max_vertices) };
        if rc != sys::SLYNX_OK {
            return Err(GfxError(rc));
        }
        Ok(Gfx { max_vertices, _not_send: PhantomData })
    }

    /// Framebuffer size in pixels.
    pub fn size(&self) -> (u32, u32) {
        // SAFETY: no preconditions; returns constants.
        unsafe { (sys::slynx_gfx_width(), sys::slynx_gfx_height()) }
    }

    /// Most vertices one frame can draw; extra vertices are dropped.
    pub fn max_vertices(&self) -> u32 {
        self.max_vertices
    }

    /// Clears to `clear` (linear RGBA), draws `vertices` as a triangle list and presents. Blocks while the GPU is
    /// still using this frame slot's memory from two frames ago and while waiting for a free swapchain image.
    pub fn frame(&mut self, clear: [f32; 4], vertices: &[Vertex]) -> Result<(), GfxError> {
        let count = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
        // SAFETY: `clear` is 4 floats and `vertices` a live slice of `count` (or more) `#[repr(C)]` vertices; the
        // shim copies them before returning and clamps `count` to `max_vertices`.
        let rc = unsafe { sys::slynx_gfx_frame(clear.as_ptr(), vertices.as_ptr(), count) };
        if rc == sys::SLYNX_OK { Ok(()) } else { Err(GfxError(rc)) }
    }
}

impl Drop for Gfx {
    fn drop(&mut self) {
        // SAFETY: `self` is the only Gfx; after this the shim is uninitialised and a new Gfx may be created.
        unsafe { sys::slynx_gfx_exit() }
    }
}
