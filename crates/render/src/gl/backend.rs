//! OpenGL 4.3 core backend.
//!
//! Per frame, independent of the number of objects:
//! - wait for the frame slot's fence (the GPU finished with it `FRAME_SLOTS` frames ago);
//! - map the slot's buffer once and copy in the uniforms, instance records and indirect draw commands;
//! - bind the uniform range, the storage range and the indirect buffer once;
//! - per batch: switch program/state if the pass changed, bind the pool's array texture if it changed, and issue one
//!   `glMultiDrawElementsIndirect`.
//!
//! The vertex array object never changes: binding 0 is the geometry arena, binding 1 a per-instance stream of
//! slot indices 0, 1, 2, ... (divisor 1) that the draw's `baseInstance` offsets, which is how the vertex shader
//! finds its instance record without `gl_DrawID` (OpenGL 4.6) or per-draw uniforms.

use super::api::*;
use crate::backend::{Backend, ClipSpace, Limits, SubmitInfo};
use crate::frame::*;
use crate::resources::LevelUpload;
use crate::types::{CullMode, RenderError, TextureFormat, Vertex, VertexExtra, VertexWeights};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::ffi::{CStr, c_void};
use core::mem::size_of;

/// Frames that may be in flight; each has its own buffer and fence.
pub const FRAME_SLOTS: usize = 3;
/// Instance slot stream length, and so the most live instances.
pub const MAX_INSTANCES: u32 = 1 << 16;
/// Most draw commands per frame.
pub const MAX_DRAWS: u32 = 1 << 16;
/// Most bone matrices per frame.
pub const MAX_BONES: u32 = 1 << 16;
/// Most morphed vertices per frame.
pub const MAX_MORPH_VERTICES: u32 = 1 << 18;

const VERT_SRC: &str = include_str!("../../shaders/mesh_vsh.glsl");
const FRAG_SRC: &str = include_str!("../../shaders/mesh_fsh.glsl");
const FRAG_ALPHA_SRC: &str = include_str!("../../shaders/mesh_alpha_fsh.glsl");

/// Where the backend draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlTarget {
    /// A framebuffer the platform owns: 0 for a window's default framebuffer, or the platform's own offscreen
    /// framebuffer in headless mode. The platform presents (swaps) after `render`.
    External { framebuffer: u32, width: u32, height: u32 },
    /// An offscreen RGBA8 + depth framebuffer owned by the backend (headless runs, screenshots).
    Offscreen { width: u32, height: u32 },
}

struct Slot {
    buffer: GLuint,
    capacity: usize,
    fence: GLsync,
    query: GLuint,
    query_pending: bool,
}

struct Pool {
    texture: GLuint,
}

/// The OpenGL 4.3 backend. Must be used on the thread where its context is current.
pub struct GlBackend {
    gl: Gl,
    programs: [GLuint; 2], // [opaque/blend, alpha-tested]
    vao: GLuint,
    arena_vbo: GLuint,
    /// The arena's second stream (`VertexExtra`).
    extra_vbo: GLuint,
    /// Per-instance vertex weights (storage buffer 3), and its size.
    weights_ssbo: GLuint,
    weights_size: usize,
    /// Outline edge points (storage buffer 4), and its size.
    outline_ssbo: GLuint,
    outline_size: usize,
    arena_ibo: GLuint,
    slot_stream: GLuint,
    pools: Vec<Pool>,
    slots: Vec<Slot>,
    frame: u64,
    target: GlTarget,
    fbo: GLuint,
    color_rb: GLuint,
    depth_rb: GLuint,
    /// The projectors' silhouette image (texture unit 1) and its framebuffer.
    silhouette: GLuint,
    silhouette_fbo: GLuint,
    /// The panel images (texture units 5 and 6): colour textures, depth renderbuffers, framebuffers.
    panel_tex: [GLuint; 2],
    panel_depth: [GLuint; 2],
    panel_fbo: [GLuint; 2],
    /// The movie picture's planes (texture units 2 to 4) and their size.
    video: [GLuint; 3],
    video_size: (u32, u32),
    ssbo_align: usize,
    staging: Vec<u8>,
    last_gpu_ns: Option<u64>,
    renderer_name: String,
    clock: Option<fn() -> u64>,
}

fn backend_err(msg: String) -> RenderError {
    RenderError::Backend(msg)
}

fn align_up(v: usize, a: usize) -> usize {
    v.div_ceil(a) * a
}

/// Views a slice of plain-old-data records as bytes.
fn as_bytes<T: Copy>(v: &[T]) -> &[u8] {
    // SAFETY: T is one of this crate's #[repr(C)] records (Vertex, VertexExtra, GpuInstance, BoneMatrix, MorphVertex,
    // DrawCmd, FrameUniforms, u16, u32)
    // with no padding and no invalid bit patterns; the byte view covers exactly the slice's memory.
    unsafe { core::slice::from_raw_parts(v.as_ptr().cast::<u8>(), core::mem::size_of_val(v)) }
}

impl GlBackend {
    /// Creates the backend on the current context.
    ///
    /// # Safety
    /// An OpenGL 4.3 core context must be current on this thread for the backend's whole life, and `loader` must
    /// be that context's `get_proc_address` (see [`Gl::load`]).
    pub unsafe fn new(
        loader: &mut dyn FnMut(&CStr) -> *const c_void,
        target: GlTarget,
    ) -> Result<GlBackend, RenderError> {
        // SAFETY: forwarded from this function's contract.
        let gl = unsafe { Gl::load(loader) }
            .map_err(|m| backend_err(format!("missing OpenGL function {}", m.0.to_string_lossy())))?;
        let (major, minor) = (gl.get_integer(IntQuery::MajorVersion), gl.get_integer(IntQuery::MinorVersion));
        if (major, minor) < (4, 3) {
            return Err(backend_err(format!("OpenGL 4.3 required, context is {major}.{minor}")));
        }
        let renderer_name = format!("{} / {} / {}", gl.string(VENDOR), gl.string(RENDERER), gl.string(VERSION));

        let mut b = GlBackend {
            programs: [0; 2],
            vao: 0,
            arena_vbo: 0,
            extra_vbo: 0,
            weights_ssbo: 0,
            weights_size: 0,
            outline_ssbo: 0,
            outline_size: 0,
            arena_ibo: 0,
            slot_stream: 0,
            pools: Vec::new(),
            slots: Vec::new(),
            frame: 0,
            target,
            fbo: 0,
            color_rb: 0,
            depth_rb: 0,
            silhouette: 0,
            silhouette_fbo: 0,
            panel_tex: [0; 2],
            panel_depth: [0; 2],
            panel_fbo: [0; 2],
            video: [0; 3],
            video_size: (0, 0),
            ssbo_align: gl.get_integer(IntQuery::ShaderStorageBufferOffsetAlignment).max(16) as usize,
            staging: Vec::new(),
            last_gpu_ns: None,
            renderer_name,
            clock: None,
            gl,
        };
        b.programs[0] = b.program(FRAG_SRC)?;
        b.programs[1] = b.program(FRAG_ALPHA_SRC)?;
        b.create_vertex_state();
        for _ in 0..FRAME_SLOTS {
            let slot = Slot {
                buffer: b.gl.gen_one(Object::Buffer),
                capacity: 0,
                fence: core::ptr::null(),
                query: b.gl.gen_one(Object::Query),
                query_pending: false,
            };
            b.slots.push(slot);
        }
        match target {
            GlTarget::Offscreen { width, height } => b.create_offscreen(width, height)?,
            GlTarget::External { framebuffer, .. } => b.fbo = framebuffer,
        }
        b.create_silhouette()?;
        b.create_panel_images()?;
        b.check("init")?;
        Ok(b)
    }

    /// "vendor / renderer / version" of the context.
    pub fn renderer_name(&self) -> &str {
        &self.renderer_name
    }

    fn check(&self, what: &str) -> Result<(), RenderError> {
        let e = self.gl.get_error();
        if e == NO_ERROR { Ok(()) } else { Err(backend_err(format!("OpenGL error {e:#x} in {what}"))) }
    }

    fn shader(&self, ty: GLenum, src: &str) -> Result<GLuint, RenderError> {
        // The shared sources start with "#version 460" for uam; OpenGL 4.3 needs its own version line.
        let body = src.split_once('\n').map_or("", |(_, rest)| rest);
        let full = format!("#version 430 core\n{body}");
        let gl = &self.gl;
        let s = gl.create_shader(ty);
        let (ptr, len) = (full.as_ptr(), full.len() as GLint);
        // SAFETY: one string with an explicit length; the driver copies it.
        unsafe { gl.shader_source(s, 1, &ptr, &len) };
        gl.compile_shader(s);
        let mut ok = 0;
        // SAFETY: COMPILE_STATUS writes one integer.
        unsafe { gl.get_shader_iv(s, COMPILE_STATUS, &mut ok) };
        if ok == 0 {
            let log = self.info_log(s, Gl::get_shader_iv, Gl::get_shader_info_log);
            gl.delete_shader(s);
            return Err(backend_err(format!("shader compile failed: {log}")));
        }
        Ok(s)
    }

    fn info_log(
        &self,
        obj: GLuint,
        iv: unsafe fn(&Gl, GLuint, GLenum, *mut GLint),
        log: unsafe fn(&Gl, GLuint, GLsizei, *mut GLsizei, *mut u8),
    ) -> String {
        let mut len = 0;
        // SAFETY: INFO_LOG_LENGTH writes one integer; the log call writes at most `buf.len()` bytes.
        unsafe {
            iv(&self.gl, obj, INFO_LOG_LENGTH, &mut len);
            let mut buf = vec![0u8; len.max(1) as usize];
            let mut written = 0;
            log(&self.gl, obj, buf.len() as GLsizei, &mut written, buf.as_mut_ptr());
            buf.truncate(written.max(0) as usize);
            String::from_utf8_lossy(&buf).into_owned()
        }
    }

    fn program(&self, frag_src: &str) -> Result<GLuint, RenderError> {
        let gl = &self.gl;
        let vs = self.shader(VERTEX_SHADER, VERT_SRC)?;
        let fs = match self.shader(FRAGMENT_SHADER, frag_src) {
            Ok(fs) => fs,
            Err(e) => {
                gl.delete_shader(vs);
                return Err(e);
            }
        };
        let p = gl.create_program();
        gl.attach_shader(p, vs);
        gl.attach_shader(p, fs);
        gl.link_program(p);
        gl.detach_shader(p, vs);
        gl.detach_shader(p, fs);
        gl.delete_shader(vs);
        gl.delete_shader(fs);
        let mut ok = 0;
        // SAFETY: LINK_STATUS writes one integer.
        unsafe { gl.get_program_iv(p, LINK_STATUS, &mut ok) };
        if ok == 0 {
            let log = self.info_log(p, Gl::get_program_iv, Gl::get_program_info_log);
            gl.delete_program(p);
            return Err(backend_err(format!("program link failed: {log}")));
        }
        Ok(p)
    }

    fn create_vertex_state(&mut self) {
        let gl = &self.gl;
        self.vao = gl.gen_one(Object::VertexArray);
        self.arena_vbo = gl.gen_one(Object::Buffer);
        self.extra_vbo = gl.gen_one(Object::Buffer);
        self.weights_ssbo = gl.gen_one(Object::Buffer);
        self.outline_ssbo = gl.gen_one(Object::Buffer);
        self.arena_ibo = gl.gen_one(Object::Buffer);
        self.slot_stream = gl.gen_one(Object::Buffer);
        gl.bind_vertex_array(self.vao);

        // Binding 0: the arena (see `Vertex`).
        gl.vertex_attrib_format(0, 3, FLOAT, FALSE, 0);
        gl.vertex_attrib_format(1, 2, FLOAT, FALSE, 12);
        gl.vertex_attrib_format(2, 4, BYTE, TRUE, 20);
        gl.vertex_attrib_format(3, 4, UNSIGNED_BYTE, TRUE, 24);
        for a in 0..4 {
            gl.vertex_attrib_binding(a, 0);
            gl.enable_vertex_attrib_array(a);
        }
        // Binding 1: instance slot stream, one u32 per instance.
        gl.vertex_attrib_i_format(4, 1, UNSIGNED_INT, 0);
        gl.vertex_attrib_binding(4, 1);
        gl.enable_vertex_attrib_array(4);
        gl.vertex_binding_divisor(1, 1);
        // Binding 2: the arena's second stream (see `VertexExtra`): bones, weights, index in the mesh.
        gl.vertex_attrib_i_format(5, 4, UNSIGNED_BYTE, 0);
        gl.vertex_attrib_format(6, 4, UNSIGNED_SHORT, TRUE, 4);
        gl.vertex_attrib_i_format(7, 1, UNSIGNED_SHORT, 12);
        for a in 5..8 {
            gl.vertex_attrib_binding(a, 2);
            gl.enable_vertex_attrib_array(a);
        }

        let slots: Vec<u32> = (0..MAX_INSTANCES).collect();
        gl.bind_buffer(ARRAY_BUFFER, self.slot_stream);
        let bytes = as_bytes(&slots);
        // SAFETY: pointer and size describe `slots`; the driver copies it.
        unsafe { gl.buffer_data(ARRAY_BUFFER, bytes.len() as GLsizeiptr, bytes.as_ptr().cast(), STATIC_DRAW) };
        gl.bind_vertex_buffer(1, self.slot_stream, 0, 4);
        gl.bind_vertex_array(0);
    }

    fn create_offscreen(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
        let gl = &self.gl;
        self.fbo = gl.gen_one(Object::Framebuffer);
        self.color_rb = gl.gen_one(Object::Renderbuffer);
        self.depth_rb = gl.gen_one(Object::Renderbuffer);
        gl.bind_renderbuffer(RENDERBUFFER, self.color_rb);
        gl.renderbuffer_storage(RENDERBUFFER, RGBA8, width as GLsizei, height as GLsizei);
        gl.bind_renderbuffer(RENDERBUFFER, self.depth_rb);
        gl.renderbuffer_storage(RENDERBUFFER, DEPTH24_STENCIL8, width as GLsizei, height as GLsizei);
        gl.bind_framebuffer(FRAMEBUFFER, self.fbo);
        gl.framebuffer_renderbuffer(FRAMEBUFFER, COLOR_ATTACHMENT0, RENDERBUFFER, self.color_rb);
        gl.framebuffer_renderbuffer(FRAMEBUFFER, DEPTH_STENCIL_ATTACHMENT, RENDERBUFFER, self.depth_rb);
        let status = gl.check_framebuffer_status(FRAMEBUFFER);
        gl.bind_framebuffer(FRAMEBUFFER, 0);
        if status != FRAMEBUFFER_COMPLETE {
            return Err(backend_err(format!("offscreen framebuffer incomplete: {status:#x}")));
        }
        Ok(())
    }

    fn create_silhouette(&mut self) -> Result<(), RenderError> {
        let gl = &self.gl;
        self.silhouette = gl.gen_one(Object::Texture);
        gl.bind_texture(TEXTURE_2D, self.silhouette);
        let s = SILHOUETTE_SIZE as GLsizei;
        gl.tex_storage_2d(TEXTURE_2D, 1, RGBA8, s, s);
        gl.tex_parameter_i(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR);
        gl.tex_parameter_i(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR);
        gl.tex_parameter_i(TEXTURE_2D, TEXTURE_WRAP_S, CLAMP_TO_EDGE);
        gl.tex_parameter_i(TEXTURE_2D, TEXTURE_WRAP_T, CLAMP_TO_EDGE);
        gl.bind_texture(TEXTURE_2D, 0);
        self.silhouette_fbo = gl.gen_one(Object::Framebuffer);
        gl.bind_framebuffer(FRAMEBUFFER, self.silhouette_fbo);
        gl.framebuffer_texture_2d(FRAMEBUFFER, COLOR_ATTACHMENT0, TEXTURE_2D, self.silhouette, 0);
        let status = gl.check_framebuffer_status(FRAMEBUFFER);
        gl.bind_framebuffer(FRAMEBUFFER, self.fbo);
        if status != FRAMEBUFFER_COMPLETE {
            return Err(backend_err(format!("silhouette framebuffer incomplete: {status:#x}")));
        }
        Ok(())
    }

    fn create_panel_images(&mut self) -> Result<(), RenderError> {
        let gl = &self.gl;
        let s = PANEL_IMAGE_SIZE as GLsizei;
        for k in 0..2 {
            let t = gl.gen_one(Object::Texture);
            gl.bind_texture(TEXTURE_2D, t);
            gl.tex_storage_2d(TEXTURE_2D, 1, RGBA8, s, s);
            gl.tex_parameter_i(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR);
            gl.tex_parameter_i(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR);
            gl.tex_parameter_i(TEXTURE_2D, TEXTURE_WRAP_S, CLAMP_TO_EDGE);
            gl.tex_parameter_i(TEXTURE_2D, TEXTURE_WRAP_T, CLAMP_TO_EDGE);
            gl.bind_texture(TEXTURE_2D, 0);
            let d = gl.gen_one(Object::Renderbuffer);
            gl.bind_renderbuffer(RENDERBUFFER, d);
            gl.renderbuffer_storage(RENDERBUFFER, DEPTH24_STENCIL8, s, s);
            let f = gl.gen_one(Object::Framebuffer);
            gl.bind_framebuffer(FRAMEBUFFER, f);
            gl.framebuffer_texture_2d(FRAMEBUFFER, COLOR_ATTACHMENT0, TEXTURE_2D, t, 0);
            gl.framebuffer_renderbuffer(FRAMEBUFFER, DEPTH_STENCIL_ATTACHMENT, RENDERBUFFER, d);
            let status = gl.check_framebuffer_status(FRAMEBUFFER);
            gl.bind_framebuffer(FRAMEBUFFER, self.fbo);
            self.panel_tex[k] = t;
            self.panel_depth[k] = d;
            self.panel_fbo[k] = f;
            if status != FRAMEBUFFER_COMPLETE {
                return Err(backend_err(format!("panel framebuffer incomplete: {status:#x}")));
            }
        }
        Ok(())
    }

    /// Uploads a movie frame into the three plane textures (made again when the size changes).
    fn upload_video(&mut self, f: &crate::video::VideoFrame<'_>) {
        let gl = &self.gl;
        let (cw, ch) = crate::video::VideoFrame::chroma_size(f.width, f.height);
        let sizes = [(f.width, f.height), (cw, ch), (cw, ch)];
        if self.video_size != (f.width, f.height) {
            for (t, &(w, h)) in self.video.iter_mut().zip(&sizes) {
                gl.delete_one(Object::Texture, *t);
                *t = gl.gen_one(Object::Texture);
                gl.bind_texture(TEXTURE_2D, *t);
                gl.tex_storage_2d(TEXTURE_2D, 1, R8, w as GLsizei, h as GLsizei);
                gl.tex_parameter_i(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR);
                gl.tex_parameter_i(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR);
                gl.tex_parameter_i(TEXTURE_2D, TEXTURE_WRAP_S, CLAMP_TO_EDGE);
                gl.tex_parameter_i(TEXTURE_2D, TEXTURE_WRAP_T, CLAMP_TO_EDGE);
            }
            self.video_size = (f.width, f.height);
        }
        gl.pixel_store_i(UNPACK_ALIGNMENT, 1);
        for ((&t, &(w, h)), plane) in self.video.iter().zip(&sizes).zip([f.y, f.u, f.v]) {
            gl.bind_texture(TEXTURE_2D, t);
            // SAFETY: the plane holds exactly w * h bytes (checked by VideoFrame::validate) and unpack alignment is 1.
            unsafe {
                gl.tex_sub_image_2d(
                    TEXTURE_2D,
                    0,
                    0,
                    0,
                    w as GLsizei,
                    h as GLsizei,
                    RED,
                    UNSIGNED_BYTE,
                    plane.as_ptr().cast(),
                )
            };
        }
        gl.bind_texture(TEXTURE_2D, 0);
    }

    /// Reads the render target back as RGBA8, top row first. Waits for the GPU.
    pub fn read_pixels(&self) -> Vec<u8> {
        let (w, h) = self.target_size();
        let gl = &self.gl;
        gl.bind_framebuffer(FRAMEBUFFER, self.fbo);
        gl.pixel_store_i(PACK_ALIGNMENT, 1);
        let row = w as usize * 4;
        let mut px = vec![0u8; row * h as usize];
        // SAFETY: the buffer holds exactly w * h RGBA8 pixels with pack alignment 1.
        unsafe { gl.read_pixels(0, 0, w as GLsizei, h as GLsizei, RGBA, UNSIGNED_BYTE, px.as_mut_ptr().cast()) };
        // OpenGL returns the bottom row first.
        let mut flipped = vec![0u8; px.len()];
        for (y, src) in px.chunks(row).enumerate() {
            let dst = (h as usize - 1 - y) * row;
            flipped[dst..dst + row].copy_from_slice(src);
        }
        flipped
    }

    /// Blocks until the GPU has finished all submitted work.
    pub fn wait_idle(&self) {
        self.gl.finish();
    }

    fn wait_slot(&mut self, i: usize) -> Result<(), RenderError> {
        let gl = &self.gl;
        let slot = &mut self.slots[i];
        if !slot.fence.is_null() {
            loop {
                // SAFETY: `fence` is a live sync object created by glFenceSync and not yet deleted.
                let r = unsafe { gl.client_wait_sync(slot.fence, SYNC_FLUSH_COMMANDS_BIT, 1_000_000_000) };
                match r {
                    ALREADY_SIGNALED | CONDITION_SATISFIED => break,
                    TIMEOUT_EXPIRED => continue,
                    _ => return Err(backend_err(format!("glClientWaitSync failed: {r:#x}"))),
                }
            }
            // SAFETY: as above; deleted exactly once, then cleared.
            unsafe { gl.delete_sync(slot.fence) };
            slot.fence = core::ptr::null();
        }
        if slot.query_pending {
            // The fence passed, so the query result is available without stalling.
            let mut ns = 0u64;
            // SAFETY: QUERY_RESULT writes one u64.
            unsafe { gl.get_query_object_ui64v(slot.query, QUERY_RESULT, &mut ns) };
            self.last_gpu_ns = Some(ns);
            slot.query_pending = false;
        }
        Ok(())
    }

    /// Program and fixed-function state for a batch (`STATE_*` bits).
    fn apply_state(&self, state: u32, cull: CullMode) {
        let gl = &self.gl;
        gl.use_program(self.programs[(state & STATE_ALPHA_TEST != 0) as usize]);
        let stencil = state & STATE_STENCIL_MASK;
        if state & STATE_NO_CULL != 0 || (cull == CullMode::None && state & STATE_CULL_FRONT == 0) {
            gl.disable(CULL_FACE);
        } else {
            gl.enable(CULL_FACE);
            gl.front_face(if state & STATE_FRONT_CW != 0 { CW } else { CCW });
            let front = state & STATE_CULL_FRONT != 0 || cull == CullMode::Front;
            gl.cull_face(if front { FRONT } else { BACK });
        }
        match state & STATE_DEPTH_MASK {
            STATE_DEPTH_LESS => {
                gl.enable(DEPTH_TEST);
                gl.depth_func(LESS);
            }
            STATE_DEPTH_LEQUAL => {
                gl.enable(DEPTH_TEST);
                gl.depth_func(LEQUAL);
            }
            _ => gl.disable(DEPTH_TEST),
        }
        gl.depth_mask(if state & STATE_DEPTH_WRITE != 0 { TRUE } else { FALSE });
        // Colour factors per mode; the destination alpha takes the source alpha (rendering.md 3.2).
        let factors = match state & STATE_BLEND_MASK {
            STATE_BLEND_ALPHA => Some((SRC_ALPHA, ONE_MINUS_SRC_ALPHA)),
            STATE_BLEND_ADD => Some((SRC_ALPHA, ONE)),
            STATE_BLEND_REVERSE_ALPHA => Some((ONE_MINUS_SRC_ALPHA, SRC_ALPHA)),
            STATE_BLEND_REVERSE_ADD => Some((ONE, SRC_ALPHA)),
            STATE_BLEND_DEST_ALPHA => Some((DST_ALPHA, ONE_MINUS_DST_ALPHA)),
            STATE_BLEND_INV_DEST_ALPHA => Some((ONE_MINUS_DST_ALPHA, DST_ALPHA)),
            STATE_BLEND_MULTIPLY => Some((ZERO, SRC_COLOR)),
            _ => None,
        };
        match factors {
            Some((src, dst)) => {
                gl.enable(BLEND);
                if state & STATE_BLEND_MASK == STATE_BLEND_MULTIPLY {
                    // The surface keeps its alpha.
                    gl.blend_func_separate(src, dst, ZERO, ONE);
                } else {
                    gl.blend_func_separate(src, dst, ONE, ZERO);
                }
            }
            None => gl.disable(BLEND),
        }
        let colour = if state & (STATE_NO_COLOR | STATE_ALPHA_ONLY) != 0 { FALSE } else { TRUE };
        let alpha = if state & STATE_NO_COLOR != 0 { FALSE } else { TRUE };
        gl.color_mask(colour, colour, colour, alpha);
        match stencil {
            STATE_STENCIL_COUNT => {
                // Count surfaces inside the volume: +1 where a back face is behind the scene, -1 where a front
                // face is.
                gl.enable(STENCIL_TEST);
                gl.stencil_func(ALWAYS, 0, 0xff);
                gl.stencil_op_separate(BACK, KEEP, INCR_WRAP, KEEP);
                gl.stencil_op_separate(FRONT, KEEP, DECR_WRAP, KEEP);
            }
            STATE_STENCIL_PAINT => {
                // Paint where the count is not zero, once: the painted pixels are reset.
                gl.enable(STENCIL_TEST);
                gl.stencil_func(NOTEQUAL, 0, 0xff);
                gl.stencil_op(KEEP, KEEP, ZERO);
            }
            _ => gl.disable(STENCIL_TEST),
        }
    }
}

impl Backend for GlBackend {
    fn name(&self) -> &'static str {
        "opengl"
    }

    fn clip_space(&self) -> ClipSpace {
        ClipSpace::GL
    }

    fn target_size(&self) -> (u32, u32) {
        match self.target {
            GlTarget::External { width, height, .. } | GlTarget::Offscreen { width, height } => (width, height),
        }
    }

    fn limits(&self) -> Limits {
        Limits {
            max_instances: MAX_INSTANCES,
            max_draws: MAX_DRAWS,
            max_batches: MAX_DRAWS,
            max_bones: MAX_BONES,
            max_morph_vertices: MAX_MORPH_VERTICES,
        }
    }

    fn set_clock(&mut self, clock: fn() -> u64) {
        self.clock = Some(clock);
    }

    fn upload_level(&mut self, level: &LevelUpload<'_>) -> Result<(), RenderError> {
        self.gl.finish();
        let gl = &self.gl;
        gl.bind_vertex_array(self.vao);
        let v = as_bytes::<Vertex>(level.vertices);
        gl.bind_buffer(ARRAY_BUFFER, self.arena_vbo);
        // SAFETY: pointers and sizes describe the arena slices; the driver copies them.
        unsafe { gl.buffer_data(ARRAY_BUFFER, v.len() as GLsizeiptr, v.as_ptr().cast(), STATIC_DRAW) };
        gl.bind_vertex_buffer(0, self.arena_vbo, 0, size_of::<Vertex>() as GLsizei);
        let x = as_bytes::<VertexExtra>(level.extras);
        gl.bind_buffer(ARRAY_BUFFER, self.extra_vbo);
        // SAFETY: as above.
        unsafe { gl.buffer_data(ARRAY_BUFFER, x.len() as GLsizeiptr, x.as_ptr().cast(), STATIC_DRAW) };
        gl.bind_vertex_buffer(2, self.extra_vbo, 0, size_of::<VertexExtra>() as GLsizei);
        // Weights: at least 64 bytes, so the buffer can always be bound.
        let mut w = as_bytes::<VertexWeights>(level.weights).to_vec();
        w.resize(w.len().max(64), 0);
        gl.bind_buffer(SHADER_STORAGE_BUFFER, self.weights_ssbo);
        // SAFETY: as above.
        unsafe { gl.buffer_data(SHADER_STORAGE_BUFFER, w.len() as GLsizeiptr, w.as_ptr().cast(), STATIC_DRAW) };
        gl.bind_buffer(SHADER_STORAGE_BUFFER, 0);
        self.weights_size = w.len();
        let mut o = as_bytes::<[f32; 4]>(level.outline_points).to_vec();
        o.resize(o.len().max(64), 0);
        gl.bind_buffer(SHADER_STORAGE_BUFFER, self.outline_ssbo);
        // SAFETY: as above.
        unsafe { gl.buffer_data(SHADER_STORAGE_BUFFER, o.len() as GLsizeiptr, o.as_ptr().cast(), STATIC_DRAW) };
        gl.bind_buffer(SHADER_STORAGE_BUFFER, 0);
        self.outline_size = o.len();
        let i = as_bytes::<u16>(level.indices);
        gl.bind_buffer(ELEMENT_ARRAY_BUFFER, self.arena_ibo); // recorded in the VAO
        // SAFETY: as above.
        unsafe { gl.buffer_data(ELEMENT_ARRAY_BUFFER, i.len() as GLsizeiptr, i.as_ptr().cast(), STATIC_DRAW) };
        gl.bind_vertex_array(0);

        for p in self.pools.drain(..) {
            gl.delete_one(Object::Texture, p.texture);
        }
        gl.pixel_store_i(UNPACK_ALIGNMENT, 1);
        for pool in level.pools {
            match pool.format {
                TextureFormat::Rgba8 => {}
            }
            let tex = gl.gen_one(Object::Texture);
            gl.bind_texture(TEXTURE_2D_ARRAY, tex);
            let layers = pool.layers.len() as GLsizei;
            gl.tex_storage_3d(
                TEXTURE_2D_ARRAY,
                pool.mip_levels as GLsizei,
                RGBA8,
                pool.width as GLsizei,
                pool.height as GLsizei,
                layers.max(1),
            );
            for (layer, t) in pool.layers.iter().enumerate() {
                for level in 0..t.mip_levels {
                    let (w, h) = t.level_size(level);
                    let range = t.level_range(level);
                    // SAFETY: `range` is inside `t.data` (validated when the texture was added) and holds exactly
                    // w * h RGBA8 texels; unpack alignment is 1.
                    unsafe {
                        gl.tex_sub_image_3d(
                            TEXTURE_2D_ARRAY,
                            level as GLint,
                            0,
                            0,
                            layer as GLint,
                            w as GLsizei,
                            h as GLsizei,
                            1,
                            RGBA,
                            UNSIGNED_BYTE,
                            t.data[range].as_ptr().cast(),
                        )
                    };
                }
            }
            let min = if pool.mip_levels > 1 { LINEAR_MIPMAP_LINEAR } else { LINEAR };
            gl.tex_parameter_i(TEXTURE_2D_ARRAY, TEXTURE_MIN_FILTER, min);
            gl.tex_parameter_i(TEXTURE_2D_ARRAY, TEXTURE_MAG_FILTER, LINEAR);
            let wrap = if pool.clamp { CLAMP_TO_EDGE } else { REPEAT };
            gl.tex_parameter_i(TEXTURE_2D_ARRAY, TEXTURE_WRAP_S, wrap);
            gl.tex_parameter_i(TEXTURE_2D_ARRAY, TEXTURE_WRAP_T, wrap);
            gl.tex_parameter_i(TEXTURE_2D_ARRAY, TEXTURE_MAX_LEVEL, pool.mip_levels as GLint - 1);
            self.pools.push(Pool { texture: tex });
        }
        gl.bind_texture(TEXTURE_2D_ARRAY, 0);
        self.check("upload_level")
    }

    fn submit(&mut self, frame: &FramePacket<'_>) -> Result<SubmitInfo, RenderError> {
        let si = (self.frame % FRAME_SLOTS as u64) as usize;
        let w0 = self.clock.map_or(0, |c| c());
        self.wait_slot(si)?;
        let wait_ns = self.clock.map_or(0, |c| c()).saturating_sub(w0);

        // Slot buffer layout: [uniforms][instances][bones][morphed vertices][draw commands], each at its required
        // alignment. The bone and morph regions are at least 64 bytes, so they can always be bound.
        // Uniforms: the frame's, then one block per view, each at a 256-byte boundary (the largest uniform offset
        // alignment OpenGL allows).
        let uni_size = size_of::<FrameUniforms>();
        let uni_stride = align_up(uni_size, 256);
        let uni_total = uni_stride * (1 + frame.views.len());
        let inst_off = align_up(uni_total, self.ssbo_align);
        let inst_size = core::mem::size_of_val(frame.instances);
        let bone_off = align_up(inst_off + inst_size, self.ssbo_align);
        let bone_size = core::mem::size_of_val(frame.bones).max(64);
        let morph_off = align_up(bone_off + bone_size, self.ssbo_align);
        let morph_size = core::mem::size_of_val(frame.morph).max(64);
        let draw_off = align_up(morph_off + morph_size, 16);
        let draw_size = core::mem::size_of_val(frame.draws);
        let total = draw_off + draw_size;

        self.staging.clear();
        self.staging.extend_from_slice(as_bytes(core::slice::from_ref(frame.uniforms)));
        for (i, v) in frame.views.iter().enumerate() {
            self.staging.resize(uni_stride * (1 + i), 0);
            self.staging.extend_from_slice(as_bytes(core::slice::from_ref(&v.uniforms)));
        }
        self.staging.resize(inst_off, 0);
        self.staging.extend_from_slice(as_bytes(frame.instances));
        self.staging.resize(bone_off, 0);
        self.staging.extend_from_slice(as_bytes(frame.bones));
        self.staging.resize(morph_off, 0);
        self.staging.extend_from_slice(as_bytes(frame.morph));
        self.staging.resize(draw_off, 0);
        self.staging.extend_from_slice(as_bytes(frame.draws));

        if let Some(v) = &frame.video {
            self.upload_video(v);
        }
        let gl = &self.gl;
        let (buffer, query) = (self.slots[si].buffer, self.slots[si].query);
        gl.bind_buffer(COPY_WRITE_BUFFER, buffer);
        if self.slots[si].capacity < total {
            // Grow with headroom; the old storage is orphaned by the driver.
            let cap = align_up(total + total / 2, 64 * 1024);
            self.slots[si].capacity = cap;
            // SAFETY: allocation only (null data).
            unsafe { gl.buffer_data(COPY_WRITE_BUFFER, cap as GLsizeiptr, core::ptr::null(), STREAM_DRAW) };
        }
        // The slot's fence has passed, so no GPU work reads this buffer: map without synchronisation.
        // SAFETY: the range is inside the buffer (capacity >= total).
        let dst = unsafe {
            gl.map_buffer_range(
                COPY_WRITE_BUFFER,
                0,
                total as GLsizeiptr,
                MAP_WRITE_BIT | MAP_INVALIDATE_RANGE_BIT | MAP_UNSYNCHRONIZED_BIT,
            )
        };
        if dst.is_null() {
            return Err(backend_err(String::from("glMapBufferRange returned null")));
        }
        // SAFETY: `dst` points to `total` writable bytes and `staging` holds exactly `total` bytes.
        unsafe { core::ptr::copy_nonoverlapping(self.staging.as_ptr(), dst.cast::<u8>(), total) };
        if gl.unmap_buffer(COPY_WRITE_BUFFER) == FALSE {
            return Err(backend_err(String::from("buffer contents lost while mapped")));
        }

        let (w, h) = match self.target {
            GlTarget::External { width, height, .. } | GlTarget::Offscreen { width, height } => (width, height),
        };
        gl.begin_query(TIME_ELAPSED, query);
        gl.bind_framebuffer(FRAMEBUFFER, self.fbo);
        gl.viewport(0, 0, w as GLsizei, h as GLsizei);
        gl.disable(SCISSOR_TEST);
        gl.depth_mask(TRUE);
        gl.clear_color(frame.clear[0], frame.clear[1], frame.clear[2], frame.clear[3]);
        gl.clear_depthf(1.0);
        gl.clear_stencil(0);
        gl.color_mask(TRUE, TRUE, TRUE, TRUE);
        gl.clear(COLOR_BUFFER_BIT | DEPTH_BUFFER_BIT | STENCIL_BUFFER_BIT);

        let mut api_draw_calls = 0;
        if !frame.draws.is_empty() {
            gl.enable(DEPTH_TEST);
            gl.depth_func(LESS);
            gl.bind_vertex_array(self.vao);
            gl.bind_buffer_range(UNIFORM_BUFFER, 0, buffer, 0, uni_size as GLsizeiptr);
            gl.bind_buffer_range(SHADER_STORAGE_BUFFER, 0, buffer, inst_off as GLintptr, inst_size as GLsizeiptr);
            gl.bind_buffer_range(SHADER_STORAGE_BUFFER, 1, buffer, bone_off as GLintptr, bone_size as GLsizeiptr);
            gl.bind_buffer_range(SHADER_STORAGE_BUFFER, 2, buffer, morph_off as GLintptr, morph_size as GLsizeiptr);
            gl.bind_buffer_range(SHADER_STORAGE_BUFFER, 3, self.weights_ssbo, 0, self.weights_size as GLsizeiptr);
            gl.bind_buffer_range(SHADER_STORAGE_BUFFER, 4, self.outline_ssbo, 0, self.outline_size as GLsizeiptr);
            gl.bind_buffer(DRAW_INDIRECT_BUFFER, buffer);
            gl.active_texture(TEXTURE1);
            gl.bind_texture(TEXTURE_2D, self.silhouette);
            for (k, &t) in self.video.iter().enumerate() {
                gl.active_texture(TEXTURE2 + k as GLenum);
                gl.bind_texture(TEXTURE_2D, t);
            }
            for (k, &t) in self.panel_tex.iter().enumerate() {
                gl.active_texture(TEXTURE5 + k as GLenum);
                gl.bind_texture(TEXTURE_2D, t);
            }
            gl.active_texture(TEXTURE0);

            // Without views, one view: every batch over the whole target.
            let whole = [View {
                uniforms: *frame.uniforms,
                viewport: [0, 0, w, h],
                clear_color: [0.0; 4],
                clear: 0,
                first_batch: 0,
                batch_count: frame.batches.len() as u32,
                target: VIEW_TARGET_MAIN,
            }];
            let (views, own_uniforms) = if frame.views.is_empty() { (&whole[..], false) } else { (frame.views, true) };
            let (mut cur_state, mut cur_pool) = (u32::MAX, u32::MAX);
            for (vi, view) in views.iter().enumerate() {
                let range = view.first_batch as usize..view.first_batch as usize + view.batch_count as usize;
                let batches =
                    frame.batches.get(range).ok_or_else(|| backend_err(String::from("view past the batches")))?;
                if own_uniforms {
                    let [x, y, vw, vh] = view.viewport;
                    let (fb, th) = match view.target {
                        VIEW_TARGET_SILHOUETTE => (self.silhouette_fbo, SILHOUETTE_SIZE),
                        t if t >= VIEW_TARGET_PANEL0 => {
                            (self.panel_fbo[(t - VIEW_TARGET_PANEL0) as usize & 1], PANEL_IMAGE_SIZE)
                        }
                        _ => (self.fbo, h),
                    };
                    gl.bind_framebuffer(FRAMEBUFFER, fb);
                    // GL's window origin is the bottom left.
                    let gy = th.saturating_sub(y + vh);
                    gl.bind_buffer_range(
                        UNIFORM_BUFFER,
                        0,
                        buffer,
                        (uni_stride * (1 + vi)) as GLintptr,
                        uni_size as GLsizeiptr,
                    );
                    gl.viewport(x as GLint, gy as GLint, vw as GLsizei, vh as GLsizei);
                    gl.enable(SCISSOR_TEST);
                    gl.scissor(x as GLint, gy as GLint, vw as GLsizei, vh as GLsizei);
                    if view.clear != 0 {
                        let mut bits = 0;
                        if view.clear & VIEW_CLEAR_DEPTH != 0 {
                            gl.depth_mask(TRUE);
                            bits |= DEPTH_BUFFER_BIT | STENCIL_BUFFER_BIT;
                        }
                        if view.clear & VIEW_CLEAR_COLOR != 0 {
                            let c = view.clear_color;
                            gl.color_mask(TRUE, TRUE, TRUE, TRUE);
                            gl.clear_color(c[0], c[1], c[2], c[3]);
                            bits |= COLOR_BUFFER_BIT;
                        }
                        gl.clear(bits);
                        // The masks changed: set the state again for the next batch.
                        cur_state = u32::MAX;
                    }
                }
                for b in batches {
                    if b.draw_count == 0 {
                        continue;
                    }
                    let end = b.first_draw as usize + b.draw_count as usize;
                    let tex =
                        self.pools.get(b.pool as usize).ok_or_else(|| backend_err(format!("bad pool {}", b.pool)))?;
                    if end > frame.draws.len() {
                        return Err(backend_err(String::from("batch past the end of the draw list")));
                    }
                    if b.state != cur_state {
                        self.apply_state(b.state, frame.cull);
                        cur_state = b.state;
                    }
                    if b.pool != cur_pool {
                        gl.bind_texture(TEXTURE_2D_ARRAY, tex.texture);
                        cur_pool = b.pool;
                    }
                    let offset = draw_off + b.first_draw as usize * size_of::<DrawCmd>();
                    // SAFETY: the commands at `offset` lie inside the slot buffer (checked above); each references a
                    // mesh range of the arena and instance slots below the instance count, both produced by the frame
                    // builder from committed resources.
                    unsafe {
                        gl.multi_draw_elements_indirect(
                            TRIANGLES,
                            UNSIGNED_SHORT,
                            offset as *const c_void,
                            b.draw_count as GLsizei,
                            size_of::<DrawCmd>() as GLsizei,
                        )
                    };
                    api_draw_calls += 1;
                }
            }
            gl.disable(SCISSOR_TEST);
            gl.bind_framebuffer(FRAMEBUFFER, self.fbo);
            gl.bind_vertex_array(0);
            gl.depth_mask(TRUE);
            gl.disable(BLEND);
            gl.disable(STENCIL_TEST);
            gl.color_mask(TRUE, TRUE, TRUE, TRUE);
        }
        gl.end_query(TIME_ELAPSED);
        let fence = gl.fence_sync(SYNC_GPU_COMMANDS_COMPLETE, 0);
        gl.flush();
        let slot = &mut self.slots[si];
        slot.query_pending = true;
        slot.fence = fence;
        self.frame += 1;
        self.check("submit")?;
        Ok(SubmitInfo { api_draw_calls, gpu_ns: self.last_gpu_ns, wait_ns })
    }
}

impl Drop for GlBackend {
    fn drop(&mut self) {
        let gl = &self.gl;
        gl.finish();
        for s in &self.slots {
            if !s.fence.is_null() {
                // SAFETY: live sync object, deleted once.
                unsafe { gl.delete_sync(s.fence) };
            }
            gl.delete_one(Object::Buffer, s.buffer);
            gl.delete_one(Object::Query, s.query);
        }
        for p in &self.pools {
            gl.delete_one(Object::Texture, p.texture);
        }
        for b in
            [self.arena_vbo, self.extra_vbo, self.weights_ssbo, self.outline_ssbo, self.arena_ibo, self.slot_stream]
        {
            gl.delete_one(Object::Buffer, b);
        }
        gl.delete_one(Object::VertexArray, self.vao);
        if matches!(self.target, GlTarget::Offscreen { .. }) {
            gl.delete_one(Object::Framebuffer, self.fbo);
        }
        gl.delete_one(Object::Framebuffer, self.silhouette_fbo);
        gl.delete_one(Object::Texture, self.silhouette);
        for &t in &self.video {
            gl.delete_one(Object::Texture, t);
        }
        for k in 0..2 {
            gl.delete_one(Object::Framebuffer, self.panel_fbo[k]);
            gl.delete_one(Object::Renderbuffer, self.panel_depth[k]);
            gl.delete_one(Object::Texture, self.panel_tex[k]);
        }
        gl.delete_one(Object::Renderbuffer, self.color_rb);
        gl.delete_one(Object::Renderbuffer, self.depth_rb);
        for p in self.programs {
            if p != 0 {
                gl.delete_program(p);
            }
        }
    }
}
