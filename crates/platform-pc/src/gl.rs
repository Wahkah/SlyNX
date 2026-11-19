//! The few OpenGL 4.3 core functions the platform itself needs (offscreen framebuffer, readback, debug draw),
//! loaded through the context's `get_proc_address`. The renderer loads its own, larger set the same way.
#![allow(non_snake_case, clippy::too_many_arguments)]

use core::ffi::{CStr, c_char, c_void};

pub type GLenum = u32;
pub type GLuint = u32;
pub type GLint = i32;
pub type GLsizei = i32;
pub type GLbitfield = u32;
pub type GLfloat = f32;
pub type GLboolean = u8;
pub type GLsizeiptr = isize;

pub const FRAMEBUFFER: GLenum = 0x8D40;
pub const READ_FRAMEBUFFER: GLenum = 0x8CA8;
pub const RENDERBUFFER: GLenum = 0x8D41;
pub const RGBA8: GLenum = 0x8058;
pub const DEPTH24_STENCIL8: GLenum = 0x88F0;
pub const COLOR_ATTACHMENT0: GLenum = 0x8CE0;
pub const DEPTH_STENCIL_ATTACHMENT: GLenum = 0x821A;
pub const FRAMEBUFFER_COMPLETE: GLenum = 0x8CD5;
pub const RGBA: GLenum = 0x1908;
pub const UNSIGNED_BYTE: GLenum = 0x1401;
pub const FLOAT: GLenum = 0x1406;
pub const COLOR_BUFFER_BIT: GLbitfield = 0x4000;
pub const DEPTH_BUFFER_BIT: GLbitfield = 0x0100;
pub const STENCIL_BUFFER_BIT: GLbitfield = 0x0400;
pub const TRIANGLES: GLenum = 0x0004;
pub const ARRAY_BUFFER: GLenum = 0x8892;
pub const STREAM_DRAW: GLenum = 0x88E0;
pub const VERTEX_SHADER: GLenum = 0x8B31;
pub const FRAGMENT_SHADER: GLenum = 0x8B30;
pub const COMPILE_STATUS: GLenum = 0x8B81;
pub const LINK_STATUS: GLenum = 0x8B82;
pub const PACK_ALIGNMENT: GLenum = 0x0D05;
pub const VERSION: GLenum = 0x1F02;
pub const RENDERER: GLenum = 0x1F01;
pub const SCISSOR_TEST: GLenum = 0x0C11;
pub const DEPTH_TEST: GLenum = 0x0B71;
pub const CULL_FACE: GLenum = 0x0B44;
pub const BLEND: GLenum = 0x0BE2;
pub const FRAMEBUFFER_SRGB: GLenum = 0x8DB9;

macro_rules! gl_functions {
    ($($name:ident : fn($($arg:ident : $ty:ty),*) $(-> $ret:ty)?;)*) => {
        /// Loaded function pointers. Every pointer was checked non-null at load time.
        #[derive(Clone, Copy)]
        pub struct Gl {
            $(pub $name: unsafe extern "system" fn($($ty),*) $(-> $ret)?,)*
        }

        impl Gl {
            /// Loads every function with `get`. Fails with the name of the first missing one.
            ///
            /// # Safety
            ///
            /// `get` must return, for each `gl*` name, either NULL or the address of that OpenGL function with the
            /// signature declared here (from the OpenGL 4.3 core specification), valid while the context lives.
            /// A context's `get_proc_address` (SDL, EGL) satisfies this while that context is current.
            pub unsafe fn load(mut get: impl FnMut(&CStr) -> *const c_void) -> Result<Gl, String> {
                Ok(Gl {
                    $($name: {
                        let name = concat!("gl", stringify!($name), "\0");
                        let cname = CStr::from_bytes_with_nul(name.as_bytes()).unwrap();
                        let p = get(cname);
                        if p.is_null() {
                            return Err(format!("missing GL function {}", &name[..name.len() - 1]));
                        }
                        // SAFETY: by this function's contract `p` is the entry point for this name with the
                        // signature declared here.
                        unsafe { core::mem::transmute::<*const c_void, unsafe extern "system" fn($($ty),*) $(-> $ret)?>(p) }
                    },)*
                })
            }
        }
    };
}

gl_functions! {
    GetString: fn(name: GLenum) -> *const u8;
    GetError: fn() -> GLenum;
    Enable: fn(cap: GLenum);
    Disable: fn(cap: GLenum);
    Viewport: fn(x: GLint, y: GLint, w: GLsizei, h: GLsizei);
    ClearColor: fn(r: GLfloat, g: GLfloat, b: GLfloat, a: GLfloat);
    Clear: fn(mask: GLbitfield);
    Flush: fn();
    Finish: fn();
    PixelStorei: fn(pname: GLenum, param: GLint);
    ReadPixels: fn(x: GLint, y: GLint, w: GLsizei, h: GLsizei, format: GLenum, ty: GLenum, data: *mut c_void);
    GenFramebuffers: fn(n: GLsizei, ids: *mut GLuint);
    DeleteFramebuffers: fn(n: GLsizei, ids: *const GLuint);
    BindFramebuffer: fn(target: GLenum, id: GLuint);
    CheckFramebufferStatus: fn(target: GLenum) -> GLenum;
    GenRenderbuffers: fn(n: GLsizei, ids: *mut GLuint);
    DeleteRenderbuffers: fn(n: GLsizei, ids: *const GLuint);
    BindRenderbuffer: fn(target: GLenum, id: GLuint);
    RenderbufferStorage: fn(target: GLenum, format: GLenum, w: GLsizei, h: GLsizei);
    FramebufferRenderbuffer: fn(target: GLenum, attachment: GLenum, rbtarget: GLenum, rb: GLuint);
    CreateShader: fn(ty: GLenum) -> GLuint;
    DeleteShader: fn(shader: GLuint);
    ShaderSource: fn(shader: GLuint, count: GLsizei, strings: *const *const c_char, lengths: *const GLint);
    CompileShader: fn(shader: GLuint);
    GetShaderiv: fn(shader: GLuint, pname: GLenum, out: *mut GLint);
    GetShaderInfoLog: fn(shader: GLuint, max: GLsizei, len: *mut GLsizei, log: *mut c_char);
    CreateProgram: fn() -> GLuint;
    DeleteProgram: fn(program: GLuint);
    AttachShader: fn(program: GLuint, shader: GLuint);
    LinkProgram: fn(program: GLuint);
    GetProgramiv: fn(program: GLuint, pname: GLenum, out: *mut GLint);
    GetProgramInfoLog: fn(program: GLuint, max: GLsizei, len: *mut GLsizei, log: *mut c_char);
    UseProgram: fn(program: GLuint);
    GenVertexArrays: fn(n: GLsizei, ids: *mut GLuint);
    DeleteVertexArrays: fn(n: GLsizei, ids: *const GLuint);
    BindVertexArray: fn(id: GLuint);
    GenBuffers: fn(n: GLsizei, ids: *mut GLuint);
    DeleteBuffers: fn(n: GLsizei, ids: *const GLuint);
    BindBuffer: fn(target: GLenum, id: GLuint);
    BufferData: fn(target: GLenum, size: GLsizeiptr, data: *const c_void, usage: GLenum);
    VertexAttribPointer: fn(index: GLuint, size: GLint, ty: GLenum, normalized: GLboolean, stride: GLsizei, offset: *const c_void);
    EnableVertexAttribArray: fn(index: GLuint);
    DrawArrays: fn(mode: GLenum, first: GLint, count: GLsizei);
}

impl Gl {
    /// `glGetString` as a Rust string (empty if the driver returns NULL).
    pub fn string(&self, name: GLenum) -> String {
        // SAFETY: a context is current (the caller holds one); the result is a static NUL-terminated string.
        unsafe {
            let p = (self.GetString)(name);
            if p.is_null() { String::new() } else { CStr::from_ptr(p.cast()).to_string_lossy().into_owned() }
        }
    }
}
