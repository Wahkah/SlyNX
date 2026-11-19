//! The iris ("keyhole") wipe (level-flow.md 5): the screen mask's five coloured triangle sets drawn over the frame,
//! shrinking onto a point while the screen around them darkens to black.
//!
//! [`IrisShape::new`] adds one flat-colour mesh per set, a quad, one [`Pass::Overlay`] material at [`IRIS_ORDER`]
//! (over the world and the picture panels, under the UI layer) and hidden instances: the five sets (drawn from set 4
//! down to set 0), four covers around the shape's bounding box, and the box fill drawn last. [`IrisShape::set`]
//! places them for a closing amount `u`; [`IrisShape::hide`] hides them.
//!
//! The rule for `u` from 0 (open) to 1 (closed): every set's alpha is multiplied by `clamp(2.5 u, 0, 1)`; set 0's
//! colour is darkened toward black (RGB x (1 - u)) with its alpha moved toward 1, and the screen outside the bounding
//! box is filled with that colour; from `u = 0.9` the box is filled too, fully at `u = 1`.

use crate::backend::Backend;
use crate::math::{Mat4, Vec3};
use crate::renderer::Renderer;
use crate::types::{InstanceId, Lighting, Material, MeshData, Pass, RenderError, TextureData, Vertex};
use alloc::vec;
use alloc::vec::Vec;

/// Order of the iris material: under the UI layer ([`crate::ui2d::UI_ORDER`], 30).
pub const IRIS_ORDER: u8 = 29;

/// The iris wipe.
pub struct IrisShape {
    /// Per set (0 to 4): its instance, if the set has triangles.
    sets: [Option<InstanceId>; 5],
    colors: [[u8; 4]; 5],
    /// Top, bottom, left and right of the bounding box, then the box itself.
    covers: [InstanceId; 5],
    /// Min x, min y, max x, max y (y up).
    bounds: [f32; 4],
}

fn unorm8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// The colours of the five sets for closing amount `u` (level-flow.md 5): alphas times `clamp(2.5 u, 0, 1)`, set 0
/// darkened toward black with its alpha moved toward 1. Also returns the box fill's colour.
pub fn iris_colors(colors: &[[u8; 4]; 5], u: f32) -> ([[u8; 4]; 5], [u8; 4]) {
    let u = u.clamp(0.0, 1.0);
    let k = (2.5 * u).min(1.0);
    let mut out = [[0u8; 4]; 5];
    for (o, c) in out.iter_mut().zip(colors) {
        *o = [c[0], c[1], c[2], unorm8(c[3] as f32 / 255.0 * k)];
    }
    let c0 = colors[0];
    let a0 = c0[3] as f32 / 255.0;
    let rgb = |c: u8| unorm8(c as f32 / 255.0 * (1.0 - u));
    out[0] = [rgb(c0[0]), rgb(c0[1]), rgb(c0[2]), unorm8((a0 + (1.0 - a0) * u) * k)];
    let fill = ((u - 0.9) / 0.1).clamp(0.0, 1.0);
    let box_fill = [out[0][0], out[0][1], out[0][2], unorm8(out[0][3] as f32 / 255.0 * fill)];
    (out, box_fill)
}

impl IrisShape {
    /// Adds the shape: `points` in the mask's plane (y up), the five sets' triangles and colours (RGBA8, the
    /// shaders 1166 to 1170), and the bounds (min x, min y, max x, max y). Call it while the level's resources are
    /// added (before [`Renderer::commit`]).
    pub fn new<B: Backend>(
        r: &mut Renderer<B>,
        points: &[[f32; 2]],
        sets: [&[[u16; 3]]; 5],
        colors: [[u8; 4]; 5],
        bounds: [f32; 4],
    ) -> Result<IrisShape, RenderError> {
        let n = [0.0, 0.0, 1.0];
        let white = [255; 4];
        // Screen y runs down: the mask's y is negated.
        let verts: Vec<Vertex> = points.iter().map(|p| Vertex::new([p[0], -p[1], 0.0], [0.0; 2], n, white)).collect();
        let t = r.add_texture(TextureData::rgba8(1, 1, vec![255; 4])?.clamped())?;
        let mat = r.add_material(Material {
            lighting: Lighting::Unlit,
            order: IRIS_ORDER,
            ..Material::opaque(t).with_pass(Pass::Overlay)
        })?;
        // Meshes from set 4 down to set 0: overlay draws of one material go in mesh order.
        let mut meshes = [None; 5];
        for k in (0..5).rev() {
            let idx: Vec<u16> = sets[k].iter().flatten().copied().collect();
            if !idx.is_empty() {
                meshes[k] = Some(r.add_mesh(&MeshData { vertices: &verts, indices: &idx })?);
            }
        }
        let q = [
            Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0], n, white),
            Vertex::new([1.0, 0.0, 0.0], [1.0, 0.0], n, white),
            Vertex::new([1.0, 1.0, 0.0], [1.0, 1.0], n, white),
            Vertex::new([0.0, 1.0, 0.0], [0.0, 1.0], n, white),
        ];
        let quad = r.add_mesh(&MeshData { vertices: &q, indices: &[0, 1, 2, 0, 2, 3] })?;
        let mut set_ids = [None; 5];
        for k in (0..5).rev() {
            if let Some(m) = meshes[k] {
                let id = r.add_instance(m, mat, Mat4::IDENTITY)?;
                r.set_visible(id, false)?;
                r.set_flat(id, true)?;
                set_ids[k] = Some(id);
            }
        }
        let mut covers = [InstanceId(0); 5];
        for c in &mut covers {
            *c = r.add_instance(quad, mat, Mat4::IDENTITY)?;
            r.set_visible(*c, false)?;
            r.set_flat(*c, true)?;
        }
        Ok(IrisShape { sets: set_ids, colors, covers, bounds })
    }

    /// Shows the wipe centred on `center` (pixels from the top left), `scale` pixels per mask unit, closing amount
    /// `u` (0 open, 1 closed).
    pub fn set<B: Backend>(
        &self,
        r: &mut Renderer<B>,
        center: [f32; 2],
        scale: f32,
        u: f32,
    ) -> Result<(), RenderError> {
        let (w, h) = r.backend().target_size();
        let clip = r.backend().clip_space();
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        let sx = 2.0 / w;
        let sy = if clip.y_down { 2.0 / h } else { -2.0 / h };
        let oy = if clip.y_down { -1.0 } else { 1.0 };
        // Pixels (y down) to clip space.
        let place = |x: f32, y: f32, kx: f32, ky: f32| {
            let mut m = Mat4::scale(Vec3::new(kx * sx, ky * sy, 1.0));
            m.m[12] = -1.0 + x * sx;
            m.m[13] = oy + y * sy;
            m
        };
        let (colors, fill) = iris_colors(&self.colors, u);
        for (k, id) in self.sets.iter().enumerate() {
            if let Some(id) = *id {
                r.set_transform(id, place(center[0], center[1], scale, scale))?;
                r.set_tint(id, colors[k])?;
                r.set_visible(id, colors[k][3] > 0)?;
            }
        }
        // The bounding box on screen (y down).
        let [x0, y0, x1, y1] = self.bounds;
        let (bl, br) = (center[0] + x0 * scale, center[0] + x1 * scale);
        let (bt, bb) = (center[1] - y1 * scale, center[1] - y0 * scale);
        let rects = [
            [0.0, 0.0, w, bt.max(0.0)],
            [0.0, bb.min(h), w, (h - bb).max(0.0)],
            [0.0, bt, bl.max(0.0), bb - bt],
            [br.min(w), bt, (w - br).max(0.0), bb - bt],
            [bl, bt, br - bl, bb - bt],
        ];
        for (k, (&id, rc)) in self.covers.iter().zip(&rects).enumerate() {
            let c = if k == 4 { fill } else { colors[0] };
            let shown = rc[2] > 0.0 && rc[3] > 0.0 && c[3] > 0;
            if shown {
                r.set_transform(id, place(rc[0], rc[1], rc[2], rc[3]))?;
                r.set_tint(id, c)?;
            }
            r.set_visible(id, shown)?;
        }
        Ok(())
    }

    /// Hides the wipe.
    pub fn hide<B: Backend>(&self, r: &mut Renderer<B>) -> Result<(), RenderError> {
        for id in self.sets.iter().flatten().chain(&self.covers) {
            r.set_visible(*id, false)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NullBackend;
    use crate::frame::{INSTANCE_FLAT, INSTANCE_SCREEN};
    use crate::renderer::Camera;

    const COLORS: [[u8; 4]; 5] = [
        [0x07, 0x1e, 0x98, 0xbf],
        [0x00, 0x51, 0xde, 0x7f],
        [0x00, 0x51, 0xde, 0x7f],
        [0xff, 0xff, 0xff, 0x7f],
        [0xff, 0xff, 0xff, 0x7f],
    ];

    #[test]
    fn colours_follow_the_closing_amount() {
        let (c, fill) = iris_colors(&COLORS, 0.0);
        assert!(c.iter().all(|c| c[3] == 0) && fill[3] == 0, "open: nothing shows");
        let (c, fill) = iris_colors(&COLORS, 0.4);
        // Alpha x 1 from u = 0.4; set 0 darkened by 0.4, its alpha 0.75 moved 40 % toward 1.
        assert_eq!(c[3], COLORS[3]);
        assert_eq!(&c[0][..3], &[4, 18, 91]);
        assert_eq!(c[0][3], 217);
        assert_eq!(fill[3], 0);
        let (c, fill) = iris_colors(&COLORS, 1.0);
        assert_eq!(c[0], [0, 0, 0, 255]);
        assert_eq!(fill, [0, 0, 0, 255], "closed: the box is black too");
    }

    #[test]
    fn sets_then_covers_in_one_draw() {
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let points = [[-35.0, -35.0], [35.0, -35.0], [35.0, 35.0], [-35.0, 35.0]];
        let tri: &[[u16; 3]] = &[[0, 1, 2], [0, 2, 3]];
        let iris =
            IrisShape::new(&mut r, &points, [tri, tri, &[], tri, &[]], COLORS, [-35.0, -35.0, 35.0, 35.0]).unwrap();
        r.commit().unwrap();
        let cam = Camera::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO);
        assert_eq!(r.render(&cam, [0.0; 4]).unwrap().visible, 0);
        // Half closed in the middle: three sets and four covers.
        iris.set(&mut r, [320.0, 240.0], 2.0, 0.5).unwrap();
        let s = r.render(&cam, [0.0; 4]).unwrap();
        assert_eq!(s.visible, 7);
        let recs = &r.backend().last_instances;
        assert!(recs.iter().all(|i| i.flags() & (INSTANCE_FLAT | INSTANCE_SCREEN) == INSTANCE_FLAT | INSTANCE_SCREEN));
        // Set 3 first (drawn from the highest set down), the covers last.
        assert_eq!(recs[0].tint.to_le_bytes(), [0xff, 0xff, 0xff, 0x7f]);
        // Closed: the box fill too.
        iris.set(&mut r, [320.0, 240.0], 0.1, 1.0).unwrap();
        assert_eq!(r.render(&cam, [0.0; 4]).unwrap().visible, 8);
        iris.hide(&mut r).unwrap();
        assert_eq!(r.render(&cam, [0.0; 4]).unwrap().visible, 0);
    }
}
