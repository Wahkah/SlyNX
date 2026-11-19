//! Nintendo Switch backend of the SlyNX platform layer.
//!
//! On the Switch (`target_os = "horizon"`, built with the `aarch64-nintendo-switch-freestanding` target) this crate
//! is `no_std` + `alloc` and provides:
//!
//! - [`SwitchPlatform`], the [`platform_api::Platform`] implementation: applet events, input for up to four players
//!   (handheld, Joy-Con pairs, single Joy-Cons, Pro Controller), audio output through audout on core 2, RomFS and SD
//!   card files with a writable user folder, threads pinned to cores, the monotonic clock and CPU boost;
//! - the display: the deko3d device, graphics queue and swapchain the renderer draws with
//!   ([`SwitchPlatform::display_handles`], or `slynx_display_get()` from C);
//! - [`SwitchDebugDraw`], a one-pipeline triangle renderer for demos;
//! - the global allocator (newlib's heap) and the panic handler (debug log, then abort).
//!
//! On every other target only the input mapping (tested on the host) is compiled, so the workspace builds and tests
//! on a PC.
#![cfg_attr(target_os = "horizon", no_std)]

extern crate alloc;

pub use platform_api;

mod input_map;

#[cfg(target_os = "horizon")]
mod audio;
#[cfg(target_os = "horizon")]
mod display;
#[cfg(target_os = "horizon")]
mod fs;
#[cfg(target_os = "horizon")]
mod heap;
#[cfg(target_os = "horizon")]
mod panic;
#[cfg(target_os = "horizon")]
mod platform;
#[cfg(target_os = "horizon")]
mod sys;
#[cfg(target_os = "horizon")]
mod thread;

#[cfg(target_os = "horizon")]
pub use display::{DisplayHandles, GfxError, SwitchDebugDraw};
#[cfg(target_os = "horizon")]
pub use fs::{SwitchFile, SwitchFs, USER_DIR};
#[cfg(target_os = "horizon")]
pub use platform::{SwitchConfig, SwitchPlatform, abort, current_core, debug_log, debug_log_bytes};
#[cfg(target_os = "horizon")]
pub use thread::SwitchThread;
