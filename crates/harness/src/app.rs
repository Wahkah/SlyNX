//! The interface between the harness and the program it drives.
//!
//! The game on PC (and every test stub) implements [`App`]. The harness owns the clock, the input and the random
//! number generator, and calls the app once per frame:
//!
//! 1. [`App::update`] with the frame's input, a fixed `dt` of 1/60 s and the seeded [`Rng`];
//! 2. [`App::render`], which draws the frame (offscreen when headless);
//! 3. [`App::capture`] on the frames the run asked for, which returns the rendered frame as an [`Image`];
//! 4. [`App::state_hash`] after every update, if the app provides one, to check determinism.
//!
//! # Determinism contract
//!
//! For a run to be reproducible the app must:
//!
//! - take time only from the context ([`FrameContext::frame`], [`FrameContext::dt`]), never from the wall clock;
//! - take randomness only from [`FrameContext::rng`] (or generators seeded from it, or from [`AppConfig::seed`]);
//! - take input only from [`FrameContext::input`];
//! - not depend on thread scheduling, hash-map iteration order or uninitialised memory for anything that affects
//!   the simulation or the image.
//!
//! Timing measurements ([`crate::perf::FrameProbe`]) read a real clock, but they are outputs only and must never
//! feed back into the simulation.

use std::fmt;

use crate::image::Image;
use crate::input::InputState;
use crate::perf::FrameProbe;
use crate::rng::Rng;

/// Simulation frames per second.
pub const FRAME_RATE: u32 = 60;

/// The fixed timestep, in seconds.
pub const FIXED_DT: f64 = 1.0 / FRAME_RATE as f64;

/// How an app is created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppConfig {
    /// Width of the render target in pixels.
    pub width: u32,
    /// Height of the render target in pixels.
    pub height: u32,
    /// The run's seed. The harness seeds [`FrameContext::rng`] with it too.
    pub seed: u64,
    /// No window: render offscreen only.
    pub headless: bool,
    /// App-specific `key=value` options from the command line (`--opt key=value`), in order.
    pub options: Vec<(String, String)>,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig { width: 1280, height: 720, seed: 0, headless: true, options: Vec::new() }
    }
}

impl AppConfig {
    /// The value of the last option named `key`.
    pub fn option(&self, key: &str) -> Option<&str> {
        self.options.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

/// An error reported by an app. The run stops at the first one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppError(pub String);

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AppError {}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError(s.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError(e.to_string())
    }
}

/// What the app gets each frame.
pub struct FrameContext<'a> {
    /// The frame number, from 0.
    pub frame: u64,
    /// The fixed timestep in seconds (always [`FIXED_DT`]).
    pub dt: f64,
    /// The controller state for this frame.
    pub input: &'a InputState,
    /// The run's random number generator.
    pub rng: &'a mut Rng,
    /// Stage timers and counters for the performance report.
    pub probe: &'a mut FrameProbe,
    /// Whether this frame will be captured after rendering.
    pub capture_requested: bool,
}

impl FrameContext<'_> {
    /// Simulated time at the start of this frame, in seconds.
    pub fn time(&self) -> f64 {
        self.frame as f64 * self.dt
    }
}

/// A program the harness can drive. See the [module documentation](self) for the call order and the determinism
/// contract.
pub trait App {
    /// Advances the simulation by one fixed step.
    fn update(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError>;

    /// Draws the current state. Report renderer counters with
    /// [`FrameProbe::count`](crate::perf::FrameProbe::count), for example
    /// [`DRAW_CALLS`](crate::perf::DRAW_CALLS) and [`TRIANGLES`](crate::perf::TRIANGLES).
    fn render(&mut self, ctx: &mut FrameContext<'_>) -> Result<(), AppError>;

    /// Returns the last rendered frame, top row first, at the configured size. Must wait for the GPU to finish.
    fn capture(&mut self) -> Result<Image, AppError>;

    /// A hash of the simulation state after the last update, for determinism checks. Use
    /// [`Fnv64`](crate::hash::Fnv64), not `std`'s hasher, so hashes are stable across Rust versions.
    fn state_hash(&self) -> Option<u64> {
        None
    }

    /// Called once after the last frame.
    fn shutdown(&mut self) {}
}
