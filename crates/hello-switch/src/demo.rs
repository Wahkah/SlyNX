//! The platform demo, written once against [`Platform`] and run by both backends.
//!
//! It shows every player's controller live, plays a tone when a face button goes down (proving the audio thread),
//! reads a file from RomFS on a thread pinned to the I/O core, counts launches in a save file in the user folder,
//! toggles CPU boost on ZR, and reports applet events and the frame rate.

use crate::scene::{Scene, View};
use crate::tone::{ToneControl, ToneSynth};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};
use platform_api::{AudioError, Buttons, Core, Event, FileSystem, JoinHandle, Platform, Root, ThreadSpec, Vertex};

const RUNS_FILE: &str = "hello/runs.txt";
const ROMFS_FILE: &str = "hello.txt";
const NO_CORE: u32 = u32::MAX;

/// Face button to note: A4, C5, E5, G5.
const NOTES: [(Buttons, f32); 4] =
    [(Buttons::A, 440.0), (Buttons::B, 523.25), (Buttons::X, 659.25), (Buttons::Y, 783.99)];

pub struct Demo {
    tone: Arc<ToneControl>,
    audio: String,
    files: String,
    save: String,
    event: &'static str,
    boost: Option<bool>,
    start_ns: u64,
    last_ns: u64,
    fps: f32,
    status: Vec<String>,
}

/// Keeps only characters the font can draw, upper-cased, at most `max` of them.
fn printable(s: &str, max: usize) -> String {
    s.chars().map(|c| c.to_ascii_uppercase()).filter(|c| crate::font::glyph(*c).is_some()).take(max).collect()
}

fn core_text(c: u32) -> String {
    if c == NO_CORE { String::from("?") } else { format!("{c}") }
}

impl Demo {
    pub fn new<P: Platform>(platform: &mut P) -> Demo {
        let tone = Arc::new(ToneControl::default());
        let audio = match platform.start_audio(Box::new(ToneSynth::new(tone.clone()))) {
            Ok(()) => format!("AUDIO {}MS", platform.audio_latency_ns() / 1_000_000),
            Err(AudioError::Disabled) => String::from("AUDIO OFF"),
            Err(AudioError::Thread) => String::from("AUDIO THREAD FAILED"),
            Err(AudioError::Device(c)) => format!("AUDIO DEVICE ERROR {}", c as u32),
        };

        // Read the RomFS file on a thread on the I/O core, and note which core it actually ran on.
        let fs = platform.fs();
        let io_core = Arc::new(AtomicU32::new(NO_CORE));
        let io_core_t = io_core.clone();
        let text = Arc::new(spin_cell::Cell::default());
        let text_t = text.clone();
        let job = Box::new(move || {
            io_core_t.store(P::current_core().unwrap_or(NO_CORE), Ordering::Relaxed);
            let line = match fs.read(Root::RomFs, ROMFS_FILE) {
                Ok(bytes) => {
                    let s = core::str::from_utf8(&bytes).unwrap_or("NOT UTF-8");
                    printable(s.lines().next().unwrap_or(""), 24)
                }
                Err(e) => printable(&format!("{e}"), 24),
            };
            text_t.set(line);
        });
        let files = match platform.spawn(ThreadSpec::new("io", Core::Io), job) {
            Ok(t) => {
                t.join();
                format!(
                    "ROMFS {}  IO CORE {}  GAME CORE {}",
                    text.take(),
                    core_text(io_core.load(Ordering::Relaxed)),
                    core_text(P::current_core().unwrap_or(NO_CORE))
                )
            }
            Err(e) => printable(&format!("IO THREAD FAILED {e}"), 40),
        };

        // Count launches in the user folder.
        let fs = platform.fs();
        let runs = fs
            .read(Root::User, RUNS_FILE)
            .ok()
            .and_then(|b| core::str::from_utf8(&b).ok().and_then(|s| s.trim().parse::<u32>().ok()))
            .unwrap_or(0)
            .saturating_add(1);
        let save = match fs.write_user(RUNS_FILE, format!("{runs}\n").as_bytes()) {
            Ok(()) => format!("SAVE RUN {runs}"),
            Err(e) => printable(&format!("SAVE FAILED {e}"), 30),
        };

        let now = platform.now_ns();
        Demo {
            tone,
            audio,
            files,
            save,
            event: "NONE",
            boost: Some(false),
            start_ns: now,
            last_ns: now,
            fps: 0.0,
            status: Vec::new(),
        }
    }

    /// The status lines of the last frame.
    pub fn status(&self) -> &[String] {
        &self.status
    }

    /// Handles this frame's events and input and appends the frame to `out`. Returns the clear colour, or `None`
    /// when the demo should exit. Call after `platform.pump()`.
    pub fn frame<P: Platform>(&mut self, platform: &mut P, out: &mut Vec<Vertex>) -> Option<[f32; 4]> {
        while let Some(event) = platform.poll_event() {
            self.event = match event {
                Event::Quit => return None,
                Event::FocusLost => "FOCUS LOST",
                Event::FocusGained => "FOCUS GAINED",
                Event::Suspended => "SUSPENDED",
                Event::Resumed => "RESUMED",
                Event::DisplayChanged(_) => "DISPLAY CHANGED",
                Event::ControllerConnected(_) => "CONTROLLER CONNECTED",
                Event::ControllerDisconnected(_) => "CONTROLLER DISCONNECTED",
            };
        }

        let input = *platform.input();
        for pad in &input.players {
            if pad.pressed.any(Buttons::PLUS) {
                return None;
            }
            for (button, freq) in NOTES {
                if pad.pressed.any(button) {
                    self.tone.trigger(freq);
                }
            }
            if pad.pressed.any(Buttons::ZR) {
                let want = !self.boost.unwrap_or(false);
                self.boost = if platform.set_cpu_boost(want) { Some(want) } else { None };
            }
        }

        let now = platform.now_ns();
        let dt = now.saturating_sub(self.last_ns) as f32 * 1e-9;
        self.last_ns = now;
        if dt > 0.0 {
            let inst = 1.0 / dt;
            self.fps = if self.fps == 0.0 { inst } else { self.fps * 0.95 + inst * 0.05 };
        }

        let boost = match self.boost {
            Some(true) => "ON",
            Some(false) => "OFF",
            None => "FAILED",
        };
        self.status.clear();
        self.status.push(format!(
            "{}  {} {}K  BOOST {}  {} FPS",
            printable(platform.name(), 12),
            self.audio,
            self.tone.rendered_frames() / 1000,
            boost,
            self.fps as u32
        ));
        self.status.push(self.files.clone());
        self.status.push(format!("{}  EVENT {}", self.save, self.event));

        let view = View {
            t: now.saturating_sub(self.start_ns) as f32 * 1e-9,
            display: platform.display(),
            input: &input,
            status: &self.status,
        };
        Some(Scene::build(&view, out))
    }
}

/// A tiny single-slot hand-off for the I/O thread's result. `no_std` has no `Mutex`; the thread writes once and the
/// game thread reads after joining it, so a flag is enough.
mod spin_cell {
    use alloc::string::String;
    use core::cell::UnsafeCell;
    use core::sync::atomic::{AtomicBool, Ordering};

    #[derive(Default)]
    pub struct Cell {
        full: AtomicBool,
        value: UnsafeCell<String>,
    }

    // SAFETY: `set` writes only while `full` is false and publishes with Release; `take` reads only after seeing
    // `full` with Acquire. The demo calls `set` once from one thread and `take` after joining it.
    unsafe impl Sync for Cell {}

    impl Cell {
        pub fn set(&self, v: String) {
            if !self.full.load(Ordering::Acquire) {
                // SAFETY: no reader touches `value` until `full` is set below.
                unsafe { *self.value.get() = v };
                self.full.store(true, Ordering::Release);
            }
        }

        pub fn take(&self) -> String {
            if self.full.swap(false, Ordering::AcqRel) {
                // SAFETY: the writer finished (Release) before `full` became true.
                unsafe { core::mem::take(&mut *self.value.get()) }
            } else {
                String::new()
            }
        }
    }
}
