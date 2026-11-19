//! What moves in a level each frame besides the player character: backdrops that follow the camera, billboards
//! that face it (rendering.md 3.4, 6.1), view-angle fades (6.2) and texture animation (rendering.md 8).

use alloc::vec::Vec;

use formats::geometry::Affine;
use render::math::Vec3;
use render::{Backend, InstanceId, MaterialId, RenderError, Renderer, TextureId};

use crate::camera::FreeCamera;
use crate::level::{
    BillboardDraw, FadeDraw, GlintDraw, OutlineDraw, PreparedLevel, SkyDraw, TexAnimDesc, Uploaded, distance_fade, mat4,
};
use crate::texanim::TexAnim;

/// The moving parts of the current level.
#[derive(Default)]
pub struct LevelMotion {
    skies: Vec<SkyDraw>,
    billboards: Vec<BillboardDraw>,
    anims: Vec<(TexAnim, TexAnimDesc, Option<u32>)>,
    materials: Vec<MaterialId>,
    textures: Vec<TextureId>,
    fades: Vec<(FadeDraw, f32)>,
    outlines: Vec<(OutlineDraw, InstanceId, f32)>,
    /// View-angle fades and the alpha last set (draws that also fade by distance are left to that).
    glints: Vec<(GlintDraw, u8)>,
}

impl LevelMotion {
    pub fn new(level: &PreparedLevel, up: &Uploaded) -> LevelMotion {
        let anims = level
            .animations
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let frames = a.frames.len().max(1) as u32;
                (TexAnim::new(a.motion, frames, 0x5eed + i as u64), a.clone(), None)
            })
            .collect();
        LevelMotion {
            skies: level.skies.clone(),
            billboards: level.billboards.clone(),
            anims,
            materials: up.materials.clone(),
            textures: up.textures.clone(),
            fades: level.fades.iter().map(|f| (*f, 1.0)).collect(),
            outlines: level
                .outlines
                .iter()
                .zip(&up.outline_instances)
                .filter(|(o, _)| o.limit.is_some())
                .map(|(o, &id)| (*o, id, 1.0))
                .collect(),
            glints: level
                .extras
                .glints
                .iter()
                .filter(|g| !level.fades.iter().any(|f| f.draw == g.draw))
                .map(|g| (*g, 255))
                .collect(),
        }
    }

    /// Advances texture animation by `dt` seconds and writes the changes into the renderer's materials.
    pub fn animate<B: Backend>(&mut self, r: &mut Renderer<B>, dt: f32) -> Result<(), RenderError> {
        for (anim, desc, shown) in &mut self.anims {
            anim.advance(dt);
            let Some(&mat) = self.materials.get(desc.material as usize) else { continue };
            if desc.motion.uses_frames() {
                let f = anim.frame();
                if *shown != Some(f)
                    && let Some(&(t, p)) = desc.frames.get(f as usize)
                {
                    let tex = self.textures[t as usize];
                    let pal = p.map(|[a, b]| [self.textures[a as usize], self.textures[b as usize]]);
                    // Frames of another size land in another pool; the material then keeps its frame.
                    if r.set_material_images(
                        mat,
                        tex,
                        pal.filter(|_| r.material(mat).is_some_and(|m| m.palettes.is_some())),
                    )
                    .is_ok()
                    {
                        *shown = Some(f);
                    }
                }
            } else {
                r.set_material_uv_offset(mat, anim.uv())?;
            }
        }
        Ok(())
    }

    /// Moves backdrops with the camera, turns billboards toward it, and fades draws and outlines by distance.
    pub fn follow_camera<B: Backend>(
        &mut self,
        r: &mut Renderer<B>,
        instances: &[InstanceId],
        cam: &FreeCamera,
    ) -> Result<(), RenderError> {
        let eye = cam.pos;
        // Draw distances (rendering.md 5, 6.3): fading draws move to their translucent twin.
        for (f, shown) in &mut self.fades {
            let Some(&id) = instances.get(f.draw as usize) else { continue };
            let a = distance_fade((f.centre - eye).length(), f.limit, f.radius);
            if a == *shown {
                continue;
            }
            *shown = a;
            r.set_visible(id, a > 0.0)?;
            let m = if a < 1.0 { f.twin } else { f.material };
            r.set_material(id, self.materials[m as usize])?;
            r.set_tint(id, [255, 255, 255, (a * 255.0 + 0.5) as u8])?;
        }
        for (o, id, shown) in &mut self.outlines {
            let a = distance_fade((o.centre - eye).length(), o.limit.unwrap_or(f32::MAX), o.radius);
            if a == *shown {
                continue;
            }
            *shown = a;
            r.set_visible(*id, a > 0.0)?;
            r.set_tint(*id, [o.color[0], o.color[1], o.color[2], (o.color[3] as f32 * a + 0.5) as u8])?;
        }
        for s in &self.skies {
            let Some(&id) = instances.get(s.draw as usize) else { continue };
            let dz = if s.follow_z { eye.z - s.origin.z } else { 0.0 };
            let mut t = s.transform;
            t.t = [t.t[0] + eye.x - s.origin.x, t.t[1] + eye.y - s.origin.y, t.t[2] + dz];
            r.set_transform(id, mat4(&t))?;
        }
        let f = cam.forward();
        for (g, shown) in &mut self.glints {
            let Some(&id) = instances.get(g.draw as usize) else { continue };
            let a = glint_alpha(g, f);
            if a != *shown {
                *shown = a;
                r.set_tint(id, [255, 255, 255, a])?;
            }
        }
        if self.billboards.is_empty() {
            return Ok(());
        }
        let right = f.cross(Vec3::Z).normalize();
        let up = right.cross(f).normalize();
        for b in &self.billboards {
            let Some(&id) = instances.get(b.draw as usize) else { continue };
            r.set_transform(id, mat4(&billboard(&b.transform, b.centre, f, up)))?;
        }
        Ok(())
    }
}

/// The alpha (0..255) of a view-angle fade seen along `forward`: `clamp(c0 + x (c1 + x (c2 + x c3)), 0, 1)` with
/// `x = |direction . forward|` (rendering.md 6.2).
pub fn glint_alpha(g: &GlintDraw, forward: Vec3) -> u8 {
    (g.glint.factor(g.direction.dot(forward).abs()) * 255.0 + 0.5) as u8
}

/// A billboard's transform: rotated about its bounding-sphere centre so its local X axis points back along the
/// camera's view axis and its local Z axis along the camera's up axis, keeping its scale (rendering.md 6.1).
pub fn billboard(t: &Affine, centre: Vec3, forward: Vec3, up: Vec3) -> Affine {
    let len = |c: [f32; 3]| Vec3::new(c[0], c[1], c[2]).length();
    let (sx, sy, sz) = (len(t.cols[0]), len(t.cols[1]), len(t.cols[2]));
    let x = -forward;
    let z = up;
    let y = z.cross(x);
    let cols = [(x * sx).to_array(), (y * sy).to_array(), (z * sz).to_array()];
    let c_world = t.transform_point(centre.to_array());
    let r = Affine::new(cols, [0.0; 3]);
    let rc = r.transform_point(centre.to_array());
    Affine::new(cols, [c_world[0] - rc[0], c_world[1] - rc[1], c_world[2] - rc[2]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glints_fade_with_the_view_angle() {
        let g = GlintDraw {
            draw: 0,
            direction: Vec3::Z,
            glint: formats::geometry::Glint { direction: [0.0, 0.0, 1.0], c: [0.0, 1.0, 0.0, 0.0] },
        };
        // Looking along the direction: x = 1, full alpha; across it: x = 0, none; at 60 degrees, half.
        assert_eq!(glint_alpha(&g, -Vec3::Z), 255);
        assert_eq!(glint_alpha(&g, Vec3::X), 0);
        let f = Vec3::new(0.0, 0.866_025_4, 0.5);
        assert_eq!(glint_alpha(&g, f), 128);
    }

    #[test]
    fn billboards_face_the_camera_about_their_centre() {
        let t = Affine::new([[0.0, 2.0, 0.0], [-2.0, 0.0, 0.0], [0.0, 0.0, 2.0]], [10.0, 0.0, 0.0]);
        let centre = Vec3::new(1.0, 0.0, 0.0);
        let b = billboard(&t, centre, Vec3::Y, Vec3::Z);
        // Local X points back at the viewer (-forward), scale kept; the centre stays where it was.
        assert_eq!(b.cols[0], [0.0, -2.0, 0.0]);
        assert_eq!(b.cols[2], [0.0, 0.0, 2.0]);
        let before = t.transform_point(centre.to_array());
        let after = b.transform_point(centre.to_array());
        assert!((0..3).all(|k| (before[k] - after[k]).abs() < 1e-5), "{before:?} {after:?}");
    }
}
