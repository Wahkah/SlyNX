//! Test and verification harness for SlyNX.
//!
//! The harness proves on PC that the ports behave: it drives an [`App`] with a fixed timestep, a seeded random
//! number generator and scripted input, captures frames as images, compares them with references, and records
//! per-frame performance numbers.
//!
//! Modules:
//!
//! - [`app`]: the [`App`] trait the game (or a test stub) implements, and the per-frame context it receives.
//! - [`input`]: controller state and the input script format used for playback.
//! - [`rng`]: the deterministic random number generator handed to apps.
//! - [`runner`]: "run N frames, capture frame K", headless and deterministic.
//! - [`image`], [`png`], [`compare`]: RGBA images, a dependency-free PNG encoder/decoder, and image comparison.
//! - [`perf`], [`json`]: the stage profiler, summary statistics, reports and baseline comparison.
//! - [`golden`]: reference images kept outside the repository, and environment-driven test skipping.
//! - [`apps`]: the registry of named apps, with the built-in `pattern` stub.
//! - [`cli`]: the `harness` command-line tool, callable from other binaries with their own app registry.

#![forbid(unsafe_code)]

pub mod app;
pub mod apps;
pub mod cli;
pub mod compare;
pub mod golden;
pub mod hash;
pub mod image;
pub mod input;
pub mod json;
pub mod perf;
pub mod png;
pub mod rng;
pub mod runner;

pub use app::{App, AppConfig, AppError, FIXED_DT, FRAME_RATE, FrameContext};
pub use compare::{CompareReport, Tolerance, compare_images};
pub use image::Image;
pub use input::{Button, InputScript, InputState};
pub use perf::{FrameProbe, PerfReport};
pub use rng::Rng;
pub use runner::{RunConfig, RunResult, run};
