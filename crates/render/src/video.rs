//! Movie frames: a 4:2:0 YUV picture uploaded once per frame and drawn as one screen-space quad, converted to RGB in
//! the fragment shader.
//!
//! [`Renderer::set_video_frame`] copies the three planes; the next [`Renderer::render`] uploads them (OpenGL:
//! `glTexSubImage2D` into three single-channel textures; deko3d: a copy into the frame slot's staging memory and a
//! GPU image copy, about 1.5 bytes per pixel). [`VideoLayer`] is the quad: one hidden screen-space instance whose
//! fragment shader reads the planes (instance flag [`crate::frame::INSTANCE_VIDEO`]).
//!
//! Conversion: BT.601 with video range (Y 16..235, Cb and Cr 16..240), as MPEG video carries it.

use crate::backend::Backend;
use crate::math::{Mat4, Vec3};
use crate::renderer::Renderer;
use crate::types::{InstanceId, Lighting, Material, MeshData, Pass, RenderError, TextureData, Vertex};
use alloc::vec;

/// Order of the video material: over the world, under the iris (29) and the UI (30).
pub const VIDEO_ORDER: u8 = 28;
/// Largest picture, in pixels each way.
pub const MAX_VIDEO_SIZE: u32 = 2048;

/// One frame's planes: Y is `width` x `height`, U (Cb) and V (Cr) are `(width + 1) / 2` x `(height + 1) / 2`, rows
/// top first, no padding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoFrame<'a> {
    pub width: u32,
    pub height: u32,
    pub y: &'a [u8],
    pub u: &'a [u8],
    pub v: &'a [u8],
}

impl VideoFrame<'_> {
    /// Width and height of the chroma planes.
    pub fn chroma_size(width: u32, height: u32) -> (u32, u32) {
        (width.div_ceil(2), height.div_ceil(2))
    }

    /// Checks the sizes.
    pub fn validate(&self) -> Result<(), RenderError> {
        let (cw, ch) = Self::chroma_size(self.width, self.height);
        if self.width == 0
            || self.height == 0
            || self.width > MAX_VIDEO_SIZE
            || self.height > MAX_VIDEO_SIZE
            || self.y.len() != (self.width * self.height) as usize
            || self.u.len() != (cw * ch) as usize
            || self.v.len() != (cw * ch) as usize
        {
            return Err(RenderError::BadTexture("video planes do not match the picture size"));
        }
        Ok(())
    }
}

/// The video quad.
pub struct VideoLayer {
    instance: InstanceId,
}

impl VideoLayer {
    /// Adds the quad mesh, a material in [`Pass::Overlay`] at [`VIDEO_ORDER`] and one hidden instance. Call it while
    /// the level's resources are added (before [`Renderer::commit`]).
    pub fn new<B: Backend>(r: &mut Renderer<B>) -> Result<VideoLayer, RenderError> {
        let n = [0.0, 0.0, 1.0];
        let white = [255; 4];
        let v = [
            Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0], n, white),
            Vertex::new([1.0, 0.0, 0.0], [1.0, 0.0], n, white),
            Vertex::new([1.0, 1.0, 0.0], [1.0, 1.0], n, white),
            Vertex::new([0.0, 1.0, 0.0], [0.0, 1.0], n, white),
        ];
        let quad = r.add_mesh(&MeshData { vertices: &v, indices: &[0, 1, 2, 0, 2, 3] })?;
        let t = r.add_texture(TextureData::rgba8(1, 1, vec![255; 4])?.clamped())?;
        let mat = r.add_material(Material {
            lighting: Lighting::Unlit,
            order: VIDEO_ORDER,
            ..Material::opaque(t).with_pass(Pass::Overlay)
        })?;
        let instance = r.add_instance(quad, mat, Mat4::IDENTITY)?;
        r.set_visible(instance, false)?;
        r.set_video_quad(instance, true)?;
        Ok(VideoLayer { instance })
    }

    /// Shows the latest frame in `rect` (x, y, width, height in target pixels, y down), times `tint` (RGBA 0..1;
    /// alpha fades the picture).
    pub fn show<B: Backend>(&self, r: &mut Renderer<B>, rect: [f32; 4], tint: [f32; 4]) -> Result<(), RenderError> {
        let (w, h) = r.backend().target_size();
        let clip = r.backend().clip_space();
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        let sx = 2.0 / w;
        let sy = if clip.y_down { 2.0 / h } else { -2.0 / h };
        let mut m = Mat4::scale(Vec3::new(rect[2] * sx, rect[3] * sy, 1.0));
        m.m[12] = -1.0 + rect[0] * sx;
        m.m[13] = if clip.y_down { -1.0 } else { 1.0 } + rect[1] * sy;
        r.set_transform(self.instance, m)?;
        r.set_tint(self.instance, tint.map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8))?;
        r.set_visible(self.instance, true)
    }

    /// The largest rectangle of the picture's aspect centred in the target (letterboxed or pillarboxed).
    pub fn fit(target: (u32, u32), picture: (u32, u32)) -> [f32; 4] {
        let (tw, th) = (target.0 as f32, target.1 as f32);
        let a = picture.0.max(1) as f32 / picture.1.max(1) as f32;
        let (w, h) = if tw / th > a { (th * a, th) } else { (tw, tw / a) };
        [(tw - w) * 0.5, (th - h) * 0.5, w, h]
    }

    pub fn hide<B: Backend>(&self, r: &mut Renderer<B>) -> Result<(), RenderError> {
        r.set_visible(self.instance, false)
    }
}

/// The RGB of a YUV sample as the shader converts it (BT.601, video range), for tests and CPU fallbacks.
pub fn yuv_to_rgb(y: u8, u: u8, v: u8) -> [u8; 3] {
    let y = (y as f32 - 16.0) * 1.164_383;
    let (u, v) = (u as f32 - 128.0, v as f32 - 128.0);
    let c = |x: f32| (x.clamp(0.0, 255.0) + 0.5) as u8;
    [c(y + 1.596_027 * v), c(y - 0.391_762 * u - 0.812_968 * v), c(y + 2.017_232 * u)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NullBackend;
    use crate::renderer::Camera;

    #[test]
    fn frames_upload_once_and_draw_one_quad() {
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let layer = VideoLayer::new(&mut r).unwrap();
        r.commit().unwrap();
        let (y, u, v) = (vec![128u8; 4 * 2], vec![128u8; 2], vec![128u8; 2]);
        assert!(r.set_video_frame(Some(VideoFrame { width: 4, height: 3, y: &y, u: &u, v: &v })).is_err());
        r.set_video_frame(Some(VideoFrame { width: 4, height: 2, y: &y, u: &u, v: &v })).unwrap();
        layer.show(&mut r, VideoLayer::fit((640, 480), (4, 2)), [1.0; 4]).unwrap();
        let cam = Camera::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO);
        let s = r.render(&cam, [0.0; 4]).unwrap();
        assert_eq!(s.visible, 1);
        assert_eq!(r.backend().last_video, Some((4, 2)));
        assert_ne!(r.backend().last_instances[0].flags() & crate::frame::INSTANCE_VIDEO, 0);
        // Uploaded once: the next frame has nothing new.
        r.render(&cam, [0.0; 4]).unwrap();
        assert_eq!(r.backend().last_video, None);
        assert_eq!(VideoLayer::fit((640, 480), (4, 2)), [0.0, 80.0, 640.0, 320.0]);
    }

    #[test]
    fn conversion() {
        assert_eq!(yuv_to_rgb(16, 128, 128), [0, 0, 0]);
        assert_eq!(yuv_to_rgb(235, 128, 128), [255, 255, 255]);
        let red = yuv_to_rgb(81, 90, 240);
        assert!(red[0] > 250 && red[1] < 5 && red[2] < 5, "{red:?}");
    }
}
