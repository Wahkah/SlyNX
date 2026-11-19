//! Headless OpenGL context through EGL (Linux, Mesa), for tests and screenshot runs without a window.
//!
//! libEGL is opened at run time with `dlopen`, so building needs no EGL development files and a machine without
//! EGL just gets an error. The context is OpenGL 4.3 core on Mesa's surfaceless platform (no window system), made
//! current without a surface; the backend renders into its own offscreen framebuffer.
//!
//! This exists so the renderer can be tested on its own. The platform layer's windowed and headless contexts are
//! the normal way to get a context.

use alloc::format;
use alloc::string::String;
use core::ffi::{CStr, c_char, c_int, c_void};
use core::marker::PhantomData;

type EglDisplay = *mut c_void;
type EglConfig = *mut c_void;
type EglContext = *mut c_void;
type EglInt = i32;

const EGL_NONE: EglInt = 0x3038;
const EGL_SURFACE_TYPE: EglInt = 0x3033;
const EGL_RENDERABLE_TYPE: EglInt = 0x3040;
const EGL_OPENGL_BIT: EglInt = 0x0008;
const EGL_PBUFFER_BIT: EglInt = 0x0001;
const EGL_OPENGL_API: u32 = 0x30A2;
const EGL_CONTEXT_MAJOR_VERSION: EglInt = 0x3098;
const EGL_CONTEXT_MINOR_VERSION: EglInt = 0x30FB;
const EGL_CONTEXT_OPENGL_PROFILE_MASK: EglInt = 0x30FD;
const EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT: EglInt = 0x0001;
const EGL_PLATFORM_SURFACELESS_MESA: u32 = 0x31DD;
const RTLD_NOW: c_int = 2;

unsafe extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}

struct Egl {
    get_proc_address: unsafe extern "C" fn(*const c_char) -> *const c_void,
    get_display: unsafe extern "C" fn(*mut c_void) -> EglDisplay,
    initialize: unsafe extern "C" fn(EglDisplay, *mut EglInt, *mut EglInt) -> u32,
    bind_api: unsafe extern "C" fn(u32) -> u32,
    choose_config: unsafe extern "C" fn(EglDisplay, *const EglInt, *mut EglConfig, EglInt, *mut EglInt) -> u32,
    create_context: unsafe extern "C" fn(EglDisplay, EglConfig, EglContext, *const EglInt) -> EglContext,
    make_current: unsafe extern "C" fn(EglDisplay, *mut c_void, *mut c_void, EglContext) -> u32,
    destroy_context: unsafe extern "C" fn(EglDisplay, EglContext) -> u32,
    terminate: unsafe extern "C" fn(EglDisplay) -> u32,
    get_error: unsafe extern "C" fn() -> EglInt,
}

/// A headless OpenGL 4.3 core context, current on the thread that created it.
pub struct HeadlessGl {
    lib: *mut c_void,
    gl_lib: *mut c_void,
    egl: Egl,
    display: EglDisplay,
    context: EglContext,
    _single_thread: PhantomData<*const ()>,
}

/// Looks up `name` in `lib` as a function pointer of type `F`.
///
/// # Safety
/// `F` must be the function's real signature (an `unsafe extern "C" fn`).
unsafe fn sym<F: Copy>(lib: *mut c_void, name: &CStr) -> Result<F, String> {
    // SAFETY: lib is a live dlopen handle and name is NUL-terminated.
    let p = unsafe { dlsym(lib, name.as_ptr()) };
    if p.is_null() {
        return Err(format!("libEGL has no {}", name.to_string_lossy()));
    }
    // SAFETY: the caller states F is the symbol's signature; F is a function pointer, the size of a pointer.
    Ok(unsafe { core::mem::transmute_copy::<*mut c_void, F>(&p) })
}

impl HeadlessGl {
    /// Opens libEGL and creates a surfaceless OpenGL 4.3 core context, current on this thread.
    pub fn new() -> Result<HeadlessGl, String> {
        // SAFETY: dlopen with a NUL-terminated name; every symbol below is looked up with its EGL 1.4/1.5
        // signature, and every EGL call follows the EGL specification's argument rules (attribute lists are
        // EGL_NONE-terminated, out pointers are live locals).
        unsafe {
            let lib = dlopen(c"libEGL.so.1".as_ptr(), RTLD_NOW);
            if lib.is_null() {
                return Err(String::from("cannot open libEGL.so.1"));
            }
            let egl = Egl {
                get_proc_address: sym(lib, c"eglGetProcAddress")?,
                get_display: sym(lib, c"eglGetDisplay")?,
                initialize: sym(lib, c"eglInitialize")?,
                bind_api: sym(lib, c"eglBindAPI")?,
                choose_config: sym(lib, c"eglChooseConfig")?,
                create_context: sym(lib, c"eglCreateContext")?,
                make_current: sym(lib, c"eglMakeCurrent")?,
                destroy_context: sym(lib, c"eglDestroyContext")?,
                terminate: sym(lib, c"eglTerminate")?,
                get_error: sym(lib, c"eglGetError")?,
            };
            let mut me = HeadlessGl {
                lib,
                gl_lib: dlopen(c"libOpenGL.so.0".as_ptr(), RTLD_NOW),
                egl,
                display: core::ptr::null_mut(),
                context: core::ptr::null_mut(),
                _single_thread: PhantomData,
            };

            // Prefer Mesa's surfaceless platform (no X or Wayland needed), else the default display.
            type GetPlatformDisplay = unsafe extern "C" fn(u32, *mut c_void, *const EglInt) -> EglDisplay;
            let gpd = (me.egl.get_proc_address)(c"eglGetPlatformDisplayEXT".as_ptr());
            if !gpd.is_null() {
                let f: GetPlatformDisplay = core::mem::transmute_copy(&gpd);
                me.display = f(EGL_PLATFORM_SURFACELESS_MESA, core::ptr::null_mut(), core::ptr::null());
            }
            if me.display.is_null() {
                me.display = (me.egl.get_display)(core::ptr::null_mut());
            }
            if me.display.is_null() {
                return Err(String::from("no EGL display"));
            }
            let (mut major, mut minor) = (0, 0);
            if (me.egl.initialize)(me.display, &mut major, &mut minor) == 0 {
                return Err(me.error("eglInitialize"));
            }
            if (me.egl.bind_api)(EGL_OPENGL_API) == 0 {
                return Err(me.error("eglBindAPI(OpenGL)"));
            }
            let mut config: EglConfig = core::ptr::null_mut();
            let mut n = 0;
            let attrs = [EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT, EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_NONE];
            if (me.egl.choose_config)(me.display, attrs.as_ptr(), &mut config, 1, &mut n) == 0 || n == 0 {
                // Surfaceless displays may only offer configs without surface types.
                let attrs = [EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT, EGL_SURFACE_TYPE, 0, EGL_NONE];
                if (me.egl.choose_config)(me.display, attrs.as_ptr(), &mut config, 1, &mut n) == 0 || n == 0 {
                    return Err(me.error("eglChooseConfig (OpenGL)"));
                }
            }
            let ctx_attrs = [
                EGL_CONTEXT_MAJOR_VERSION,
                4,
                EGL_CONTEXT_MINOR_VERSION,
                3,
                EGL_CONTEXT_OPENGL_PROFILE_MASK,
                EGL_CONTEXT_OPENGL_CORE_PROFILE_BIT,
                EGL_NONE,
            ];
            me.context = (me.egl.create_context)(me.display, config, core::ptr::null_mut(), ctx_attrs.as_ptr());
            if me.context.is_null() {
                return Err(me.error("eglCreateContext (OpenGL 4.3 core)"));
            }
            if (me.egl.make_current)(me.display, core::ptr::null_mut(), core::ptr::null_mut(), me.context) == 0 {
                return Err(me.error("eglMakeCurrent (surfaceless)"));
            }
            Ok(me)
        }
    }

    fn error(&self, what: &str) -> String {
        // SAFETY: eglGetError takes no arguments.
        let e = unsafe { (self.egl.get_error)() };
        format!("{what} failed (EGL error {e:#x})")
    }

    /// Address of an OpenGL function, or null.
    pub fn get_proc_address(&self, name: &CStr) -> *const c_void {
        // SAFETY: name is NUL-terminated; eglGetProcAddress and dlsym only read it.
        unsafe {
            let p = (self.egl.get_proc_address)(name.as_ptr());
            if !p.is_null() || self.gl_lib.is_null() {
                return p;
            }
            dlsym(self.gl_lib, name.as_ptr())
        }
    }
}

impl Drop for HeadlessGl {
    fn drop(&mut self) {
        // SAFETY: handles created in `new`; each is released once. Null handles are skipped.
        unsafe {
            if !self.display.is_null() {
                (self.egl.make_current)(
                    self.display,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                );
                if !self.context.is_null() {
                    (self.egl.destroy_context)(self.display, self.context);
                }
                (self.egl.terminate)(self.display);
            }
            if !self.gl_lib.is_null() {
                dlclose(self.gl_lib);
            }
            dlclose(self.lib);
        }
    }
}
