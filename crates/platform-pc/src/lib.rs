//! PC backend of the SlyNX platform layer (Linux and Windows), for development, the test harness and image
//! comparison.
//!
//! [`PcPlatform`] implements [`platform_api::Platform`] with SDL2: a window with an OpenGL 4.3 core context, keyboard
//! and gamepads (up to four players), audio output, files from three folders standing in for the Switch's RomFS, SD
//! card and user folder, threads and a monotonic (or, for deterministic runs, fixed-step) clock.
//!
//! With [`PcConfig::headless`] there is no visible window: rendering goes to an offscreen framebuffer that
//! [`PcPlatform::read_pixels`] reads back. On Linux the context comes from EGL's surfaceless platform, which needs no
//! display server at all; elsewhere (or if EGL is missing) a hidden SDL window provides it.
//!
//! SDL2 and EGL are declared by hand in `ffi.rs` and linked from the system (`libSDL2`, `libEGL`); the crate has no
//! dependencies outside the workspace.

mod audio;
mod context;
mod draw;
mod ffi;
mod fs;
pub mod gl;
mod input;
mod thread;

pub use draw::PcDebugDraw;
pub use fs::{PcFile, PcFs};
pub use thread::PcThread;

use audio::PcAudio;
use context::{GlShared, Sdl};
use platform_api::{
    AudioError, AudioRender, ControllerKind, Core, Deadzone, DisplayInfo, Event, InputState, MAX_PLAYERS, Platform,
    RawPad, Stick, ThreadError, ThreadSpec,
};
use std::collections::VecDeque;
use std::ffi::{CStr, c_char, c_void};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Options for [`PcPlatform::new`].
#[derive(Clone, Debug)]
pub struct PcConfig {
    pub title: String,
    /// Window or offscreen size. 1280x720 matches handheld, 1920x1080 docked.
    pub width: u32,
    pub height: u32,
    /// No visible window; render offscreen.
    pub headless: bool,
    pub vsync: bool,
    /// Open an audio device for `start_audio`. Off for headless runs by default.
    pub audio: bool,
    /// Read the keyboard and gamepads. Off for headless runs (use [`PcPlatform::set_scripted_input`]).
    pub controllers: bool,
    /// Folder standing in for RomFS.
    pub romfs_dir: PathBuf,
    /// Folder standing in for the SD card.
    pub sd_dir: PathBuf,
    /// Writable user folder.
    pub user_dir: PathBuf,
    /// Pin threads to cores 0/1/2 like on the Switch. Off by default: on a development PC it rarely helps.
    pub pin_threads: bool,
    /// If set, `now_ns` advances by exactly this much per `pump` instead of following the wall clock, so runs are
    /// deterministic.
    pub fixed_step_ns: Option<u64>,
}

fn default_user_dir() -> PathBuf {
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("SlyNX");
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg).join("slynx");
    }
    if let Some(home) = std::env::var_os("HOME") {
        // The XDG default data folder, $HOME + .local + share.
        return PathBuf::from(home).join(".local").join("share").join("slynx");
    }
    PathBuf::from("user")
}

impl Default for PcConfig {
    fn default() -> Self {
        PcConfig {
            title: "SlyNX".into(),
            width: 1280,
            height: 720,
            headless: false,
            vsync: true,
            audio: true,
            controllers: true,
            romfs_dir: PathBuf::from("romfs"),
            sd_dir: PathBuf::from("sdmc"),
            user_dir: default_user_dir(),
            pin_threads: false,
            fixed_step_ns: None,
        }
    }
}

impl PcConfig {
    /// A headless configuration for tests: offscreen `width` x `height`, no audio, no controllers, a fixed 60 Hz
    /// clock.
    pub fn headless(width: u32, height: u32) -> PcConfig {
        PcConfig {
            width,
            height,
            headless: true,
            vsync: false,
            audio: false,
            controllers: false,
            fixed_step_ns: Some(16_666_667),
            ..PcConfig::default()
        }
    }

    /// Applies environment overrides: `SLYNX_HEADLESS=1`, `SLYNX_DOCKED=1` (1920x1080), `SLYNX_ROMFS_DIR`,
    /// `SLYNX_SD_DIR`, `SLYNX_USER_DIR`, `SLYNX_PIN_THREADS=1`.
    pub fn with_env(mut self) -> PcConfig {
        let flag = |name: &str| std::env::var(name).is_ok_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        if flag("SLYNX_HEADLESS") {
            let (w, h) = (self.width, self.height);
            self = PcConfig { romfs_dir: self.romfs_dir, sd_dir: self.sd_dir, ..PcConfig::headless(w, h) };
        }
        if flag("SLYNX_DOCKED") {
            (self.width, self.height) = (1920, 1080);
        }
        if flag("SLYNX_PIN_THREADS") {
            self.pin_threads = true;
        }
        for (name, dir) in [
            ("SLYNX_ROMFS_DIR", &mut self.romfs_dir),
            ("SLYNX_SD_DIR", &mut self.sd_dir),
            ("SLYNX_USER_DIR", &mut self.user_dir),
        ] {
            if let Some(v) = std::env::var_os(name).filter(|v| !v.is_empty()) {
                *dir = PathBuf::from(v);
            }
        }
        self
    }
}

/// The GL context, for the renderer. Valid while the platform (or a debug draw) is alive, on the thread that
/// created the platform only.
#[derive(Clone)]
pub struct GlContext(Rc<GlShared>);

impl GlContext {
    /// Looks up a GL function by name (`c"glDrawElements"`). NULL if the driver does not have it.
    pub fn get_proc_address(&self, name: &CStr) -> *const c_void {
        self.0.get_proc_address(name)
    }

    /// The framebuffer to render the final image into: 0 for a window, the offscreen framebuffer when headless.
    /// Bind it instead of 0.
    pub fn default_framebuffer(&self) -> u32 {
        self.0.default_framebuffer()
    }

    /// Current drawable size in pixels.
    pub fn size(&self) -> (u32, u32) {
        self.0.size()
    }

    pub fn is_offscreen(&self) -> bool {
        self.0.is_offscreen()
    }

    /// `GL_VERSION` and `GL_RENDERER`.
    pub fn describe(&self) -> (String, String) {
        (self.0.gl.string(gl::VERSION), self.0.gl.string(gl::RENDERER))
    }
}

/// A C-callable loader for code that wants a plain function pointer (`gladLoadGL`-style). Only valid while a
/// platform exists, on its thread; returns NULL otherwise.
///
/// # Safety
///
/// `name` must be NULL or point to a NUL-terminated string.
pub unsafe extern "C" fn get_proc_address_c(name: *const c_char) -> *const c_void {
    if name.is_null() {
        return std::ptr::null();
    }
    CURRENT_GL.with(|c| match c.borrow().as_ref() {
        // SAFETY: the caller guarantees a NUL-terminated name.
        Some(gl) => gl.get_proc_address(unsafe { CStr::from_ptr(name) }),
        None => std::ptr::null(),
    })
}

thread_local! {
    static CURRENT_GL: std::cell::RefCell<Option<Rc<GlShared>>> = const { std::cell::RefCell::new(None) };
}

static TAKEN: AtomicBool = AtomicBool::new(false);

struct Controller {
    handle: *mut c_void,
    instance: i32,
    kind: ControllerKind,
}

/// The PC platform. Only one may exist at a time (SDL is process-wide); keep it on the thread that created it.
pub struct PcPlatform {
    config: PcConfig,
    // Field order is drop order: audio and controllers before the GL context, SDL last.
    audio: Option<PcAudio>,
    controllers: [Option<Controller>; MAX_PLAYERS],
    gl: Rc<GlShared>,
    sdl: Rc<Sdl>,
    fs: PcFs,
    input: InputState,
    deadzone: Deadzone,
    scripted: Option<[RawPad; MAX_PLAYERS]>,
    events: VecDeque<Event>,
    quit: bool,
    start: Instant,
    frames: u64,
}

impl PcPlatform {
    pub fn new(config: PcConfig) -> Result<PcPlatform, String> {
        if TAKEN.swap(true, Ordering::AcqRel) {
            return Err("a PcPlatform already exists".into());
        }
        match Self::create(config) {
            Ok(p) => Ok(p),
            Err(e) => {
                TAKEN.store(false, Ordering::Release);
                Err(e)
            }
        }
    }

    fn create(config: PcConfig) -> Result<PcPlatform, String> {
        let sdl = Sdl::init()?;
        if config.controllers {
            sdl.subsystem(ffi::SDL_INIT_GAMECONTROLLER | ffi::SDL_INIT_EVENTS)
                .map_err(|e| format!("SDL game controllers: {e}"))?;
        }
        let (w, h) = (config.width.max(1), config.height.max(1));
        let gl = if config.headless {
            Self::headless_context(&sdl, &config.title, w, h)?
        } else {
            GlShared::sdl_window(sdl.clone(), &config.title, w, h, false, config.vsync)?
        };
        let gl = Rc::new(gl);
        CURRENT_GL.with(|c| *c.borrow_mut() = Some(gl.clone()));
        let fs = PcFs::new(config.romfs_dir.clone(), config.sd_dir.clone(), config.user_dir.clone());
        let p = PcPlatform {
            audio: None,
            controllers: Default::default(),
            gl,
            sdl,
            fs,
            input: InputState::default(),
            deadzone: Deadzone::default(),
            scripted: None,
            events: VecDeque::new(),
            quit: false,
            start: Instant::now(),
            frames: 0,
            config,
        };
        if p.config.pin_threads {
            thread::pin_current(Core::Game);
        }
        Ok(p)
    }

    fn headless_context(sdl: &Rc<Sdl>, title: &str, w: u32, h: u32) -> Result<GlShared, String> {
        #[cfg(target_os = "linux")]
        let egl_err = match GlShared::egl_surfaceless(sdl.clone(), w, h) {
            Ok(gl) => return Ok(gl),
            Err(e) => e,
        };
        #[cfg(not(target_os = "linux"))]
        let egl_err = String::from("EGL not used on this OS");
        GlShared::sdl_window(sdl.clone(), title, w, h, true, false)
            .map_err(|e| format!("no headless GL context: EGL: {egl_err}; hidden SDL window: {e}"))
    }

    /// The GL context for the renderer.
    pub fn gl(&self) -> GlContext {
        GlContext(self.gl.clone())
    }

    /// A debug draw on this platform's context.
    pub fn debug_draw(&self, max_vertices: usize) -> Result<PcDebugDraw, String> {
        PcDebugDraw::new(self.gl.clone(), max_vertices)
    }

    /// Shows the frame: swaps the window's buffers (waiting for vsync if enabled); headless, flushes.
    pub fn present(&mut self) {
        self.gl.present();
    }

    /// Reads the default framebuffer as RGBA8, `width * height * 4` bytes, rows top to bottom. Headless, this is
    /// the last frame drawn; with a window, call it before `present` (the back buffer is undefined after a swap).
    pub fn read_pixels(&self) -> Vec<u8> {
        self.gl.read_pixels()
    }

    /// Replaces the controllers with scripted input from the next `pump` on (`None` goes back to real devices).
    /// For deterministic test runs.
    pub fn set_scripted_input(&mut self, pads: Option<[RawPad; MAX_PLAYERS]>) {
        self.scripted = pads;
    }

    pub fn config(&self) -> &PcConfig {
        &self.config
    }

    fn open_controller(&mut self, device_index: i32) {
        // SAFETY: SDL's controller subsystem is initialised (we only get device events when it is).
        unsafe {
            if ffi::SDL_IsGameController(device_index) == 0 {
                return;
            }
            let Some(slot) = self.controllers.iter().position(|c| c.is_none()) else { return };
            let handle = ffi::SDL_GameControllerOpen(device_index);
            if handle.is_null() {
                return;
            }
            let instance = ffi::SDL_JoystickInstanceID(ffi::SDL_GameControllerGetJoystick(handle));
            if self.controllers.iter().flatten().any(|c| c.instance == instance) {
                ffi::SDL_GameControllerClose(handle); // already open (SDL reports startup devices once more)
                return;
            }
            let kind = input::kind(ffi::SDL_GameControllerGetType(handle));
            self.controllers[slot] = Some(Controller { handle, instance, kind });
        }
    }

    fn close_controller(&mut self, instance: i32) {
        for slot in &mut self.controllers {
            if slot.as_ref().is_some_and(|c| c.instance == instance) {
                let c = slot.take().unwrap();
                // SAFETY: the handle is open and closed once.
                unsafe { ffi::SDL_GameControllerClose(c.handle) };
            }
        }
    }

    fn handle_events(&mut self) {
        let mut e = ffi::SDL_Event::zeroed();
        // SAFETY: `e` is a 56-byte, 8-aligned buffer, the size of SDL_Event.
        while unsafe { ffi::SDL_PollEvent(&mut e) } != 0 {
            match e.kind() {
                ffi::SDL_QUIT => self.request_quit(),
                ffi::SDL_WINDOWEVENT => match e.window_event() {
                    ffi::SDL_WINDOWEVENT_FOCUS_LOST => self.events.push_back(Event::FocusLost),
                    ffi::SDL_WINDOWEVENT_FOCUS_GAINED => self.events.push_back(Event::FocusGained),
                    ffi::SDL_WINDOWEVENT_MINIMIZED => self.events.push_back(Event::Suspended),
                    ffi::SDL_WINDOWEVENT_RESTORED => self.events.push_back(Event::Resumed),
                    ffi::SDL_WINDOWEVENT_SIZE_CHANGED => {
                        if self.gl.refresh_size() {
                            self.events.push_back(Event::DisplayChanged(self.display()));
                        }
                    }
                    ffi::SDL_WINDOWEVENT_CLOSE => self.request_quit(),
                    _ => {}
                },
                ffi::SDL_KEYDOWN if !e.key_repeat() => match e.key_scancode() {
                    ffi::SCANCODE_ESCAPE => self.request_quit(),
                    ffi::SCANCODE_F11 => self.toggle_docked(),
                    _ => {}
                },
                ffi::SDL_CONTROLLERDEVICEADDED => self.open_controller(e.device_which()),
                ffi::SDL_CONTROLLERDEVICEREMOVED => self.close_controller(e.device_which()),
                _ => {}
            }
        }
    }

    fn request_quit(&mut self) {
        if !self.quit {
            self.quit = true;
            self.events.push_back(Event::Quit);
        }
    }

    /// F11: switch the window between 1280x720 ("handheld") and 1920x1080 ("docked").
    fn toggle_docked(&mut self) {
        if let Some(window) = self.gl.window() {
            let (w, h) = if self.display().docked { (1280, 720) } else { (1920, 1080) };
            // SAFETY: the window is live.
            unsafe { ffi::SDL_SetWindowSize(window, w, h) };
        }
    }

    fn read_devices(&self) -> [RawPad; MAX_PLAYERS] {
        let mut raw = [RawPad::DISCONNECTED; MAX_PLAYERS];
        for (pad, c) in raw.iter_mut().zip(&self.controllers) {
            let Some(c) = c else { continue };
            let h = c.handle;
            // SAFETY: the controller is open while it sits in its slot.
            let (buttons, left, right) = input::gamepad(
                |b| unsafe { ffi::SDL_GameControllerGetButton(h, b) } != 0,
                |a| unsafe { ffi::SDL_GameControllerGetAxis(h, a) },
            );
            *pad = RawPad { kind: c.kind, buttons, left, right };
        }
        // The keyboard is player 1: on its own, or merged into player 1's gamepad.
        if self.gl.window().is_some() {
            let mut n = 0;
            // SAFETY: SDL returns its internal key array of `n` entries, valid for the life of the program.
            let keys = unsafe {
                let p = ffi::SDL_GetKeyboardState(&mut n);
                if p.is_null() { &[][..] } else { std::slice::from_raw_parts(p, n.max(0) as usize) }
            };
            let (buttons, left, right) = input::keyboard(keys);
            let p1 = &mut raw[0];
            if p1.kind == ControllerKind::None {
                p1.kind = ControllerKind::Keyboard;
            }
            p1.buttons |= buttons;
            if left != Stick::ZERO {
                p1.left = left;
            }
            if right != Stick::ZERO {
                p1.right = right;
            }
        }
        raw
    }
}

impl Platform for PcPlatform {
    type Fs = PcFs;
    type Thread = PcThread;

    fn name(&self) -> &'static str {
        if self.gl.is_offscreen() { "pc-headless" } else { "pc" }
    }

    fn pump(&mut self) -> bool {
        if !self.quit {
            self.handle_events();
        }
        self.frames += 1;
        let raw = match self.scripted {
            Some(s) => s,
            None if self.config.controllers => self.read_devices(),
            None => [RawPad::DISCONNECTED; MAX_PLAYERS],
        };
        let before: [ControllerKind; MAX_PLAYERS] = std::array::from_fn(|i| self.input.players[i].kind);
        self.input.update(&raw, self.deadzone);
        for (i, (was, pad)) in before.iter().zip(self.input.players.iter()).enumerate() {
            match (*was == ControllerKind::None, pad.connected()) {
                (true, true) => self.events.push_back(Event::ControllerConnected(i as u8)),
                (false, false) => self.events.push_back(Event::ControllerDisconnected(i as u8)),
                _ => {}
            }
        }
        !self.quit
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
        let (width, height) = self.gl.size();
        DisplayInfo { width, height, docked: width >= 1920 && height >= 1080 }
    }

    fn now_ns(&self) -> u64 {
        match self.config.fixed_step_ns {
            Some(step) => self.frames.saturating_mul(step),
            None => self.start.elapsed().as_nanos() as u64,
        }
    }

    fn set_cpu_boost(&mut self, _on: bool) -> bool {
        true
    }

    fn fs(&self) -> PcFs {
        self.fs.clone()
    }

    fn start_audio(&mut self, render: Box<dyn AudioRender>) -> Result<(), AudioError> {
        self.stop_audio();
        if !self.config.audio {
            return Err(AudioError::Disabled);
        }
        self.audio = Some(PcAudio::open(self.sdl.clone(), render)?);
        Ok(())
    }

    fn stop_audio(&mut self) {
        self.audio = None;
    }

    fn audio_latency_ns(&self) -> u64 {
        self.audio.as_ref().map_or(0, PcAudio::latency_ns)
    }

    fn spawn(&self, spec: ThreadSpec, f: Box<dyn FnOnce() + Send + 'static>) -> Result<PcThread, ThreadError> {
        thread::spawn(spec, self.config.pin_threads, f)
    }

    fn pin_current_thread(&self, core: Core) -> bool {
        thread::pin_current(core)
    }

    fn current_core() -> Option<u32> {
        thread::current_core()
    }
}

impl Drop for PcPlatform {
    fn drop(&mut self) {
        self.audio = None;
        for slot in &mut self.controllers {
            if let Some(c) = slot.take() {
                // SAFETY: open handle, closed once.
                unsafe { ffi::SDL_GameControllerClose(c.handle) };
            }
        }
        CURRENT_GL.with(|c| *c.borrow_mut() = None);
        TAKEN.store(false, Ordering::Release);
    }
}
