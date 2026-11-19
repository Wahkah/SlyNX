//! A 2D layer for user interfaces: textured and flat quads in pixels, drawn in list order over everything.
//!
//! [`UiLayer::new`] adds one quad mesh, one image (a glyph sheet, sampled with clamp-to-edge and no mipmaps), one
//! material in [`Pass::Overlay`] and a fixed number of hidden quad instances. Each frame [`UiLayer::set_quads`]
//! places as many as the list needs. All quads are one draw command: flat quads use the instance's flat-colour flag
//! instead of another texture, and the instances are in list order, which the overlay pass keeps.

use crate::backend::Backend;
use crate::math::{Mat4, Vec3};
use crate::renderer::Renderer;
use crate::types::{InstanceId, Lighting, Material, MeshData, Pass, RenderError, TextureData, Vertex};
use alloc::vec;
use alloc::vec::Vec;

/// One quad in pixels of the render target, y down from the top left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UiQuad {
    /// x, y, width, height in pixels.
    pub rect: [f32; 4],
    /// The image area `[u0, v0, u1, v1]` (top left, bottom right; row 0 of the image is v = 0).
    pub uv: [f32; 4],
    /// Sample the image (`false`: flat colour).
    pub textured: bool,
    /// RGBA 0..1, multiplied with the texel (straight alpha).
    pub color: [f32; 4],
}

/// Order of the UI's material: under the renderer's own stats text ([`crate::overlay::TextOverlay`], order 31).
pub const UI_ORDER: u8 = 30;

/// A list of screen-space quads drawn with the renderer.
pub struct UiLayer {
    instances: Vec<InstanceId>,
    shown: usize,
}

fn unorm8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

impl UiLayer {
    /// Adds the layer's mesh, image (`sheet`, RGBA8 straight alpha; `None` for flat quads only) and `max_quads`
    /// hidden instances. Call it while the level's resources are added (before [`Renderer::commit`]).
    pub fn new<B: Backend>(
        r: &mut Renderer<B>,
        sheet: Option<TextureData>,
        max_quads: usize,
    ) -> Result<UiLayer, RenderError> {
        let n = [0.0, 0.0, 1.0];
        let white = [255; 4];
        let v = [
            Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0], n, white),
            Vertex::new([1.0, 0.0, 0.0], [1.0, 0.0], n, white),
            Vertex::new([1.0, 1.0, 0.0], [1.0, 1.0], n, white),
            Vertex::new([0.0, 1.0, 0.0], [0.0, 1.0], n, white),
        ];
        let quad = r.add_mesh(&MeshData { vertices: &v, indices: &[0, 1, 2, 0, 2, 3] })?;
        let sheet = match sheet {
            Some(t) => TextureData { mip_levels: 1, ..t }.clamped(),
            None => TextureData::rgba8(1, 1, vec![255; 4])?.clamped(),
        };
        sheet.validate()?;
        let t = r.add_texture(sheet)?;
        let mat = r.add_material(Material {
            lighting: Lighting::Unlit,
            order: UI_ORDER,
            ..Material::opaque(t).with_pass(Pass::Overlay)
        })?;
        let mut instances = Vec::with_capacity(max_quads);
        for _ in 0..max_quads {
            let id = r.add_instance(quad, mat, Mat4::IDENTITY)?;
            r.set_visible(id, false)?;
            instances.push(id);
        }
        Ok(UiLayer { instances, shown: 0 })
    }

    /// Most quads shown at once.
    pub fn capacity(&self) -> usize {
        self.instances.len()
    }

    /// Quads shown by the last [`UiLayer::set_quads`].
    pub fn shown(&self) -> usize {
        self.shown
    }

    /// Shows `quads` in order (later ones over earlier ones); quads past the capacity are dropped.
    pub fn set_quads<B: Backend>(&mut self, r: &mut Renderer<B>, quads: &[UiQuad]) -> Result<(), RenderError> {
        let (w, h) = r.backend().target_size();
        let clip = r.backend().clip_space();
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        let sx = 2.0 / w;
        let sy = if clip.y_down { 2.0 / h } else { -2.0 / h };
        let y0 = if clip.y_down { -1.0 } else { 1.0 };
        let n = quads.len().min(self.instances.len());
        for (q, &id) in quads.iter().zip(&self.instances) {
            let mut m = Mat4::scale(Vec3::new(q.rect[2] * sx, q.rect[3] * sy, 1.0));
            m.m[12] = -1.0 + q.rect[0] * sx;
            m.m[13] = y0 + q.rect[1] * sy;
            r.set_transform(id, m)?;
            r.set_uv_rect(id, Some(q.uv))?;
            r.set_flat(id, !q.textured)?;
            r.set_tint(id, [unorm8(q.color[0]), unorm8(q.color[1]), unorm8(q.color[2]), unorm8(q.color[3])])?;
            r.set_visible(id, true)?;
        }
        for &id in &self.instances[n..self.shown.max(n)] {
            r.set_visible(id, false)?;
        }
        self.shown = n;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NullBackend;
    use crate::frame::{INSTANCE_FLAT, INSTANCE_SCREEN};
    use crate::renderer::Camera;

    #[test]
    fn quads_are_one_draw_in_list_order() {
        let mut r = Renderer::new(NullBackend::new(640, 480));
        let sheet = TextureData::rgba8(4, 4, vec![255; 64]).unwrap();
        let mut ui = UiLayer::new(&mut r, Some(sheet), 8).unwrap();
        r.commit().unwrap();
        let quads = [
            UiQuad { rect: [0.0, 0.0, 640.0, 480.0], uv: [0.0; 4], textured: false, color: [0.0, 0.0, 0.0, 0.5] },
            UiQuad { rect: [32.0, 48.0, 16.0, 8.0], uv: [0.25, 0.5, 0.75, 1.0], textured: true, color: [1.0; 4] },
        ];
        ui.set_quads(&mut r, &quads).unwrap();
        let cam = Camera::look_at(Vec3::new(0.0, 0.0, 5.0), Vec3::ZERO);
        let s = r.render(&cam, [0.0; 4]).unwrap();
        assert_eq!((s.visible, s.draw_commands), (2, 1));
        let recs = &r.backend().last_instances;
        assert_eq!(recs[0].flags() & (INSTANCE_FLAT | INSTANCE_SCREEN), INSTANCE_FLAT | INSTANCE_SCREEN);
        assert_eq!(recs[1].flags() & INSTANCE_FLAT, 0);
        // The textured quad: UV offset and scale; pixel (32, 48) -> clip (-1 + 0.1, 1 - 0.2).
        assert_eq!(recs[1].uv_offset, [0.25, 0.5]);
        assert_eq!((f32::from_bits(recs[1].weights_base), f32::from_bits(recs[1].layers)), (0.5, 0.5));
        assert!((recs[1].rows[0][3] + 0.9).abs() < 1e-6 && (recs[1].rows[1][3] - 0.8).abs() < 1e-6);
        assert_eq!(recs[0].tint.to_le_bytes()[3], 128);
        ui.set_quads(&mut r, &quads[..1]).unwrap();
        assert_eq!(r.render(&cam, [0.0; 4]).unwrap().visible, 1);
    }
}
