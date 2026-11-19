//! The OpenGL 4.3 core context: an SDL window, or with no window an EGL surfaceless context (Linux) or a hidden SDL
//! window rendering into an offscreen framebuffer.

use crate::ffi::{self, sdl_error};
use crate::gl::{self, Gl};
use std::cell::Cell;
use std::ffi::{CStr, CString, c_void};
use std::rc::Rc;

/// Owns SDL initialisation; the last owner shuts SDL down.
pub(crate) struct Sdl;

impl Sdl {
    pub fn init() -> Result<Rc<Sdl>, String> {
        // Positional face buttons on every controller (see `input.rs`). Must be set before controllers open.
        set_hint("SDL_GAMECONTROLLER_USE_BUTTON_LABELS", "0");
        // Keep the desktop compositor on for a development window.
        set_hint("SDL_VIDEO_X11_NET_WM_BYPASS_COMPOSITOR", "0");
        // SAFETY: called once per platform (guarded by the platform's singleton flag); 0 initialises no subsystem.
        if unsafe { ffi::SDL_Init(0) } != 0 {
            return Err(format!("SDL_Init: {}", sdl_error()));
        }
        Ok(Rc::new(Sdl))
    }

    pub fn subsystem(&self, flags: u32) -> Result<(), String> {
        // SAFETY: SDL is initialised while `self` exists.
        if unsafe { ffi::SDL_InitSubSystem(flags) } != 0 {
            return Err(sdl_error());
        }
        Ok(())
    }
}

impl Drop for Sdl {
    fn drop(&mut self) {
        // SAFETY: every SDL object is owned by something holding an `Rc<Sdl>`, so they are all gone.
        unsafe { ffi::SDL_Quit() }
    }
}

pub(crate) fn set_hint(name: &str, value: &str) {
    let (n, v) = (CString::new(name).unwrap(), CString::new(value).unwrap());
    // SAFETY: both are NUL-terminated; SDL copies them.
    unsafe { ffi::SDL_SetHint(n.as_ptr(), v.as_ptr()) };
}

enum Backend {
    Sdl {
        window: *mut c_void,
        context: *mut c_void,
    },
    #[cfg(target_os = "linux")]
    Egl {
        display: ffi::egl::EGLDisplay,
        context: ffi::egl::EGLContext,
    },
}

/// Offscreen colour + depth/stencil target used when there is no visible window.
struct Offscreen {
    fbo: u32,
    color: u32,
    depth: u32,
}

/// The GL context and its window (if any), shared by the platform and the debug draw.
pub(crate) struct GlShared {
    backend: Backend,
    pub gl: Gl,
    offscreen: Option<Offscreen>,
    size: Cell<(u32, u32)>,
    _sdl: Rc<Sdl>,
}

impl GlShared {
    /// A visible (or, with `hidden`, invisible) SDL window with a GL 4.3 core context.
    pub fn sdl_window(
        sdl: Rc<Sdl>,
        title: &str,
        w: u32,
        h: u32,
        hidden: bool,
        vsync: bool,
    ) -> Result<GlShared, String> {
        sdl.subsystem(ffi::SDL_INIT_VIDEO).map_err(|e| format!("SDL video: {e}"))?;
        // SAFETY: plain SDL calls with valid arguments; the window and context are owned by the returned value.
        unsafe {
            for (attr, value) in [
                (ffi::SDL_GL_CONTEXT_MAJOR_VERSION, 4),
                (ffi::SDL_GL_CONTEXT_MINOR_VERSION, 3),
                (ffi::SDL_GL_CONTEXT_PROFILE_MASK, ffi::SDL_GL_CONTEXT_PROFILE_CORE),
                (ffi::SDL_GL_CONTEXT_FLAGS, ffi::SDL_GL_CONTEXT_FORWARD_COMPATIBLE_FLAG),
                (ffi::SDL_GL_RED_SIZE, 8),
                (ffi::SDL_GL_GREEN_SIZE, 8),
                (ffi::SDL_GL_BLUE_SIZE, 8),
                (ffi::SDL_GL_ALPHA_SIZE, 8),
                (ffi::SDL_GL_DEPTH_SIZE, 24),
                (ffi::SDL_GL_STENCIL_SIZE, 8),
                (ffi::SDL_GL_DOUBLEBUFFER, 1),
            ] {
                ffi::SDL_GL_SetAttribute(attr, value);
            }
            let title = CString::new(title.replace('\0', " ")).unwrap();
            let mut flags = ffi::SDL_WINDOW_OPENGL | ffi::SDL_WINDOW_RESIZABLE;
            if hidden {
                flags |= ffi::SDL_WINDOW_HIDDEN;
            }
            let window = ffi::SDL_CreateWindow(
                title.as_ptr(),
                ffi::SDL_WINDOWPOS_CENTERED,
                ffi::SDL_WINDOWPOS_CENTERED,
                w as i32,
                h as i32,
                flags,
            );
            if window.is_null() {
                return Err(format!("SDL_CreateWindow: {}", sdl_error()));
            }
            let context = ffi::SDL_GL_CreateContext(window);
            if context.is_null() {
                let e = sdl_error();
                ffi::SDL_DestroyWindow(window);
                return Err(format!("OpenGL 4.3 core context: {e}"));
            }
            ffi::SDL_GL_MakeCurrent(window, context);
            ffi::SDL_GL_SetSwapInterval(if vsync { 1 } else { 0 });
            let backend = Backend::Sdl { window, context };
            // The context was just made current, so SDL's loader returns this context's entry points.
            let gl = match Gl::load(|n| ffi::SDL_GL_GetProcAddress(n.as_ptr()) as *const c_void) {
                Ok(gl) => gl,
                Err(e) => {
                    ffi::SDL_GL_DeleteContext(context);
                    ffi::SDL_DestroyWindow(window);
                    return Err(e);
                }
            };
            let mut shared = GlShared { backend, gl, offscreen: None, size: Cell::new((w, h)), _sdl: sdl };
            if hidden {
                shared.make_offscreen(w, h)?;
            } else {
                shared.refresh_size();
            }
            Ok(shared)
        }
    }

    /// An EGL context with no surface at all (Mesa's surfaceless platform), rendering into an offscreen target.
    /// Works with no display server.
    #[cfg(target_os = "linux")]
    pub fn egl_surfaceless(sdl: Rc<Sdl>, w: u32, h: u32) -> Result<GlShared, String> {
        use ffi::egl::*;
        // SAFETY: standard EGL bring-up; every handle created here is released on failure or by Drop.
        unsafe {
            let exts = eglQueryString(core::ptr::null_mut(), EGL_EXTENSIONS);
            let has_surfaceless =
                !exts.is_null() && CStr::from_ptr(exts).to_string_lossy().contains("EGL_MESA_platform_surfaceless");
            if !has_surfaceless {
                return Err("EGL_MESA_platform_surfaceless not available".into());
            }
            let display =
                eglGetPlatformDisplay(EGL_PLATFORM_SURFACELESS_MESA, core::ptr::null_mut(), core::ptr::null());
            if display.is_null() {
                return Err(format!("eglGetPlatformDisplay: {:#x}", eglGetError()));
            }
            let (mut major, mut minor) = (0, 0);
            if eglInitialize(display, &mut major, &mut minor) == 0 {
                return Err(format!("eglInitialize: {:#x}", eglGetError()));
            }
            let fail = |what: &str| {
                let e = format!("{what}: {:#x}", eglGetError());
                eglTerminate(display);
                Err(e)
            };
            if eglBindAPI(EGL_OPENGL_API) == 0 {
                return fail("eglBindAPI");
            }
            let attribs = [EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT, EGL_NONE];
            let mut config = core::ptr::null_mut();
            let mut n = 0;
            if eglChooseConfig(display, attribs.as_ptr(), &mut config, 1, &mut n) == 0 || n < 1 {
                return fail("eglChooseConfig");
            }
            let ctx_attribs = [
                EGL_CONTEXT_MAJOR_VERSION,
                4,
                EGL_CONTEXT_MINOR_VERSION,
                3,
                EGL_CONTEXT_OPENGL_PROFILE_MASK,
                EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT,
                EGL_NONE,
            ];
            let context = eglCreateContext(display, config, core::ptr::null_mut(), ctx_attribs.as_ptr());
            if context.is_null() {
                return fail("eglCreateContext (OpenGL 4.3 core)");
            }
            if eglMakeCurrent(display, core::ptr::null_mut(), core::ptr::null_mut(), context) == 0 {
                eglDestroyContext(display, context);
                return fail("eglMakeCurrent");
            }
            let backend = Backend::Egl { display, context };
            // The context was just made current, so EGL's loader returns this context's entry points.
            let gl = match Gl::load(|n| eglGetProcAddress(n.as_ptr()) as *const c_void) {
                Ok(gl) => gl,
                Err(e) => {
                    eglMakeCurrent(display, core::ptr::null_mut(), core::ptr::null_mut(), core::ptr::null_mut());
                    eglDestroyContext(display, context);
                    eglTerminate(display);
                    return Err(e);
                }
            };
            let mut shared = GlShared { backend, gl, offscreen: None, size: Cell::new((w, h)), _sdl: sdl };
            shared.make_offscreen(w, h)?;
            Ok(shared)
        }
    }

    fn make_offscreen(&mut self, w: u32, h: u32) -> Result<(), String> {
        let gl = &self.gl;
        let (mut fbo, mut color, mut depth) = (0, 0, 0);
        // SAFETY: the context is current; all names are generated here and attached to the bound framebuffer.
        let status = unsafe {
            (gl.GenFramebuffers)(1, &mut fbo);
            (gl.GenRenderbuffers)(1, &mut color);
            (gl.GenRenderbuffers)(1, &mut depth);
            (gl.BindRenderbuffer)(gl::RENDERBUFFER, color);
            (gl.RenderbufferStorage)(gl::RENDERBUFFER, gl::RGBA8, w as i32, h as i32);
            (gl.BindRenderbuffer)(gl::RENDERBUFFER, depth);
            (gl.RenderbufferStorage)(gl::RENDERBUFFER, gl::DEPTH24_STENCIL8, w as i32, h as i32);
            (gl.BindRenderbuffer)(gl::RENDERBUFFER, 0);
            (gl.BindFramebuffer)(gl::FRAMEBUFFER, fbo);
            (gl.FramebufferRenderbuffer)(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::RENDERBUFFER, color);
            (gl.FramebufferRenderbuffer)(gl::FRAMEBUFFER, gl::DEPTH_STENCIL_ATTACHMENT, gl::RENDERBUFFER, depth);
            (gl.CheckFramebufferStatus)(gl::FRAMEBUFFER)
        };
        self.offscreen = Some(Offscreen { fbo, color, depth });
        self.size.set((w, h));
        if status != gl::FRAMEBUFFER_COMPLETE {
            return Err(format!("offscreen framebuffer incomplete: {status:#x}"));
        }
        Ok(())
    }

    pub fn get_proc_address(&self, name: &CStr) -> *const c_void {
        // SAFETY: `name` is NUL-terminated; both loaders accept any name and return NULL if unknown.
        unsafe {
            match self.backend {
                Backend::Sdl { .. } => ffi::SDL_GL_GetProcAddress(name.as_ptr()) as *const c_void,
                #[cfg(target_os = "linux")]
                Backend::Egl { .. } => ffi::egl::eglGetProcAddress(name.as_ptr()) as *const c_void,
            }
        }
    }

    pub fn default_framebuffer(&self) -> u32 {
        self.offscreen.as_ref().map_or(0, |o| o.fbo)
    }

    pub fn is_offscreen(&self) -> bool {
        self.offscreen.is_some()
    }

    pub fn size(&self) -> (u32, u32) {
        self.size.get()
    }

    pub fn window(&self) -> Option<*mut c_void> {
        match self.backend {
            Backend::Sdl { window, .. } if self.offscreen.is_none() => Some(window),
            _ => None,
        }
    }

    /// Re-reads the drawable size of a visible window. Returns true if it changed.
    pub fn refresh_size(&self) -> bool {
        let Some(window) = self.window() else { return false };
        let (mut w, mut h) = (0, 0);
        // SAFETY: `window` is live while `self` is.
        unsafe { ffi::SDL_GL_GetDrawableSize(window, &mut w, &mut h) };
        let new = (w.max(1) as u32, h.max(1) as u32);
        let changed = new != self.size.get();
        self.size.set(new);
        changed
    }

    /// Shows the frame: swaps a visible window; offscreen, just flushes.
    pub fn present(&self) {
        match self.window() {
            // SAFETY: the window and its context are live and current.
            Some(window) => unsafe { ffi::SDL_GL_SwapWindow(window) },
            // SAFETY: the context is current.
            None => unsafe { (self.gl.Flush)() },
        }
    }

    /// Reads the default framebuffer as RGBA8, rows top to bottom.
    pub fn read_pixels(&self) -> Vec<u8> {
        let (w, h) = self.size();
        let row = w as usize * 4;
        let mut data = vec![0u8; row * h as usize];
        // SAFETY: the context is current; `data` holds exactly w * h RGBA8 pixels with pack alignment 1.
        unsafe {
            (self.gl.BindFramebuffer)(gl::READ_FRAMEBUFFER, self.default_framebuffer());
            (self.gl.PixelStorei)(gl::PACK_ALIGNMENT, 1);
            (self.gl.ReadPixels)(0, 0, w as i32, h as i32, gl::RGBA, gl::UNSIGNED_BYTE, data.as_mut_ptr().cast());
        }
        // GL returns the bottom row first.
        let mut flipped = vec![0u8; data.len()];
        for (dst, src) in flipped.chunks_exact_mut(row).zip(data.chunks_exact(row).rev()) {
            dst.copy_from_slice(src);
        }
        flipped
    }
}

impl Drop for GlShared {
    fn drop(&mut self) {
        // SAFETY: the context is current on this thread (it is never made current elsewhere); the objects were
        // created by this value.
        unsafe {
            if let Some(o) = self.offscreen.take() {
                (self.gl.DeleteFramebuffers)(1, &o.fbo);
                (self.gl.DeleteRenderbuffers)(1, &o.color);
                (self.gl.DeleteRenderbuffers)(1, &o.depth);
            }
            match self.backend {
                Backend::Sdl { window, context } => {
                    ffi::SDL_GL_DeleteContext(context);
                    ffi::SDL_DestroyWindow(window);
                }
                #[cfg(target_os = "linux")]
                Backend::Egl { display, context } => {
                    use ffi::egl::*;
                    eglMakeCurrent(display, core::ptr::null_mut(), core::ptr::null_mut(), core::ptr::null_mut());
                    eglDestroyContext(display, context);
                    eglTerminate(display);
                }
            }
        }
    }
}
