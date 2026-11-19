//! The display hand-off to the renderer, and the debug draw.

use crate::sys;
use core::ffi::c_void;
use core::fmt;
use core::marker::PhantomData;
use platform_api::{DebugDraw, Vertex};

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
            5 => "deko3d object creation failed",
            _ => "unknown error",
        };
        write!(f, "gfx error {} ({what})", self.0)
    }
}

/// The deko3d objects the platform owns, for the renderer. The pointers stay valid until the platform is dropped
/// and do not change when the console is docked or undocked; `width`, `height` and `docked` do, so read them every
/// frame (or on `Event::DisplayChanged`).
///
/// In C the same objects are `slynx_display_get()->device` and so on (`c/slynx_shim.h`), with real deko3d types.
#[derive(Clone, Copy, Debug)]
pub struct DisplayHandles {
    /// `DkDevice`.
    pub device: *mut c_void,
    /// `DkQueue`, a graphics queue. Submit the frame's commands here; acquire and present use it too.
    pub queue: *mut c_void,
    /// `DkSwapchain`.
    pub swapchain: *mut c_void,
    /// Pointer to the first `DkImage` of the swapchain; image `i` is at `images + i * image_stride` bytes.
    pub images: *const c_void,
    pub image_count: u32,
    pub image_stride: u32,
    /// Allocated image size: always 1920x1080.
    pub fb_width: u32,
    pub fb_height: u32,
    /// Current output size: 1920x1080 docked, 1280x720 handheld (top-left corner of the image).
    pub width: u32,
    pub height: u32,
    /// `DkImageFormat` of the images (`RGBA8_Unorm`).
    pub format: u32,
    pub docked: bool,
}

impl DisplayHandles {
    pub(crate) fn query() -> Option<DisplayHandles> {
        let mut raw = sys::RawDisplayHandles {
            device: core::ptr::null_mut(),
            queue: core::ptr::null_mut(),
            swapchain: core::ptr::null_mut(),
            images: core::ptr::null(),
            image_count: 0,
            image_stride: 0,
            fb_width: 0,
            fb_height: 0,
            width: 0,
            height: 0,
            format: 0,
            docked: false,
        };
        // SAFETY: `raw` is a live, writable struct with the C layout.
        if !unsafe { sys::slynx_display_handles(&mut raw) } {
            return None;
        }
        Some(DisplayHandles {
            device: raw.device,
            queue: raw.queue,
            swapchain: raw.swapchain,
            images: raw.images,
            image_count: raw.image_count,
            image_stride: raw.image_stride,
            fb_width: raw.fb_width,
            fb_height: raw.fb_height,
            width: raw.width,
            height: raw.height,
            format: raw.format,
            docked: raw.docked,
        })
    }
}

/// A single-pipeline triangle renderer on the platform's display, for demos and bring-up. Only one may exist at a
/// time; it must not be used at the same time as the real renderer.
pub struct SwitchDebugDraw {
    max_vertices: u32,
    /// The shim keeps the debug draw's state in C globals shared with the main thread's platform, so the handle
    /// must stay on that thread: not `Send`, not `Sync`.
    _not_send: PhantomData<*mut ()>,
}

impl SwitchDebugDraw {
    /// Needs a live `SwitchPlatform` (which owns the display). The debug draw registers a display exit hook, so
    /// dropping the platform first releases its GPU memory while the device still exists; frames after that fail
    /// with "not initialised" and its own Drop does nothing.
    pub fn new(_platform: &crate::SwitchPlatform, max_vertices: u32) -> Result<SwitchDebugDraw, GfxError> {
        // SAFETY: the shim checks that the display exists and refuses a second init.
        let rc = unsafe { sys::slynx_dd_init(max_vertices) };
        if rc != sys::SLYNX_OK {
            return Err(GfxError(rc));
        }
        Ok(SwitchDebugDraw { max_vertices, _not_send: PhantomData })
    }
}

impl DebugDraw for SwitchDebugDraw {
    type Error = GfxError;

    fn max_vertices(&self) -> usize {
        self.max_vertices as usize
    }

    fn frame(&mut self, clear: [f32; 4], vertices: &[Vertex]) -> Result<(), GfxError> {
        let count = u32::try_from(vertices.len()).unwrap_or(u32::MAX);
        // SAFETY: `clear` is 4 floats and `vertices` a live slice of `count` (or more) `#[repr(C)]` vertices laid out
        // like `SlynxVertex`; the shim copies them before returning and clamps `count` to `max_vertices`.
        let rc = unsafe { sys::slynx_dd_frame(clear.as_ptr(), vertices.as_ptr(), count) };
        if rc == sys::SLYNX_OK { Ok(()) } else { Err(GfxError(rc)) }
    }
}

impl Drop for SwitchDebugDraw {
    fn drop(&mut self) {
        // SAFETY: idempotent; waits for the GPU before freeing.
        unsafe { sys::slynx_dd_exit() }
    }
}
