//! Nintendo Switch platform layer for SlyNX.
//!
//! On the Switch (`target_os = "horizon"`, built with the `aarch64-nintendo-switch-freestanding` target) this crate
//! is `no_std` + `alloc` and provides:
//!
//! - a global allocator backed by libnx's C heap (`malloc`/`memalign`/`free`),
//! - the panic handler (logs the message, then aborts),
//! - safe wrappers over the C shim in `c/`: applet loop, input, time, debug log and a minimal deko3d renderer.
//!
//! On every other target the platform functions are absent and only the plain data types ([`Vertex`],
//! [`Buttons`]) are compiled, so the workspace still builds and tests on a PC.
#![cfg_attr(target_os = "horizon", no_std)]

extern crate alloc;

mod types;
pub use types::{Buttons, Vertex};

#[cfg(target_os = "horizon")]
mod sys;

#[cfg(target_os = "horizon")]
mod heap;

#[cfg(target_os = "horizon")]
mod panic;

#[cfg(target_os = "horizon")]
mod switch;
#[cfg(target_os = "horizon")]
pub use switch::*;
