//! The platform interface SlyNX runs on.
//!
//! Two backends implement [`Platform`]: `platform-switch` (libnx + deko3d on the Nintendo Switch) and `platform-pc`
//! (SDL2 + OpenGL on Linux and Windows, with a headless mode for tests). The engine, the harness and the demo are
//! written against this trait; only the graphics hand-off differs per backend, because the renderer has one backend
//! per graphics API anyway (see `docs/platform.md`).
//!
//! This crate is `no_std` + `alloc` and has no FFI, so everything in it is plain data and logic that is tested on
//! any host: button edges, stick deadzones, action mapping, sample conversion, path validation.
#![no_std]

extern crate alloc;
#[cfg(test)]
extern crate std;

pub mod audio;
pub mod draw;
pub mod event;
pub mod fs;
pub mod input;
pub mod thread;

pub use audio::{AudioError, AudioRender, CHANNELS, SAMPLE_RATE};
pub use draw::{DebugDraw, Vertex};
pub use event::{DisplayInfo, Event};
pub use fs::{DataFile, FileSystem, FsError, Root};
pub use input::{
    ActionMap, Actions, Binding, Buttons, ControllerKind, Deadzone, Dir, InputState, MAX_PLAYERS, PadState, RawPad,
    Stick, StickId,
};
pub use thread::{Core, JoinHandle, Priority, ThreadError, ThreadSpec};

use alloc::boxed::Box;

/// One platform backend. Create it once at startup (both backends refuse a second instance) and keep it on the game
/// thread: it is not `Send`.
///
/// A frame looks like this:
///
/// ```ignore
/// while platform.pump() {
///     while let Some(event) = platform.poll_event() { /* focus, display mode, controllers */ }
///     let input = platform.input();
///     // update, render, present (renderer-specific)
/// }
/// ```
pub trait Platform {
    /// File access for any thread.
    type Fs: FileSystem + Clone + Send + Sync + 'static;
    /// A thread started with [`Platform::spawn`].
    type Thread: JoinHandle;

    /// Short backend name for logs ("switch", "pc", "pc-headless").
    fn name(&self) -> &'static str;

    /// Processes system messages and reads the controllers. Call once at the top of every frame. Returns `false` when
    /// the system or the user asked the program to exit; the caller should then leave its main loop.
    fn pump(&mut self) -> bool;

    /// The next event queued by [`Platform::pump`], oldest first.
    fn poll_event(&mut self) -> Option<Event>;

    /// Controller state as of the last [`Platform::pump`].
    fn input(&self) -> &InputState;

    /// Changes the stick deadzone used from the next [`Platform::pump`] on.
    fn set_deadzone(&mut self, deadzone: Deadzone);

    /// The current output: render size and whether the console is docked.
    fn display(&self) -> DisplayInfo;

    /// Monotonic time in nanoseconds since the platform was created. Never goes backwards.
    fn now_ns(&self) -> u64;

    /// Requests CPU boost (Switch: `FastLoad`, CPU at its highest clock while the GPU is throttled) for loading
    /// screens. Returns `true` if the request was applied; the PC accepts and ignores it.
    fn set_cpu_boost(&mut self, on: bool) -> bool;

    /// File access. The handle is cheap to clone and can be moved to other threads (the loader on [`Core::Io`]).
    fn fs(&self) -> Self::Fs;

    /// Starts audio output: `render` is called on the audio thread (Switch: core 2) to fill each buffer of
    /// [`SAMPLE_RATE`] Hz, [`CHANNELS`]-channel interleaved `f32` samples. Replaces a previous renderer.
    fn start_audio(&mut self, render: Box<dyn AudioRender>) -> Result<(), AudioError>;

    /// Stops audio output and drops the renderer. Does nothing if audio is not running.
    fn stop_audio(&mut self);

    /// Output latency in nanoseconds: how long a sample written by the renderer takes to reach the speaker, at most.
    /// Zero when audio is not running.
    fn audio_latency_ns(&self) -> u64;

    /// Starts a thread. On the Switch it runs on `spec.core` only; on the PC pinning is optional (see
    /// `PcConfig::pin_threads`).
    fn spawn(&self, spec: ThreadSpec, f: Box<dyn FnOnce() + Send + 'static>) -> Result<Self::Thread, ThreadError>;

    /// Pins the calling thread to `core`. The game thread should call this once with [`Core::Game`] (the Switch
    /// backend already does so for the thread that creates it).
    fn pin_current_thread(&self, core: Core) -> bool;

    /// The CPU core the calling thread is running on, if the system can tell. Callable from any thread.
    fn current_core() -> Option<u32>
    where
        Self: Sized;
}
