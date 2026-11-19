//! Decoded frames (Y, Cb and Cr planes, 4:2:0) and their conversion to RGB.

use alloc::vec;
use alloc::vec::Vec;

use crate::headers::PictureType;

/// Facts about a decoded frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameInfo {
    /// Position in display order, counting from 0 at the start of the stream (skipped pictures count too).
    pub display_index: u64,
    /// Coding type of the frame (of its first field for field pictures).
    pub picture_type: PictureType,
    pub temporal_reference: u16,
    pub top_field_first: bool,
    pub progressive_frame: bool,
    pub repeat_first_field: bool,
    /// Whether the frame was coded as two field pictures.
    pub field_pictures: bool,
}

/// A decoded 4:2:0 frame. Luma is `stride` x `coded_height` bytes, each chroma plane `chroma_stride` x
/// `coded_height / 2`; the visible picture is the top-left `width` x `height` of luma.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub stride: usize,
    pub coded_height: usize,
    pub chroma_stride: usize,
    pub y: Vec<u8>,
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
    pub info: FrameInfo,
    /// matrix_coefficients of the sequence (5 or 6: BT.601; 1: BT.709).
    pub matrix_coefficients: u8,
}

/// Borrowed planes of a frame, for uploading as three single-channel textures (luma at full size, chroma at half
/// width and half height) and converting on the GPU with [`ColorMatrix`].
#[derive(Clone, Copy, Debug)]
pub struct Planes<'a> {
    pub width: usize,
    pub height: usize,
    pub y: &'a [u8],
    pub y_stride: usize,
    pub cb: &'a [u8],
    pub cr: &'a [u8],
    pub chroma_stride: usize,
}

impl Frame {
    /// A mid-grey frame with the given coded size (multiples of 16).
    pub fn new(width: usize, height: usize, coded_width: usize, coded_height: usize) -> Frame {
        let cw = coded_width / 2;
        Frame {
            width,
            height,
            stride: coded_width,
            coded_height,
            chroma_stride: cw,
            y: vec![128; coded_width * coded_height],
            cb: vec![128; cw * (coded_height / 2)],
            cr: vec![128; cw * (coded_height / 2)],
            info: FrameInfo::default(),
            matrix_coefficients: 5,
        }
    }

    /// The planes for a GPU upload.
    pub fn planes(&self) -> Planes<'_> {
        Planes {
            width: self.width,
            height: self.height,
            y: &self.y,
            y_stride: self.stride,
            cb: &self.cb,
            cr: &self.cr,
            chroma_stride: self.chroma_stride,
        }
    }

    /// The colour matrix for this frame's sequence.
    pub fn color_matrix(&self) -> ColorMatrix {
        ColorMatrix::for_matrix_coefficients(self.matrix_coefficients)
    }

    /// Copies another frame's contents into this one, reusing the allocation.
    pub fn copy_from(&mut self, other: &Frame) {
        self.width = other.width;
        self.height = other.height;
        self.stride = other.stride;
        self.coded_height = other.coded_height;
        self.chroma_stride = other.chroma_stride;
        self.info = other.info;
        self.matrix_coefficients = other.matrix_coefficients;
        copy_vec(&mut self.y, &other.y);
        copy_vec(&mut self.cb, &other.cb);
        copy_vec(&mut self.cr, &other.cr);
    }

    /// Converts to RGBA8 (alpha 255), `width * height * 4` bytes, top row first. Chroma is upsampled by
    /// repetition. `out` must hold at least `width * height * 4` bytes; the function returns `false` otherwise.
    pub fn to_rgba(&self, out: &mut [u8]) -> bool {
        let need = self.width * self.height * 4;
        if out.len() < need {
            return false;
        }
        let m = IntMatrix::for_matrix_coefficients(self.matrix_coefficients);
        for (row, dst) in out[..need].chunks_exact_mut(self.width * 4).enumerate() {
            let yrow = &self.y[row * self.stride..][..self.width];
            let crow = (row / 2) * self.chroma_stride;
            let cbrow = &self.cb[crow..][..self.width.div_ceil(2)];
            let crrow = &self.cr[crow..][..self.width.div_ceil(2)];
            convert_row(&m, yrow, cbrow, crrow, dst);
        }
        true
    }

    /// RGBA8 in a new vector.
    pub fn to_rgba_vec(&self) -> Vec<u8> {
        let mut v = vec![0u8; self.width * self.height * 4];
        self.to_rgba(&mut v);
        v
    }
}

fn copy_vec(dst: &mut Vec<u8>, src: &[u8]) {
    dst.clear();
    dst.extend_from_slice(src);
}

/// Integer conversion coefficients (8 fractional bits), studio range in, full range out.
struct IntMatrix {
    y: i32,
    r_cr: i32,
    g_cb: i32,
    g_cr: i32,
    b_cb: i32,
}

impl IntMatrix {
    fn for_matrix_coefficients(mc: u8) -> IntMatrix {
        if mc == 1 {
            // ITU-R BT.709
            IntMatrix { y: 298, r_cr: 459, g_cb: 55, g_cr: 136, b_cb: 541 }
        } else {
            // ITU-R BT.601 (SMPTE 170M), the default for these streams
            IntMatrix { y: 298, r_cr: 409, g_cb: 100, g_cr: 208, b_cb: 516 }
        }
    }
}

#[inline(always)]
fn convert_row(m: &IntMatrix, y: &[u8], cb: &[u8], cr: &[u8], dst: &mut [u8]) {
    for (i, (px, &yv)) in dst.as_chunks_mut::<4>().0.iter_mut().zip(y).enumerate() {
        let c = (i32::from(yv) - 16) * m.y + 128;
        let d = i32::from(cb[i / 2]) - 128;
        let e = i32::from(cr[i / 2]) - 128;
        px[0] = ((c + m.r_cr * e) >> 8).clamp(0, 255) as u8;
        px[1] = ((c - m.g_cb * d - m.g_cr * e) >> 8).clamp(0, 255) as u8;
        px[2] = ((c + m.b_cb * d) >> 8).clamp(0, 255) as u8;
        px[3] = 255;
    }
}

/// YCbCr to RGB conversion for a shader: `rgb = rows * (y, cb, cr, 1)` with each sample normalised to 0..1 (an
/// 8-bit texture sampled as UNORM).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorMatrix {
    /// Three rows of a 3 x 4 matrix (column-major as `mat3x4` or row vectors as three `vec4` uniforms).
    pub rows: [[f32; 4]; 3],
}

impl ColorMatrix {
    /// ITU-R BT.601 studio range (the MPEG-2 default for standard definition).
    pub fn bt601() -> ColorMatrix {
        Self::build(0.299, 0.114)
    }

    /// ITU-R BT.709 studio range.
    pub fn bt709() -> ColorMatrix {
        Self::build(0.2126, 0.0722)
    }

    /// The matrix for a sequence's `matrix_coefficients` (1 = BT.709; anything else is treated as BT.601).
    pub fn for_matrix_coefficients(mc: u8) -> ColorMatrix {
        if mc == 1 { Self::bt709() } else { Self::bt601() }
    }

    fn build(kr: f32, kb: f32) -> ColorMatrix {
        let kg = 1.0 - kr - kb;
        let ys = 255.0 / 219.0;
        let cs = 255.0 / 224.0;
        let r_cr = 2.0 * (1.0 - kr) * cs;
        let b_cb = 2.0 * (1.0 - kb) * cs;
        let g_cb = -b_cb * kb / kg;
        let g_cr = -r_cr * kr / kg;
        let y0 = 16.0 / 255.0;
        let c0 = 128.0 / 255.0;
        ColorMatrix {
            rows: [
                [ys, 0.0, r_cr, -ys * y0 - r_cr * c0],
                [ys, g_cb, g_cr, -ys * y0 - (g_cb + g_cr) * c0],
                [ys, b_cb, 0.0, -ys * y0 - b_cb * c0],
            ],
        }
    }

    /// Applies the matrix to 8-bit samples (for checking a shader against [`Frame::to_rgba`]).
    pub fn apply(&self, y: u8, cb: u8, cr: u8) -> [f32; 3] {
        let v = [f32::from(y) / 255.0, f32::from(cb) / 255.0, f32::from(cr) / 255.0, 1.0];
        let dot = |r: &[f32; 4]| r[0] * v[0] + r[1] * v[1] + r[2] * v[2] + r[3];
        [dot(&self.rows[0]), dot(&self.rows[1]), dot(&self.rows[2])]
    }
}

/// A GLSL fragment-shader body for the GPU path: three `R8` textures and the matrix rows as uniforms.
pub const GLSL_YUV_TO_RGB: &str = "\
uniform sampler2D u_y;
uniform sampler2D u_cb;
uniform sampler2D u_cr;
uniform vec4 u_rows[3]; // ColorMatrix::rows
vec3 movie_rgb(vec2 uv) {
    vec4 s = vec4(texture(u_y, uv).r, texture(u_cb, uv).r, texture(u_cr, uv).r, 1.0);
    return clamp(vec3(dot(u_rows[0], s), dot(u_rows[1], s), dot(u_rows[2], s)), 0.0, 1.0);
}
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grey_and_primaries() {
        let mut f = Frame::new(2, 2, 16, 16);
        f.y.fill(16);
        let rgba = f.to_rgba_vec();
        assert_eq!(&rgba[..4], &[0, 0, 0, 255]);
        f.y.fill(235);
        assert_eq!(&f.to_rgba_vec()[..4], &[255, 255, 255, 255]);
        // Pure red in BT.601 studio range: Y 81, Cb 90, Cr 240.
        f.y.fill(81);
        f.cb.fill(90);
        f.cr.fill(240);
        let px = &f.to_rgba_vec()[..3];
        assert!(px[0] >= 253 && px[1] <= 2 && px[2] <= 2, "{px:?}");
    }

    #[test]
    fn float_matrix_matches_integer_path() {
        for mc in [1u8, 5] {
            let m = ColorMatrix::for_matrix_coefficients(mc);
            let im = IntMatrix::for_matrix_coefficients(mc);
            for &(y, cb, cr) in &[(16u8, 128u8, 128u8), (235, 128, 128), (81, 90, 240), (145, 54, 34), (100, 200, 60)] {
                let f = m.apply(y, cb, cr);
                let mut px = [0u8; 4];
                convert_row(&im, &[y], &[cb], &[cr], &mut px);
                for c in 0..3 {
                    let fv = (f[c] * 255.0).clamp(0.0, 255.0);
                    assert!((fv - f32::from(px[c])).abs() <= 2.0, "mc {mc} {y} {cb} {cr} channel {c}");
                }
            }
        }
    }

    #[test]
    fn small_output_refused() {
        let f = Frame::new(16, 16, 16, 16);
        let mut out = [0u8; 10];
        assert!(!f.to_rgba(&mut out));
    }
}
