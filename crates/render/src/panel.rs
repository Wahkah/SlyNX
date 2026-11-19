//! The dialogue picture panels' frames (camera.md 7.1): each panel is 160 x 125.4 units of the 640-wide frame,
//! straight on its outer side (toward the screen edge) and a half circle of radius 62.7 on its inner side, with a
//! 3.3-unit outline and 57 horizontal bands over the picture, alternating black and mid grey at alpha 48/128, each
//! shifting sideways a little.
//!
//! The panel's scene is drawn into a panel image ([`Renderer::set_image_panels`]); [`PanelFrames`] draws the image
//! in the panel's shape with the outline and the bands, as screen-space instances at [`PANEL_ORDER`] (over the world,
//! under the movie picture, the iris and the UI).

use crate::backend::Backend;
use crate::frame::PANEL_IMAGE_SIZE;
use crate::math::{Mat4, Vec3};
use crate::renderer::{Camera, Panel, Renderer};
use crate::types::{InstanceId, Lighting, Material, MeshData, Pass, RenderError, TextureData, Vertex};
use alloc::vec;
use alloc::vec::Vec;

/// Order of the frames' material.
pub const PANEL_ORDER: u8 = 27;
/// Panel size in units of the 640-wide frame.
pub const PANEL_SIZE: [f32; 2] = [160.0, 125.4];
/// Radius of the round inner end.
pub const PANEL_RADIUS: f32 = 62.7;
/// Width of the outline.
pub const PANEL_OUTLINE: f32 = 3.3;
/// Number of bands over the picture.
pub const PANEL_BANDS: usize = 57;
/// The bands' colours: black and mid grey at alpha 48/128.
pub const BAND_COLORS: [[u8; 4]; 2] = [[0, 0, 0, 96], [128, 128, 128, 96]];
/// Largest sideways shift of a band, in panel units.
pub const BAND_SHIFT: f32 = 1.5;
const ARC_SEGMENTS: usize = 24;

/// Which side of the screen a panel is on (its straight side faces that edge).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// The outline of a left panel (straight side at x = 0, round end on the right), y down, in panel units, grown by
/// `grow` all round.
fn outline(grow: f32) -> Vec<[f32; 2]> {
    let [w, h] = PANEL_SIZE;
    let r = PANEL_RADIUS;
    let cx = w - r;
    let cy = h * 0.5;
    let mut p = vec![[-grow, -grow]];
    for k in 0..=ARC_SEGMENTS {
        let a = -core::f32::consts::FRAC_PI_2 + core::f32::consts::PI * k as f32 / ARC_SEGMENTS as f32;
        p.push([cx + (r + grow) * crate::math::cos(a), cy + (r + grow) * crate::math::sin(a)]);
    }
    p.push([-grow, h + grow]);
    p
}

/// Half the width of the left panel's shape at height `y` (from its straight side).
fn extent(y: f32) -> f32 {
    let [w, h] = PANEL_SIZE;
    let r = PANEL_RADIUS;
    let dy = y - h * 0.5;
    w - r + crate::math::sqrt((r * r - dy * dy).max(0.0))
}

/// One panel's frame: picture, outline, bands.
struct Frame {
    picture: InstanceId,
    outline: InstanceId,
    bands: Vec<InstanceId>,
}

/// The two panels' frames.
pub struct PanelFrames {
    frames: [Frame; 2],
    /// Band shifts: current, target, and a pseudo-random state.
    shift: [[f32; PANEL_BANDS]; 2],
    target: [[f32; PANEL_BANDS]; 2],
    seed: u32,
}

impl PanelFrames {
    /// Adds the meshes, a material and hidden instances for both panels. Call it while the level's resources are
    /// added (before [`Renderer::commit`]).
    pub fn new<B: Backend>(r: &mut Renderer<B>) -> Result<PanelFrames, RenderError> {
        let n = [0.0, 0.0, 1.0];
        let white = [255; 4];
        let [w, h] = PANEL_SIZE;
        // The picture: a fan over the shape, UVs across the panel.
        let shape = outline(0.0);
        let centre = [w * 0.5, h * 0.5];
        let mut v = vec![Vertex::new([centre[0], centre[1], 0.0], [centre[0] / w, centre[1] / h], n, white)];
        v.extend(shape.iter().map(|p| Vertex::new([p[0], p[1], 0.0], [p[0] / w, p[1] / h], n, white)));
        let m = shape.len() as u16;
        let mut idx = Vec::new();
        for k in 0..m {
            idx.extend_from_slice(&[0, 1 + k, 1 + (k + 1) % m]);
        }
        let picture = r.add_mesh(&MeshData { vertices: &v, indices: &idx })?;
        // The outline: a ring between the shape and the shape grown by the outline width.
        let outer = outline(PANEL_OUTLINE);
        let mut v = Vec::new();
        for (a, b) in shape.iter().zip(&outer) {
            v.push(Vertex::new([a[0], a[1], 0.0], [0.0; 2], n, white));
            v.push(Vertex::new([b[0], b[1], 0.0], [0.0; 2], n, white));
        }
        let mut idx = Vec::new();
        for k in 0..m {
            let (i, j) = (2 * k, 2 * ((k + 1) % m));
            idx.extend_from_slice(&[i, i + 1, j + 1, i, j + 1, j]);
        }
        let ring = r.add_mesh(&MeshData { vertices: &v, indices: &idx })?;
        let q = [
            Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0], n, white),
            Vertex::new([1.0, 0.0, 0.0], [1.0, 0.0], n, white),
            Vertex::new([1.0, 1.0, 0.0], [1.0, 1.0], n, white),
            Vertex::new([0.0, 1.0, 0.0], [0.0, 1.0], n, white),
        ];
        let quad = r.add_mesh(&MeshData { vertices: &q, indices: &[0, 1, 2, 0, 2, 3] })?;
        let t = r.add_texture(TextureData::rgba8(1, 1, vec![255; 4])?.clamped())?;
        let mat = r.add_material(Material {
            lighting: Lighting::Unlit,
            order: PANEL_ORDER,
            ..Material::opaque(t).with_pass(Pass::Overlay)
        })?;
        let mut make = |k: u8| -> Result<Frame, RenderError> {
            // Mesh order is draw order: the picture, the bands, then the outline.
            let picture_id = r.add_instance(picture, mat, Mat4::IDENTITY)?;
            r.set_visible(picture_id, false)?;
            r.set_panel_image(picture_id, Some(k))?;
            let outline_id = r.add_instance(ring, mat, Mat4::IDENTITY)?;
            r.set_visible(outline_id, false)?;
            r.set_flat(outline_id, true)?;
            let mut bands = Vec::with_capacity(PANEL_BANDS);
            for b in 0..PANEL_BANDS {
                let id = r.add_instance(quad, mat, Mat4::IDENTITY)?;
                r.set_visible(id, false)?;
                r.set_flat(id, true)?;
                r.set_tint(id, BAND_COLORS[b % 2])?;
                bands.push(id);
            }
            Ok(Frame { picture: picture_id, outline: outline_id, bands })
        };
        let frames = [make(0)?, make(1)?];
        Ok(PanelFrames { frames, shift: [[0.0; PANEL_BANDS]; 2], target: [[0.0; PANEL_BANDS]; 2], seed: 0x5eed_1234 })
    }

    /// The panel's rectangle in target pixels: HUD elements 26 (left) and 27 (right), 1 unit from their screen edge
    /// and 20 above the bottom (hud.md 2), `scale` target pixels per frame unit.
    pub fn rect(target: (u32, u32), side: Side, scale: f32) -> [f32; 4] {
        let (w, h) = (PANEL_SIZE[0] * scale, PANEL_SIZE[1] * scale);
        let x = match side {
            Side::Left => scale,
            Side::Right => target.0 as f32 - scale - w,
        };
        [x, target.1 as f32 - 20.0 * scale - h, w, h]
    }

    /// The panel's scene for [`Renderer::set_image_panels`]: `camera` into image `side as u8`, at `rect`'s size.
    pub fn view(side: Side, camera: Camera, rect: [f32; 4], clear: [f32; 4]) -> (Panel, u8) {
        (Panel { camera, rect, clear: Some(clear) }, side as u8)
    }

    /// Advances the bands' sideways drift by `dt` seconds: each band eases toward a target that changes now and then.
    pub fn update(&mut self, dt: f32) {
        for side in 0..2 {
            for b in 0..PANEL_BANDS {
                self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                if self.seed >> 28 == 0 {
                    let u = (self.seed >> 8) as f32 / (1u32 << 24) as f32;
                    self.target[side][b] = (u * 2.0 - 1.0) * BAND_SHIFT;
                }
                let s = &mut self.shift[side][b];
                *s += (self.target[side][b] - *s) * (dt * 6.0).min(1.0);
            }
        }
    }

    /// Shows a panel's frame in `rect` (target pixels), the picture from its image, the outline in `outline`.
    pub fn show<B: Backend>(
        &self,
        r: &mut Renderer<B>,
        side: Side,
        rect: [f32; 4],
        outline: [u8; 4],
    ) -> Result<(), RenderError> {
        let (w, h) = r.backend().target_size();
        let clip = r.backend().clip_space();
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        let sx = 2.0 / w;
        let sy = if clip.y_down { 2.0 / h } else { -2.0 / h };
        let oy = if clip.y_down { -1.0 } else { 1.0 };
        let k = rect[2] / PANEL_SIZE[0];
        // Panel units to clip space; the right panel is the left one mirrored.
        let flip = side == Side::Right;
        let place = |x: f32, y: f32, kx: f32, ky: f32| {
            let (x, kx) = if flip { (rect[0] + rect[2] - x * k, -kx) } else { (rect[0] + x * k, kx) };
            let mut m = Mat4::scale(Vec3::new(kx * k * sx, ky * k * sy, 1.0));
            m.m[12] = -1.0 + x * sx;
            m.m[13] = oy + (rect[1] + y * k) * sy;
            m
        };
        let f = &self.frames[side as usize];
        r.set_transform(f.picture, place(0.0, 0.0, 1.0, 1.0))?;
        // The image holds the picture in its top-left corner, rect-sized. OpenGL images have their first row at the
        // bottom, so there the picture's top is at v = 1.
        let s = PANEL_IMAGE_SIZE as f32;
        let (u1, v1) = ((rect[2] / s).min(1.0), (rect[3] / s).min(1.0));
        let uv = if clip.y_down { [0.0, 0.0, u1, v1] } else { [0.0, 1.0, u1, 1.0 - v1] };
        let uv = if flip { [uv[2], uv[1], uv[0], uv[3]] } else { uv };
        r.set_uv_rect(f.picture, Some(uv))?;
        r.set_visible(f.picture, true)?;
        r.set_transform(f.outline, place(0.0, 0.0, 1.0, 1.0))?;
        r.set_tint(f.outline, outline)?;
        r.set_visible(f.outline, true)?;
        let bh = PANEL_SIZE[1] / PANEL_BANDS as f32;
        for (b, &id) in f.bands.iter().enumerate() {
            let y = (b as f32 + 0.5) * bh;
            let e = extent(y);
            // The band shifted sideways, kept inside the shape.
            let s = self.shift[side as usize][b];
            let (x0, x1) = (s.max(0.0), (e + s).min(e));
            r.set_transform(id, place(x0, b as f32 * bh, x1 - x0, bh))?;
            r.set_visible(id, x1 > x0)?;
        }
        Ok(())
    }

    /// Hides a panel's frame.
    pub fn hide<B: Backend>(&self, r: &mut Renderer<B>, side: Side) -> Result<(), RenderError> {
        let f = &self.frames[side as usize];
        for &id in [f.picture, f.outline].iter().chain(&f.bands) {
            r.set_visible(id, false)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NullBackend;
    use crate::frame::{INSTANCE_PANEL0, INSTANCE_PANEL1, VIEW_TARGET_PANEL0};

    #[test]
    fn shape_and_placement() {
        // Straight part 97.3 wide, then the half circle out to 160 at mid height.
        assert!((extent(PANEL_SIZE[1] * 0.5) - 160.0).abs() < 1e-3);
        assert!((extent(0.0) - 97.3).abs() < 1e-3);
        let r = PanelFrames::rect((640, 448), Side::Right, 1.0);
        assert_eq!(r, [640.0 - 1.0 - 160.0, 448.0 - 20.0 - 125.4, 160.0, 125.4]);
    }

    #[test]
    fn panels_draw_into_images_then_frames() {
        let mut r = Renderer::new(NullBackend::new(640, 448));
        let frames = PanelFrames::new(&mut r).unwrap();
        r.commit().unwrap();
        let cam = Camera::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO);
        let left = PanelFrames::rect((640, 448), Side::Left, 1.0);
        let right = PanelFrames::rect((640, 448), Side::Right, 1.0);
        r.set_image_panels(&[
            PanelFrames::view(Side::Left, cam, left, [0.0, 0.0, 0.0, 1.0]),
            PanelFrames::view(Side::Right, cam, right, [0.0, 0.0, 0.0, 1.0]),
        ])
        .unwrap();
        frames.show(&mut r, Side::Left, left, [0, 0, 0, 255]).unwrap();
        frames.show(&mut r, Side::Right, right, [0, 0, 0, 255]).unwrap();
        let s = r.render(&cam, [0.0; 4]).unwrap();
        assert_eq!(s.visible as usize, 2 * (2 + PANEL_BANDS));
        let nb = r.backend();
        let targets: Vec<u32> = nb.last_views.iter().map(|v| v.target).collect();
        assert!(targets.contains(&VIEW_TARGET_PANEL0) && targets.contains(&(VIEW_TARGET_PANEL0 + 1)));
        let flags: Vec<u32> = nb.last_instances.iter().map(|i| i.flags()).collect();
        assert_eq!(flags.iter().filter(|f| *f & INSTANCE_PANEL0 != 0).count(), 1);
        assert_eq!(flags.iter().filter(|f| *f & INSTANCE_PANEL1 != 0).count(), 1);
        frames.hide(&mut r, Side::Left).unwrap();
        frames.hide(&mut r, Side::Right).unwrap();
        r.set_image_panels(&[]).unwrap();
        assert_eq!(r.render(&cam, [0.0; 4]).unwrap().visible, 0);
    }
}
