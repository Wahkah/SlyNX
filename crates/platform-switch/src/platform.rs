//! [`SwitchPlatform`]: the Switch implementation of [`Platform`].

use crate::audio::{self, AudioThread};
use crate::display::{DisplayHandles, GfxError};
use crate::fs::SwitchFs;
use crate::input_map::{PadReading, to_raw_all};
use crate::sys;
use crate::thread::{self, SwitchThread};
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, Ordering};
use platform_api::{
    AudioError, AudioRender, ControllerKind, Core, Deadzone, DisplayInfo, Event, InputState, MAX_PLAYERS, Platform,
    ThreadError, ThreadSpec,
};

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

/// The CPU core the calling thread is running on.
pub fn current_core() -> u32 {
    // SAFETY: no preconditions.
    unsafe { sys::slynx_current_core() as u32 }
}

/// Options for [`SwitchPlatform::new`].
#[derive(Clone, Copy, Debug)]
pub struct SwitchConfig {
    /// `DkDeviceFlags` for the deko3d device. Default: depth 0..1, upper-left origin.
    pub device_flags: u32,
    /// Swapchain images, 2 or 3. Default 3.
    pub swapchain_images: u32,
}

impl SwitchConfig {
    /// `DkDeviceFlags_DepthZeroToOne | DkDeviceFlags_OriginUpperLeft`, both of which are 0 (the deko3d default).
    pub const DEFAULT_DEVICE_FLAGS: u32 = 0;
}

impl Default for SwitchConfig {
    fn default() -> Self {
        SwitchConfig { device_flags: Self::DEFAULT_DEVICE_FLAGS, swapchain_images: 3 }
    }
}

const FOCUS_IN: u32 = 1;
const FOCUS_BACKGROUND: u32 = 3;

static TAKEN: AtomicBool = AtomicBool::new(false);

/// The Switch platform. Only one may exist; create it first thing in `main` and keep it on the main thread.
pub struct SwitchPlatform {
    start_ns: u64,
    input: InputState,
    deadzone: Deadzone,
    events: VecDeque<Event>,
    focus: u32,
    docked: bool,
    quit: bool,
    audio: Option<AudioThread>,
    _not_send: PhantomData<*mut ()>,
}

impl SwitchPlatform {
    /// Initialises the platform: mounts RomFS (if present), configures input, pins the calling thread to core 0
    /// and creates the display. Fails with "already initialised" if a platform already exists.
    pub fn new(config: SwitchConfig) -> Result<SwitchPlatform, GfxError> {
        if TAKEN.swap(true, Ordering::AcqRel) {
            return Err(GfxError(1));
        }
        // SAFETY: called once (guarded above).
        unsafe { sys::slynx_platform_init() };
        thread::pin_current(Core::Game);

        let mut st = sys::AppletState::default();
        // SAFETY: `st` is a live local.
        unsafe { sys::slynx_applet_pump(&mut st) };
        // SAFETY: the display is not initialised yet (guarded by TAKEN).
        let rc = unsafe { sys::slynx_display_init(config.device_flags, config.swapchain_images, st.docked) };
        if rc != sys::SLYNX_OK {
            // SAFETY: undoes slynx_platform_init.
            unsafe { sys::slynx_platform_exit() };
            TAKEN.store(false, Ordering::Release);
            return Err(GfxError(rc));
        }

        Ok(SwitchPlatform {
            // SAFETY: no preconditions.
            start_ns: unsafe { sys::slynx_ticks_ns() },
            input: InputState::default(),
            deadzone: Deadzone::default(),
            events: VecDeque::new(),
            focus: if st.focus == 0 { FOCUS_IN } else { st.focus },
            docked: st.docked,
            quit: false,
            audio: None,
            _not_send: PhantomData,
        })
    }

    /// The deko3d device, queue and swapchain for the renderer.
    pub fn display_handles(&self) -> DisplayHandles {
        DisplayHandles::query().expect("display exists while the platform does")
    }

    /// Acquires the next swapchain image and returns its index. Commands submitted to the display queue after this
    /// wait until the image is free.
    pub fn acquire_image(&mut self) -> u32 {
        // SAFETY: the display exists while `self` does.
        unsafe { sys::slynx_display_acquire() as u32 }
    }

    /// Presents image `index` once the commands already submitted to the display queue finish.
    pub fn present(&mut self, index: u32) {
        // SAFETY: the shim ignores out-of-range indices.
        unsafe { sys::slynx_display_present(index as i32) }
    }

    /// Waits until the GPU has finished everything submitted to the display queue.
    pub fn gpu_wait_idle(&self) {
        // SAFETY: no preconditions.
        unsafe { sys::slynx_display_wait_idle() }
    }

    /// Registers `hook` to run when the platform is dropped, before the deko3d device is destroyed (newest hook
    /// first). Every GPU user that owns device objects registers its exit function here once it is initialised and
    /// removes it with [`SwitchPlatform::remove_display_exit_hook`] when it shuts down by itself. Adding the same
    /// hook twice is a no-op; at most 8 hooks fit.
    ///
    /// # Safety
    ///
    /// `hook` must be safe to call on the main thread at any time before the device is destroyed, and idempotent
    /// (the user's own Drop may call the same function again later, after the device is gone, and must then do
    /// nothing).
    pub unsafe fn add_display_exit_hook(&self, hook: unsafe extern "C" fn()) -> Result<(), GfxError> {
        // SAFETY: the shim only stores the pointer; the caller vouches for calling it later.
        let rc = unsafe { sys::slynx_display_add_exit_hook(hook) };
        if rc == sys::SLYNX_OK { Ok(()) } else { Err(GfxError(rc)) }
    }

    /// Removes a hook added with [`SwitchPlatform::add_display_exit_hook`]. Removing one that is not registered
    /// does nothing.
    pub fn remove_display_exit_hook(&self, hook: unsafe extern "C" fn()) {
        // SAFETY: the shim compares and drops the pointer; it is never called here.
        unsafe { sys::slynx_display_remove_exit_hook(hook) }
    }

    fn update_focus(&mut self, focus: u32, resumed: bool) {
        let prev = self.focus;
        if focus != 0 && focus != prev {
            if prev == FOCUS_IN {
                self.events.push_back(Event::FocusLost);
            }
            if focus == FOCUS_BACKGROUND {
                self.events.push_back(Event::Suspended);
            }
            if prev == FOCUS_BACKGROUND {
                self.events.push_back(Event::Resumed);
            }
            if focus == FOCUS_IN {
                self.events.push_back(Event::FocusGained);
            }
            self.focus = focus;
        } else if resumed {
            self.events.push_back(Event::Resumed);
        }
    }
}

impl Platform for SwitchPlatform {
    type Fs = SwitchFs;
    type Thread = SwitchThread;

    fn name(&self) -> &'static str {
        "switch"
    }

    fn pump(&mut self) -> bool {
        if self.quit {
            return false;
        }
        let mut st = sys::AppletState::default();
        // SAFETY: `st` is a live local.
        let running = unsafe { sys::slynx_applet_pump(&mut st) };

        self.update_focus(st.focus, st.resumed);
        if st.docked != self.docked {
            self.docked = st.docked;
            // SAFETY: the display exists while `self` does.
            unsafe { sys::slynx_display_set_docked(st.docked) };
            self.events.push_back(Event::DisplayChanged(self.display()));
        }

        let mut readings = [PadReading::default(); sys::MAX_PLAYERS];
        // SAFETY: `readings` has room for the shim's four players.
        unsafe { sys::slynx_input_read(readings.as_mut_ptr()) };
        let before: [ControllerKind; MAX_PLAYERS] = core::array::from_fn(|i| self.input.players[i].kind);
        self.input.update(&to_raw_all(&readings), self.deadzone);
        for (i, (was, pad)) in before.iter().zip(self.input.players.iter()).enumerate() {
            match (*was == ControllerKind::None, pad.connected()) {
                (true, true) => self.events.push_back(Event::ControllerConnected(i as u8)),
                (false, false) => self.events.push_back(Event::ControllerDisconnected(i as u8)),
                _ => {}
            }
        }

        if !running {
            self.quit = true;
            self.events.push_back(Event::Quit);
        }
        running
    }

    fn poll_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    fn input(&self) -> &InputState {
        &self.input
    }

    fn set_deadzone(&mut self, deadzone: Deadzone) {
        self.deadzone = deadzone;
    }

    fn display(&self) -> DisplayInfo {
        let (width, height) = if self.docked { (1920, 1080) } else { (1280, 720) };
        DisplayInfo { width, height, docked: self.docked }
    }

    fn now_ns(&self) -> u64 {
        // SAFETY: no preconditions.
        unsafe { sys::slynx_ticks_ns() }.saturating_sub(self.start_ns)
    }

    fn set_cpu_boost(&mut self, on: bool) -> bool {
        // SAFETY: no preconditions.
        unsafe { sys::slynx_set_cpu_boost(on) == 0 }
    }

    fn fs(&self) -> SwitchFs {
        SwitchFs
    }

    fn start_audio(&mut self, render: Box<dyn AudioRender>) -> Result<(), AudioError> {
        self.stop_audio();
        self.audio = Some(AudioThread::start(render)?);
        Ok(())
    }

    fn stop_audio(&mut self) {
        self.audio = None;
    }

    fn audio_latency_ns(&self) -> u64 {
        if self.audio.is_some() { audio::LATENCY_NS } else { 0 }
    }

    fn spawn(&self, spec: ThreadSpec, f: Box<dyn FnOnce() + Send + 'static>) -> Result<SwitchThread, ThreadError> {
        thread::spawn(spec, f)
    }

    fn pin_current_thread(&self, core: Core) -> bool {
        thread::pin_current(core)
    }

    fn current_core() -> Option<u32> {
        Some(current_core())
    }
}

impl Drop for SwitchPlatform {
    fn drop(&mut self) {
        self.stop_audio();
        // SAFETY: tear down in reverse order of creation; each call is idempotent. `slynx_display_exit` first runs
        // the exit hooks of every GPU user still alive (the debug draw, the renderer), newest first, while the
        // device exists, so their own later Drop finds nothing to free.
        unsafe {
            sys::slynx_display_exit();
            sys::slynx_platform_exit();
        }
        TAKEN.store(false, Ordering::Release);
    }
}
