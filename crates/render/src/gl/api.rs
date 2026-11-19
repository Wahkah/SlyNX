//! Hand-written OpenGL 4.3 core function loading: only the entry points the backend uses.
//!
//! [`Gl::load`] resolves every function through the platform's `get_proc_address` and fails if one is missing.
//! The wrappers are split in two groups:
//!
//! - safe wrappers for calls whose arguments are plain values (names, enums, sizes): with a current context, the
//!   driver validates them and reports errors through `glGetError`, so a wrong value cannot corrupt memory;
//! - `unsafe` wrappers for calls that take pointers or make the GPU read through offsets into buffers.
//!
//! `Gl` is neither `Send` nor `Sync`: an OpenGL context is current on one thread only.

#![allow(clippy::too_many_arguments)]

use core::ffi::{CStr, c_void};
use core::marker::PhantomData;

pub type GLenum = u32;
pub type GLuint = u32;
pub type GLint = i32;
pub type GLsizei = i32;
pub type GLsizeiptr = isize;
pub type GLintptr = isize;
pub type GLboolean = u8;
pub type GLbitfield = u32;
pub type GLuint64 = u64;
pub type GLsync = *const c_void;

pub const NO_ERROR: GLenum = 0;
pub const FALSE: GLboolean = 0;
pub const TRUE: GLboolean = 1;
pub const ONE: GLenum = 1;
pub const ZERO: GLenum = 0;
pub const DST_ALPHA: GLenum = 0x0304;
pub const ONE_MINUS_DST_ALPHA: GLenum = 0x0305;
pub const SRC_COLOR: GLenum = 0x0300;
pub const TEXTURE1: GLenum = 0x84C1;
pub const TEXTURE2: GLenum = 0x84C2;
pub const TEXTURE5: GLenum = 0x84C5;
pub const R8: GLenum = 0x8229;
pub const RED: GLenum = 0x1903;
pub const CLAMP_TO_EDGE: GLint = 0x812F;
pub const STENCIL_BUFFER_BIT: GLbitfield = 0x0400;
pub const LEQUAL: GLenum = 0x0203;
pub const NOTEQUAL: GLenum = 0x0205;
pub const ALWAYS: GLenum = 0x0207;
pub const KEEP: GLenum = 0x1E00;
pub const INCR_WRAP: GLenum = 0x8507;
pub const DECR_WRAP: GLenum = 0x8508;
pub const STENCIL_TEST: GLenum = 0x0B90;
pub const DEPTH24_STENCIL8: GLenum = 0x88F0;
pub const DEPTH_STENCIL_ATTACHMENT: GLenum = 0x821A;
pub const DEPTH_BUFFER_BIT: GLbitfield = 0x0100;
pub const COLOR_BUFFER_BIT: GLbitfield = 0x4000;
pub const TRIANGLES: GLenum = 0x0004;
pub const LESS: GLenum = 0x0201;
pub const SRC_ALPHA: GLenum = 0x0302;
pub const ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
pub const FRONT: GLenum = 0x0404;
pub const BACK: GLenum = 0x0405;
pub const CW: GLenum = 0x0900;
pub const CCW: GLenum = 0x0901;
pub const CULL_FACE: GLenum = 0x0B44;
pub const DEPTH_TEST: GLenum = 0x0B71;
pub const BLEND: GLenum = 0x0BE2;
pub const SCISSOR_TEST: GLenum = 0x0C11;
pub const PACK_ALIGNMENT: GLenum = 0x0D05;
pub const UNPACK_ALIGNMENT: GLenum = 0x0CF5;
pub const TEXTURE_2D: GLenum = 0x0DE1;
pub const BYTE: GLenum = 0x1400;
pub const UNSIGNED_BYTE: GLenum = 0x1401;
pub const UNSIGNED_SHORT: GLenum = 0x1403;
pub const UNSIGNED_INT: GLenum = 0x1405;
pub const FLOAT: GLenum = 0x1406;
pub const RGBA: GLenum = 0x1908;
pub const VENDOR: GLenum = 0x1F00;
pub const RENDERER: GLenum = 0x1F01;
pub const VERSION: GLenum = 0x1F02;
pub const LINEAR: GLint = 0x2601;
pub const LINEAR_MIPMAP_LINEAR: GLint = 0x2703;
pub const TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub const TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub const TEXTURE_WRAP_S: GLenum = 0x2802;
pub const TEXTURE_WRAP_T: GLenum = 0x2803;
pub const REPEAT: GLint = 0x2901;
pub const RGBA8: GLenum = 0x8058;
pub const TEXTURE_MAX_LEVEL: GLenum = 0x813D;
pub const DEPTH_COMPONENT24: GLenum = 0x81A6;
pub const MAJOR_VERSION: GLenum = 0x821B;
pub const MINOR_VERSION: GLenum = 0x821C;
pub const TEXTURE0: GLenum = 0x84C0;
pub const ARRAY_BUFFER: GLenum = 0x8892;
pub const ELEMENT_ARRAY_BUFFER: GLenum = 0x8893;
pub const STATIC_DRAW: GLenum = 0x88E4;
pub const STREAM_DRAW: GLenum = 0x88E0;
pub const TIME_ELAPSED: GLenum = 0x88BF;
pub const QUERY_RESULT: GLenum = 0x8866;
pub const QUERY_RESULT_AVAILABLE: GLenum = 0x8867;
pub const FRAGMENT_SHADER: GLenum = 0x8B30;
pub const VERTEX_SHADER: GLenum = 0x8B31;
pub const COMPILE_STATUS: GLenum = 0x8B81;
pub const LINK_STATUS: GLenum = 0x8B82;
pub const INFO_LOG_LENGTH: GLenum = 0x8B84;
pub const TEXTURE_2D_ARRAY: GLenum = 0x8C1A;
pub const FRAMEBUFFER_COMPLETE: GLenum = 0x8CD5;
pub const COLOR_ATTACHMENT0: GLenum = 0x8CE0;
pub const DEPTH_ATTACHMENT: GLenum = 0x8D00;
pub const FRAMEBUFFER: GLenum = 0x8D40;
pub const RENDERBUFFER: GLenum = 0x8D41;
pub const UNIFORM_BUFFER: GLenum = 0x8A11;
pub const UNIFORM_BUFFER_OFFSET_ALIGNMENT: GLenum = 0x8A34;
pub const COPY_WRITE_BUFFER: GLenum = 0x8F37;
pub const DRAW_INDIRECT_BUFFER: GLenum = 0x8F3F;
pub const SHADER_STORAGE_BUFFER: GLenum = 0x90D2;
pub const SHADER_STORAGE_BUFFER_OFFSET_ALIGNMENT: GLenum = 0x90DF;
pub const SYNC_GPU_COMMANDS_COMPLETE: GLenum = 0x9117;
pub const SYNC_FLUSH_COMMANDS_BIT: GLbitfield = 0x0001;
pub const ALREADY_SIGNALED: GLenum = 0x911A;
pub const TIMEOUT_EXPIRED: GLenum = 0x911B;
pub const CONDITION_SATISFIED: GLenum = 0x911C;
pub const WAIT_FAILED: GLenum = 0x911D;
pub const MAP_WRITE_BIT: GLbitfield = 0x0002;
pub const MAP_INVALIDATE_RANGE_BIT: GLbitfield = 0x0004;
pub const MAP_UNSYNCHRONIZED_BIT: GLbitfield = 0x0020;

/// A required entry point the context does not provide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissingFunction(pub &'static CStr);

macro_rules! gl_api {
    (
        safe { $( fn $sname:ident($($sa:ident : $st:ty),*) $(-> $sr:ty)? = $sgl:literal; )* }
        raw { $( fn $uname:ident($($ua:ident : $ut:ty),*) $(-> $ur:ty)? = $ugl:literal; )* }
    ) => {
        struct Fns {
            $( $sname: unsafe extern "system" fn($($st),*) $(-> $sr)?, )*
            $( $uname: unsafe extern "system" fn($($ut),*) $(-> $ur)?, )*
        }

        /// Loaded OpenGL entry points. See the module documentation for the safety model.
        pub struct Gl {
            f: Fns,
            _single_thread: PhantomData<*const ()>,
        }

        impl Gl {
            /// Resolves every entry point through `loader` (the platform's `get_proc_address`).
            ///
            /// # Safety
            /// An OpenGL 4.3 (or later) core context must be current on this thread, and stay current for as long
            /// as the returned `Gl` is used. `loader` must return either null or the address of the named function
            /// with the standard OpenGL signature.
            pub unsafe fn load(loader: &mut dyn FnMut(&CStr) -> *const c_void) -> Result<Gl, MissingFunction> {
                let f = Fns {
                    $( $sname: {
                        let p = loader($sgl);
                        if p.is_null() { return Err(MissingFunction($sgl)); }
                        // SAFETY: non-null and, per the caller's contract, the function with this signature.
                        unsafe { core::mem::transmute::<*const c_void, unsafe extern "system" fn($($st),*) $(-> $sr)?>(p) }
                    }, )*
                    $( $uname: {
                        let p = loader($ugl);
                        if p.is_null() { return Err(MissingFunction($ugl)); }
                        // SAFETY: as above.
                        unsafe { core::mem::transmute::<*const c_void, unsafe extern "system" fn($($ut),*) $(-> $ur)?>(p) }
                    }, )*
                };
                Ok(Gl { f, _single_thread: PhantomData })
            }

            $(
                #[inline]
                pub fn $sname(&self, $($sa: $st),*) $(-> $sr)? {
                    // SAFETY: the context is current (contract of `load`, and `Gl` cannot leave this thread); the
                    // arguments are plain values the driver validates.
                    unsafe { (self.f.$sname)($($sa),*) }
                }
            )*

            $(
                /// # Safety
                /// As the OpenGL function: pointers must be valid for the sizes the call reads or writes, and
                /// offsets into bound buffers must stay inside them.
                #[inline]
                pub unsafe fn $uname(&self, $($ua: $ut),*) $(-> $ur)? {
                    // SAFETY: forwarded to the caller.
                    unsafe { (self.f.$uname)($($ua),*) }
                }
            )*
        }
    };
}

gl_api! {
    safe {
        fn get_error() -> GLenum = c"glGetError";
        fn enable(cap: GLenum) = c"glEnable";
        fn disable(cap: GLenum) = c"glDisable";
        fn viewport(x: GLint, y: GLint, w: GLsizei, h: GLsizei) = c"glViewport";
        fn scissor(x: GLint, y: GLint, w: GLsizei, h: GLsizei) = c"glScissor";
        fn clear_color(r: f32, g: f32, b: f32, a: f32) = c"glClearColor";
        fn clear_depthf(d: f32) = c"glClearDepthf";
        fn clear(mask: GLbitfield) = c"glClear";
        fn depth_mask(flag: GLboolean) = c"glDepthMask";
        fn depth_func(func: GLenum) = c"glDepthFunc";
        fn blend_func(src: GLenum, dst: GLenum) = c"glBlendFunc";
        fn blend_func_separate(src: GLenum, dst: GLenum, src_a: GLenum, dst_a: GLenum) = c"glBlendFuncSeparate";
        fn tex_storage_2d(target: GLenum, levels: GLsizei, fmt: GLenum, w: GLsizei, h: GLsizei) = c"glTexStorage2D";
        fn framebuffer_texture_2d(target: GLenum, att: GLenum, textarget: GLenum, tex: GLuint, level: GLint) = c"glFramebufferTexture2D";
        fn cull_face(mode: GLenum) = c"glCullFace";
        fn color_mask(r: GLboolean, g: GLboolean, b: GLboolean, a: GLboolean) = c"glColorMask";
        fn stencil_func(func: GLenum, reference: GLint, mask: GLuint) = c"glStencilFunc";
        fn stencil_op(sfail: GLenum, dpfail: GLenum, dppass: GLenum) = c"glStencilOp";
        fn stencil_op_separate(face: GLenum, sfail: GLenum, dpfail: GLenum, dppass: GLenum) = c"glStencilOpSeparate";
        fn clear_stencil(s: GLint) = c"glClearStencil";
        fn front_face(mode: GLenum) = c"glFrontFace";
        fn bind_vertex_array(vao: GLuint) = c"glBindVertexArray";
        fn vertex_attrib_format(i: GLuint, size: GLint, ty: GLenum, norm: GLboolean, off: GLuint) = c"glVertexAttribFormat";
        fn vertex_attrib_i_format(i: GLuint, size: GLint, ty: GLenum, off: GLuint) = c"glVertexAttribIFormat";
        fn vertex_attrib_binding(i: GLuint, binding: GLuint) = c"glVertexAttribBinding";
        fn vertex_binding_divisor(binding: GLuint, divisor: GLuint) = c"glVertexBindingDivisor";
        fn bind_vertex_buffer(binding: GLuint, buf: GLuint, off: GLintptr, stride: GLsizei) = c"glBindVertexBuffer";
        fn enable_vertex_attrib_array(i: GLuint) = c"glEnableVertexAttribArray";
        fn bind_buffer(target: GLenum, buf: GLuint) = c"glBindBuffer";
        fn bind_buffer_range(target: GLenum, index: GLuint, buf: GLuint, off: GLintptr, size: GLsizeiptr) = c"glBindBufferRange";
        fn unmap_buffer(target: GLenum) -> GLboolean = c"glUnmapBuffer";
        fn create_shader(ty: GLenum) -> GLuint = c"glCreateShader";
        fn compile_shader(s: GLuint) = c"glCompileShader";
        fn delete_shader(s: GLuint) = c"glDeleteShader";
        fn create_program() -> GLuint = c"glCreateProgram";
        fn attach_shader(p: GLuint, s: GLuint) = c"glAttachShader";
        fn detach_shader(p: GLuint, s: GLuint) = c"glDetachShader";
        fn link_program(p: GLuint) = c"glLinkProgram";
        fn use_program(p: GLuint) = c"glUseProgram";
        fn delete_program(p: GLuint) = c"glDeleteProgram";
        fn active_texture(unit: GLenum) = c"glActiveTexture";
        fn bind_texture(target: GLenum, tex: GLuint) = c"glBindTexture";
        fn tex_storage_3d(target: GLenum, levels: GLsizei, fmt: GLenum, w: GLsizei, h: GLsizei, d: GLsizei) = c"glTexStorage3D";
        fn tex_parameter_i(target: GLenum, name: GLenum, value: GLint) = c"glTexParameteri";
        fn pixel_store_i(name: GLenum, value: GLint) = c"glPixelStorei";
        fn bind_framebuffer(target: GLenum, fb: GLuint) = c"glBindFramebuffer";
        fn check_framebuffer_status(target: GLenum) -> GLenum = c"glCheckFramebufferStatus";
        fn bind_renderbuffer(target: GLenum, rb: GLuint) = c"glBindRenderbuffer";
        fn renderbuffer_storage(target: GLenum, fmt: GLenum, w: GLsizei, h: GLsizei) = c"glRenderbufferStorage";
        fn framebuffer_renderbuffer(target: GLenum, att: GLenum, rbtarget: GLenum, rb: GLuint) = c"glFramebufferRenderbuffer";
        fn begin_query(target: GLenum, id: GLuint) = c"glBeginQuery";
        fn end_query(target: GLenum) = c"glEndQuery";
        fn fence_sync(cond: GLenum, flags: GLbitfield) -> GLsync = c"glFenceSync";
        fn finish() = c"glFinish";
        fn flush() = c"glFlush";
    }
    raw {
        fn get_integer_v(name: GLenum, out: *mut GLint) = c"glGetIntegerv";
        fn get_string(name: GLenum) -> *const u8 = c"glGetString";
        fn gen_vertex_arrays(n: GLsizei, out: *mut GLuint) = c"glGenVertexArrays";
        fn delete_vertex_arrays(n: GLsizei, names: *const GLuint) = c"glDeleteVertexArrays";
        fn gen_buffers(n: GLsizei, out: *mut GLuint) = c"glGenBuffers";
        fn delete_buffers(n: GLsizei, names: *const GLuint) = c"glDeleteBuffers";
        fn buffer_data(target: GLenum, size: GLsizeiptr, data: *const c_void, usage: GLenum) = c"glBufferData";
        fn map_buffer_range(target: GLenum, off: GLintptr, len: GLsizeiptr, access: GLbitfield) -> *mut c_void = c"glMapBufferRange";
        fn shader_source(s: GLuint, count: GLsizei, strings: *const *const u8, lens: *const GLint) = c"glShaderSource";
        fn get_shader_iv(s: GLuint, name: GLenum, out: *mut GLint) = c"glGetShaderiv";
        fn get_shader_info_log(s: GLuint, cap: GLsizei, len: *mut GLsizei, log: *mut u8) = c"glGetShaderInfoLog";
        fn get_program_iv(p: GLuint, name: GLenum, out: *mut GLint) = c"glGetProgramiv";
        fn get_program_info_log(p: GLuint, cap: GLsizei, len: *mut GLsizei, log: *mut u8) = c"glGetProgramInfoLog";
        fn gen_textures(n: GLsizei, out: *mut GLuint) = c"glGenTextures";
        fn delete_textures(n: GLsizei, names: *const GLuint) = c"glDeleteTextures";
        fn tex_sub_image_2d(target: GLenum, level: GLint, x: GLint, y: GLint, w: GLsizei, h: GLsizei, fmt: GLenum, ty: GLenum, data: *const c_void) = c"glTexSubImage2D";
        fn tex_sub_image_3d(target: GLenum, level: GLint, x: GLint, y: GLint, z: GLint, w: GLsizei, h: GLsizei, d: GLsizei, fmt: GLenum, ty: GLenum, data: *const c_void) = c"glTexSubImage3D";
        fn gen_framebuffers(n: GLsizei, out: *mut GLuint) = c"glGenFramebuffers";
        fn delete_framebuffers(n: GLsizei, names: *const GLuint) = c"glDeleteFramebuffers";
        fn gen_renderbuffers(n: GLsizei, out: *mut GLuint) = c"glGenRenderbuffers";
        fn delete_renderbuffers(n: GLsizei, names: *const GLuint) = c"glDeleteRenderbuffers";
        fn read_pixels(x: GLint, y: GLint, w: GLsizei, h: GLsizei, fmt: GLenum, ty: GLenum, out: *mut c_void) = c"glReadPixels";
        fn multi_draw_elements_indirect(mode: GLenum, ty: GLenum, indirect: *const c_void, count: GLsizei, stride: GLsizei) = c"glMultiDrawElementsIndirect";
        fn client_wait_sync(sync: GLsync, flags: GLbitfield, timeout: GLuint64) -> GLenum = c"glClientWaitSync";
        fn delete_sync(sync: GLsync) = c"glDeleteSync";
        fn gen_queries(n: GLsizei, out: *mut GLuint) = c"glGenQueries";
        fn delete_queries(n: GLsizei, names: *const GLuint) = c"glDeleteQueries";
        fn get_query_object_ui64v(id: GLuint, name: GLenum, out: *mut GLuint64) = c"glGetQueryObjectui64v";
    }
}

/// Object kinds with `glGen*` / `glDelete*` pairs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Object {
    VertexArray,
    Buffer,
    Texture,
    Framebuffer,
    Renderbuffer,
    Query,
}

/// `glGetIntegerv` queries that return exactly one value (so [`Gl::get_integer`] cannot write past its result).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum IntQuery {
    MajorVersion = MAJOR_VERSION,
    MinorVersion = MINOR_VERSION,
    UniformBufferOffsetAlignment = UNIFORM_BUFFER_OFFSET_ALIGNMENT,
    ShaderStorageBufferOffsetAlignment = SHADER_STORAGE_BUFFER_OFFSET_ALIGNMENT,
}

impl Gl {
    /// One new object name of `kind` (`glGen*` with a count of 1).
    pub fn gen_one(&self, kind: Object) -> GLuint {
        let mut n = 0;
        let out: *mut GLuint = &mut n;
        // SAFETY: each glGen* call writes exactly one name (count 1) into a live local.
        unsafe {
            match kind {
                Object::VertexArray => self.gen_vertex_arrays(1, out),
                Object::Buffer => self.gen_buffers(1, out),
                Object::Texture => self.gen_textures(1, out),
                Object::Framebuffer => self.gen_framebuffers(1, out),
                Object::Renderbuffer => self.gen_renderbuffers(1, out),
                Object::Query => self.gen_queries(1, out),
            }
        }
        n
    }

    /// Deletes one object name of `kind` (`glDelete*` with a count of 1). Name 0 is ignored.
    pub fn delete_one(&self, kind: Object, name: GLuint) {
        if name == 0 {
            return;
        }
        let p: *const GLuint = &name;
        // SAFETY: each glDelete* call reads exactly one name (count 1) from a live local; the driver ignores names
        // that are not objects of that kind.
        unsafe {
            match kind {
                Object::VertexArray => self.delete_vertex_arrays(1, p),
                Object::Buffer => self.delete_buffers(1, p),
                Object::Texture => self.delete_textures(1, p),
                Object::Framebuffer => self.delete_framebuffers(1, p),
                Object::Renderbuffer => self.delete_renderbuffers(1, p),
                Object::Query => self.delete_queries(1, p),
            }
        }
    }

    /// A single-value integer state query.
    pub fn get_integer(&self, query: IntQuery) -> GLint {
        let mut v = 0;
        // SAFETY: every `IntQuery` returns exactly one integer, so the one-element out-parameter is large enough.
        unsafe { self.get_integer_v(query as GLenum, &mut v) };
        v
    }

    /// A `glGetString` value, or an empty string.
    pub fn string(&self, name: GLenum) -> alloc::string::String {
        // SAFETY: glGetString returns null or a static NUL-terminated string owned by the driver.
        unsafe {
            let p = self.get_string(name);
            if p.is_null() {
                return alloc::string::String::new();
            }
            CStr::from_ptr(p.cast()).to_string_lossy().into_owned()
        }
    }
}
