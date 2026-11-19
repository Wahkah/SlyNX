//! Debug draw on OpenGL: the same one-pipeline triangle renderer as the Switch's, for demos and bring-up tests.

use crate::context::GlShared;
use crate::gl::{self, Gl};
use platform_api::{DebugDraw, Vertex};
use std::ffi::{CString, c_char};
use std::rc::Rc;

const VERTEX_SHADER: &str = "#version 430 core
layout(location = 0) in vec2 inPos;
layout(location = 1) in vec4 inColor;
out vec4 vColor;
void main() {
    // The platform's y axis points down (-1 is the top of the screen), GL's points up.
    gl_Position = vec4(inPos.x, -inPos.y, 0.0, 1.0);
    vColor = inColor;
}
";

const FRAGMENT_SHADER: &str = "#version 430 core
in vec4 vColor;
out vec4 outColor;
void main() {
    outColor = vColor;
}
";

/// The debug draw. Holds the GL context alive; create it after the platform with [`crate::PcPlatform::debug_draw`].
pub struct PcDebugDraw {
    shared: Rc<GlShared>,
    program: u32,
    vao: u32,
    vbo: u32,
    max_vertices: usize,
}

fn compile(gl: &Gl, kind: u32, src: &str) -> Result<u32, String> {
    let src = CString::new(src).unwrap();
    // SAFETY: the context is current; `src` is NUL-terminated and outlives the call; the log buffer is sized.
    unsafe {
        let shader = (gl.CreateShader)(kind);
        let ptr: *const c_char = src.as_ptr();
        (gl.ShaderSource)(shader, 1, &ptr, core::ptr::null());
        (gl.CompileShader)(shader);
        let mut ok = 0;
        (gl.GetShaderiv)(shader, gl::COMPILE_STATUS, &mut ok);
        if ok == 0 {
            let mut log = vec![0u8; 1024];
            let mut len = 0;
            (gl.GetShaderInfoLog)(shader, log.len() as i32, &mut len, log.as_mut_ptr().cast());
            (gl.DeleteShader)(shader);
            return Err(format!("shader: {}", String::from_utf8_lossy(&log[..len.max(0) as usize])));
        }
        Ok(shader)
    }
}

impl PcDebugDraw {
    pub(crate) fn new(shared: Rc<GlShared>, max_vertices: usize) -> Result<PcDebugDraw, String> {
        let gl = shared.gl;
        let vs = compile(&gl, gl::VERTEX_SHADER, VERTEX_SHADER)?;
        let fs = match compile(&gl, gl::FRAGMENT_SHADER, FRAGMENT_SHADER) {
            Ok(fs) => fs,
            Err(e) => {
                // SAFETY: `vs` is a live shader name.
                unsafe { (gl.DeleteShader)(vs) };
                return Err(e);
            }
        };
        // SAFETY: the context is current; names are created here and owned by the returned value.
        unsafe {
            let program = (gl.CreateProgram)();
            (gl.AttachShader)(program, vs);
            (gl.AttachShader)(program, fs);
            (gl.LinkProgram)(program);
            (gl.DeleteShader)(vs);
            (gl.DeleteShader)(fs);
            let mut ok = 0;
            (gl.GetProgramiv)(program, gl::LINK_STATUS, &mut ok);
            if ok == 0 {
                let mut log = vec![0u8; 1024];
                let mut len = 0;
                (gl.GetProgramInfoLog)(program, log.len() as i32, &mut len, log.as_mut_ptr().cast());
                (gl.DeleteProgram)(program);
                return Err(format!("link: {}", String::from_utf8_lossy(&log[..len.max(0) as usize])));
            }
            let (mut vao, mut vbo) = (0, 0);
            (gl.GenVertexArrays)(1, &mut vao);
            (gl.GenBuffers)(1, &mut vbo);
            (gl.BindVertexArray)(vao);
            (gl.BindBuffer)(gl::ARRAY_BUFFER, vbo);
            let stride = size_of::<Vertex>() as i32;
            (gl.VertexAttribPointer)(0, 2, gl::FLOAT, 0, stride, core::ptr::null());
            (gl.VertexAttribPointer)(1, 4, gl::FLOAT, 0, stride, 8usize as *const _);
            (gl.EnableVertexAttribArray)(0);
            (gl.EnableVertexAttribArray)(1);
            (gl.BindVertexArray)(0);
            Ok(PcDebugDraw { shared, program, vao, vbo, max_vertices })
        }
    }

    /// Draws without presenting, so the frame can be read back with `PcPlatform::read_pixels` first.
    pub fn draw(&mut self, clear: [f32; 4], vertices: &[Vertex]) {
        let gl = &self.shared.gl;
        let n = vertices.len().min(self.max_vertices);
        let n = n - n % 3;
        let (w, h) = self.shared.size();
        // SAFETY: the context is current; the buffer upload reads `n` live vertices.
        unsafe {
            (gl.BindFramebuffer)(gl::FRAMEBUFFER, self.shared.default_framebuffer());
            (gl.Viewport)(0, 0, w as i32, h as i32);
            for cap in [gl::SCISSOR_TEST, gl::DEPTH_TEST, gl::CULL_FACE, gl::BLEND, gl::FRAMEBUFFER_SRGB] {
                (gl.Disable)(cap);
            }
            (gl.ClearColor)(clear[0], clear[1], clear[2], clear[3]);
            (gl.Clear)(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
            if n > 0 {
                (gl.UseProgram)(self.program);
                (gl.BindVertexArray)(self.vao);
                (gl.BindBuffer)(gl::ARRAY_BUFFER, self.vbo);
                (gl.BufferData)(
                    gl::ARRAY_BUFFER,
                    (n * size_of::<Vertex>()) as isize,
                    vertices.as_ptr().cast(),
                    gl::STREAM_DRAW,
                );
                (gl.DrawArrays)(gl::TRIANGLES, 0, n as i32);
                (gl.BindVertexArray)(0);
                (gl.UseProgram)(0);
            }
        }
    }
}

impl DebugDraw for PcDebugDraw {
    type Error = String;

    fn max_vertices(&self) -> usize {
        self.max_vertices
    }

    fn frame(&mut self, clear: [f32; 4], vertices: &[Vertex]) -> Result<(), String> {
        self.draw(clear, vertices);
        self.shared.present();
        Ok(())
    }
}

impl Drop for PcDebugDraw {
    fn drop(&mut self) {
        let gl = &self.shared.gl;
        // SAFETY: the context is alive (we hold it) and current; the names are ours.
        unsafe {
            (gl.DeleteBuffers)(1, &self.vbo);
            (gl.DeleteVertexArrays)(1, &self.vao);
            (gl.DeleteProgram)(self.program);
        }
    }
}
