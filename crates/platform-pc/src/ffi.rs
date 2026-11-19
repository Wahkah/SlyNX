//! Hand-written declarations for the parts of SDL2, EGL and the C library this crate uses. Values come from the
//! public SDL2 2.x headers (`SDL_video.h`, `SDL_events.h`, `SDL_gamecontroller.h`, `SDL_audio.h`,
//! `SDL_scancode.h`) and the Khronos EGL headers.
#![allow(non_camel_case_types, non_snake_case, dead_code)]

use core::ffi::{c_char, c_int, c_void};

// ---------------------------------------------------------------------------------------------------------------
// SDL2

pub const SDL_INIT_AUDIO: u32 = 0x0000_0010;
pub const SDL_INIT_VIDEO: u32 = 0x0000_0020;
pub const SDL_INIT_GAMECONTROLLER: u32 = 0x0000_2000;
pub const SDL_INIT_EVENTS: u32 = 0x0000_4000;

pub const SDL_WINDOW_OPENGL: u32 = 0x0000_0002;
pub const SDL_WINDOW_HIDDEN: u32 = 0x0000_0008;
pub const SDL_WINDOW_RESIZABLE: u32 = 0x0000_0020;
pub const SDL_WINDOWPOS_CENTERED: c_int = 0x2FFF_0000;

// SDL_GLattr
pub const SDL_GL_RED_SIZE: c_int = 0;
pub const SDL_GL_GREEN_SIZE: c_int = 1;
pub const SDL_GL_BLUE_SIZE: c_int = 2;
pub const SDL_GL_ALPHA_SIZE: c_int = 3;
pub const SDL_GL_DOUBLEBUFFER: c_int = 5;
pub const SDL_GL_DEPTH_SIZE: c_int = 6;
pub const SDL_GL_STENCIL_SIZE: c_int = 7;
pub const SDL_GL_CONTEXT_MAJOR_VERSION: c_int = 17;
pub const SDL_GL_CONTEXT_MINOR_VERSION: c_int = 18;
pub const SDL_GL_CONTEXT_FLAGS: c_int = 20;
pub const SDL_GL_CONTEXT_PROFILE_MASK: c_int = 21;
pub const SDL_GL_CONTEXT_PROFILE_CORE: c_int = 1;
pub const SDL_GL_CONTEXT_FORWARD_COMPATIBLE_FLAG: c_int = 2;

// Event types
pub const SDL_QUIT: u32 = 0x100;
pub const SDL_WINDOWEVENT: u32 = 0x200;
pub const SDL_KEYDOWN: u32 = 0x300;
pub const SDL_CONTROLLERDEVICEADDED: u32 = 0x653;
pub const SDL_CONTROLLERDEVICEREMOVED: u32 = 0x654;

// SDL_WindowEventID
pub const SDL_WINDOWEVENT_SIZE_CHANGED: u8 = 6;
pub const SDL_WINDOWEVENT_MINIMIZED: u8 = 7;
pub const SDL_WINDOWEVENT_RESTORED: u8 = 9;
pub const SDL_WINDOWEVENT_FOCUS_GAINED: u8 = 12;
pub const SDL_WINDOWEVENT_FOCUS_LOST: u8 = 13;
pub const SDL_WINDOWEVENT_CLOSE: u8 = 14;

// SDL_Scancode
pub const SCANCODE_A: usize = 4;
pub const SCANCODE_1: usize = 30;
pub const SCANCODE_RETURN: usize = 40;
pub const SCANCODE_ESCAPE: usize = 41;
pub const SCANCODE_BACKSPACE: usize = 42;
pub const SCANCODE_F11: usize = 68;
pub const SCANCODE_RIGHT: usize = 79;
pub const SCANCODE_LEFT: usize = 80;
pub const SCANCODE_DOWN: usize = 81;
pub const SCANCODE_UP: usize = 82;

/// Scancode of an upper-case ASCII letter.
pub const fn scancode_letter(c: u8) -> usize {
    SCANCODE_A + (c - b'A') as usize
}

/// Scancode of a digit key 1..9 on the main row.
pub const fn scancode_digit(d: u8) -> usize {
    SCANCODE_1 + (d - 1) as usize
}

// SDL_GameControllerButton
pub const CONTROLLER_BUTTON_A: c_int = 0;
pub const CONTROLLER_BUTTON_B: c_int = 1;
pub const CONTROLLER_BUTTON_X: c_int = 2;
pub const CONTROLLER_BUTTON_Y: c_int = 3;
pub const CONTROLLER_BUTTON_BACK: c_int = 4;
pub const CONTROLLER_BUTTON_START: c_int = 6;
pub const CONTROLLER_BUTTON_LEFTSTICK: c_int = 7;
pub const CONTROLLER_BUTTON_RIGHTSTICK: c_int = 8;
pub const CONTROLLER_BUTTON_LEFTSHOULDER: c_int = 9;
pub const CONTROLLER_BUTTON_RIGHTSHOULDER: c_int = 10;
pub const CONTROLLER_BUTTON_DPAD_UP: c_int = 11;
pub const CONTROLLER_BUTTON_DPAD_DOWN: c_int = 12;
pub const CONTROLLER_BUTTON_DPAD_LEFT: c_int = 13;
pub const CONTROLLER_BUTTON_DPAD_RIGHT: c_int = 14;

// SDL_GameControllerAxis
pub const CONTROLLER_AXIS_LEFTX: c_int = 0;
pub const CONTROLLER_AXIS_LEFTY: c_int = 1;
pub const CONTROLLER_AXIS_RIGHTX: c_int = 2;
pub const CONTROLLER_AXIS_RIGHTY: c_int = 3;
pub const CONTROLLER_AXIS_TRIGGERLEFT: c_int = 4;
pub const CONTROLLER_AXIS_TRIGGERRIGHT: c_int = 5;

// SDL_GameControllerType
pub const CONTROLLER_TYPE_NINTENDO_SWITCH_PRO: c_int = 5;
pub const CONTROLLER_TYPE_NINTENDO_SWITCH_JOYCON_LEFT: c_int = 11;
pub const CONTROLLER_TYPE_NINTENDO_SWITCH_JOYCON_RIGHT: c_int = 12;
pub const CONTROLLER_TYPE_NINTENDO_SWITCH_JOYCON_PAIR: c_int = 13;

pub const AUDIO_F32SYS: u16 = if cfg!(target_endian = "little") { 0x8120 } else { 0x9120 };

/// `SDL_Event`: a 56-byte union. Fields are read by offset (see the accessors).
#[repr(C, align(8))]
pub struct SDL_Event(pub [u8; 56]);

impl SDL_Event {
    pub fn zeroed() -> SDL_Event {
        SDL_Event([0; 56])
    }
    fn u32_at(&self, off: usize) -> u32 {
        u32::from_ne_bytes(self.0[off..off + 4].try_into().unwrap())
    }
    pub fn kind(&self) -> u32 {
        self.u32_at(0)
    }
    /// `SDL_WindowEvent.event`.
    pub fn window_event(&self) -> u8 {
        self.0[12]
    }
    /// `SDL_ControllerDeviceEvent.which`.
    pub fn device_which(&self) -> i32 {
        self.u32_at(8) as i32
    }
    /// `SDL_KeyboardEvent.keysym.scancode`.
    pub fn key_scancode(&self) -> usize {
        self.u32_at(16) as usize
    }
    /// `SDL_KeyboardEvent.repeat`.
    pub fn key_repeat(&self) -> bool {
        self.0[13] != 0
    }
}

pub type SDL_AudioCallback = Option<unsafe extern "C" fn(userdata: *mut c_void, stream: *mut u8, len: c_int)>;

#[repr(C)]
pub struct SDL_AudioSpec {
    pub freq: c_int,
    pub format: u16,
    pub channels: u8,
    pub silence: u8,
    pub samples: u16,
    pub padding: u16,
    pub size: u32,
    pub callback: SDL_AudioCallback,
    pub userdata: *mut c_void,
}

#[link(name = "SDL2")]
unsafe extern "C" {
    pub fn SDL_Init(flags: u32) -> c_int;
    pub fn SDL_InitSubSystem(flags: u32) -> c_int;
    pub fn SDL_QuitSubSystem(flags: u32);
    pub fn SDL_Quit();
    pub fn SDL_GetError() -> *const c_char;
    pub fn SDL_SetHint(name: *const c_char, value: *const c_char) -> c_int;

    pub fn SDL_CreateWindow(title: *const c_char, x: c_int, y: c_int, w: c_int, h: c_int, flags: u32) -> *mut c_void;
    pub fn SDL_DestroyWindow(window: *mut c_void);
    pub fn SDL_SetWindowSize(window: *mut c_void, w: c_int, h: c_int);
    pub fn SDL_GL_SetAttribute(attr: c_int, value: c_int) -> c_int;
    pub fn SDL_GL_CreateContext(window: *mut c_void) -> *mut c_void;
    pub fn SDL_GL_DeleteContext(context: *mut c_void);
    pub fn SDL_GL_MakeCurrent(window: *mut c_void, context: *mut c_void) -> c_int;
    pub fn SDL_GL_GetProcAddress(proc_: *const c_char) -> *mut c_void;
    pub fn SDL_GL_SwapWindow(window: *mut c_void);
    pub fn SDL_GL_SetSwapInterval(interval: c_int) -> c_int;
    pub fn SDL_GL_GetDrawableSize(window: *mut c_void, w: *mut c_int, h: *mut c_int);

    pub fn SDL_PollEvent(event: *mut SDL_Event) -> c_int;
    pub fn SDL_GetKeyboardState(numkeys: *mut c_int) -> *const u8;

    pub fn SDL_IsGameController(joystick_index: c_int) -> c_int;
    pub fn SDL_GameControllerOpen(joystick_index: c_int) -> *mut c_void;
    pub fn SDL_GameControllerClose(gamecontroller: *mut c_void);
    pub fn SDL_GameControllerGetJoystick(gamecontroller: *mut c_void) -> *mut c_void;
    pub fn SDL_JoystickInstanceID(joystick: *mut c_void) -> i32;
    pub fn SDL_GameControllerGetButton(gamecontroller: *mut c_void, button: c_int) -> u8;
    pub fn SDL_GameControllerGetAxis(gamecontroller: *mut c_void, axis: c_int) -> i16;
    pub fn SDL_GameControllerGetType(gamecontroller: *mut c_void) -> c_int;

    pub fn SDL_OpenAudioDevice(
        device: *const c_char,
        iscapture: c_int,
        desired: *const SDL_AudioSpec,
        obtained: *mut SDL_AudioSpec,
        allowed_changes: c_int,
    ) -> u32;
    pub fn SDL_PauseAudioDevice(dev: u32, pause_on: c_int);
    pub fn SDL_CloseAudioDevice(dev: u32);
}

/// SDL's last error message.
pub fn sdl_error() -> String {
    // SAFETY: SDL_GetError returns a valid NUL-terminated string (possibly empty) owned by SDL.
    unsafe {
        let p = SDL_GetError();
        if p.is_null() { String::new() } else { core::ffi::CStr::from_ptr(p).to_string_lossy().into_owned() }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// EGL (Linux only): a context with no window, for headless rendering.

#[cfg(target_os = "linux")]
pub mod egl {
    use core::ffi::{c_char, c_void};

    pub type EGLDisplay = *mut c_void;
    pub type EGLConfig = *mut c_void;
    pub type EGLContext = *mut c_void;
    pub type EGLSurface = *mut c_void;

    pub const EGL_EXTENSIONS: i32 = 0x3055;
    pub const EGL_PLATFORM_SURFACELESS_MESA: u32 = 0x31DD;
    pub const EGL_OPENGL_API: u32 = 0x30A2;
    pub const EGL_RENDERABLE_TYPE: i32 = 0x3040;
    pub const EGL_OPENGL_BIT: i32 = 0x0008;
    pub const EGL_NONE: i32 = 0x3038;
    pub const EGL_CONTEXT_MAJOR_VERSION: i32 = 0x3098;
    pub const EGL_CONTEXT_MINOR_VERSION: i32 = 0x30FB;
    pub const EGL_CONTEXT_OPENGL_PROFILE_MASK: i32 = 0x30FD;
    pub const EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT: i32 = 0x1;

    #[link(name = "EGL")]
    unsafe extern "C" {
        pub fn eglGetProcAddress(procname: *const c_char) -> *mut c_void;
        pub fn eglQueryString(dpy: EGLDisplay, name: i32) -> *const c_char;
        pub fn eglGetPlatformDisplay(
            platform: u32,
            native_display: *mut c_void,
            attrib_list: *const isize,
        ) -> EGLDisplay;
        pub fn eglInitialize(dpy: EGLDisplay, major: *mut i32, minor: *mut i32) -> u32;
        pub fn eglTerminate(dpy: EGLDisplay) -> u32;
        pub fn eglBindAPI(api: u32) -> u32;
        pub fn eglChooseConfig(
            dpy: EGLDisplay,
            attrib_list: *const i32,
            configs: *mut EGLConfig,
            config_size: i32,
            num_config: *mut i32,
        ) -> u32;
        pub fn eglCreateContext(
            dpy: EGLDisplay,
            config: EGLConfig,
            share_context: EGLContext,
            attrib_list: *const i32,
        ) -> EGLContext;
        pub fn eglDestroyContext(dpy: EGLDisplay, ctx: EGLContext) -> u32;
        pub fn eglMakeCurrent(dpy: EGLDisplay, draw: EGLSurface, read: EGLSurface, ctx: EGLContext) -> u32;
        pub fn eglGetError() -> i32;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Thread affinity and the current core.

#[cfg(target_os = "linux")]
unsafe extern "C" {
    pub fn sched_setaffinity(pid: i32, cpusetsize: usize, mask: *const u64) -> c_int;
    pub fn sched_getcpu() -> c_int;
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GetCurrentThread() -> *mut c_void;
    pub fn SetThreadAffinityMask(thread: *mut c_void, mask: usize) -> usize;
    pub fn GetCurrentProcessorNumber() -> u32;
}
