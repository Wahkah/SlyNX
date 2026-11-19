//! Thread placement. The Switch gives a homebrew program three CPU cores (0, 1, 2); SlyNX puts the game on core 0,
//! file I/O on core 1 and audio on core 2.

use core::fmt;

/// The role a thread plays, which decides its core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Core {
    /// Core 0: the game loop and rendering.
    Game,
    /// Core 1: file loading and decompression.
    Io,
    /// Core 2: the audio mixer.
    Audio,
}

impl Core {
    /// The CPU core number.
    pub const fn index(self) -> u32 {
        match self {
            Core::Game => 0,
            Core::Io => 1,
            Core::Audio => 2,
        }
    }
}

/// Scheduling priority relative to the game thread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Priority {
    /// Above the game thread (audio).
    High,
    #[default]
    Normal,
    /// Below the game thread (background loading).
    Low,
}

/// How to start a thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreadSpec {
    /// Shown in debuggers (PC); ignored on the Switch.
    pub name: &'static str,
    pub core: Core,
    pub priority: Priority,
    /// Stack size in bytes, rounded up to the page size by the backend.
    pub stack_size: usize,
}

impl ThreadSpec {
    pub const DEFAULT_STACK: usize = 256 * 1024;

    pub const fn new(name: &'static str, core: Core) -> ThreadSpec {
        ThreadSpec { name, core, priority: Priority::Normal, stack_size: Self::DEFAULT_STACK }
    }

    pub const fn priority(mut self, priority: Priority) -> ThreadSpec {
        self.priority = priority;
        self
    }

    pub const fn stack_size(mut self, bytes: usize) -> ThreadSpec {
        self.stack_size = bytes;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadError {
    /// The system refused to create the thread. The code is the backend's.
    Create(u32),
}

impl fmt::Display for ThreadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThreadError::Create(c) => write!(f, "could not create thread ({c:#x})"),
        }
    }
}

/// A started thread. Dropping the handle waits for the thread to finish (the Switch cannot detach threads).
pub trait JoinHandle {
    /// Waits for the thread to finish.
    fn join(self);
}
