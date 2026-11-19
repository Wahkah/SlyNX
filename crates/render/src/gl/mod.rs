//! OpenGL 4.3 core backend (PC).
//!
//! The platform layer supplies a current context and its `get_proc_address`; [`GlBackend::new`] loads the entry
//! points it needs by hand (no binding crates). On Linux, [`egl::HeadlessGl`] makes a windowless context for tests
//! and screenshots.

pub mod api;
mod backend;

#[cfg(target_os = "linux")]
pub mod egl;

pub use backend::{FRAME_SLOTS, GlBackend, GlTarget, MAX_DRAWS, MAX_INSTANCES};
