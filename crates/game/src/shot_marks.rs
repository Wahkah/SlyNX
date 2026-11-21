//! Placeholder streaks for the machines' projectiles (`engine::behavior::vehicles::shots`): the specs name no
//! projectile mesh or effect the port can draw yet (the pool and gun templates are scripts' or untraced), so each
//! shot is drawn as a thin unlit box stretched along its velocity. A Sly 2 or Sly 3 session makes the mesh while its
//! level uploads ([`ShotMarks::upload`], before the commit) and places the streaks every frame
//! ([`ShotMarks::draw`]); instances are pooled and hidden when not needed.

use alloc::vec::Vec;

use engine::world::World;
use render::math::{Mat4, Vec3};
use render::{Backend, InstanceId, Material, MaterialId, MeshData, MeshId, RenderError, Renderer, TextureData};

/// A streak's length per unit of speed (seconds of flight shown) and its limits, and its thickness.
pub const TRAIL_SECONDS: f32 = 0.02;
pub const LENGTH: [f32; 2] = [40.0, 400.0];
pub const WIDTH: f32 = 10.0;
/// The streak's colour.
pub const COLOR: [u8; 4] = [255, 220, 120, 255];
/// Most streaks drawn (and instances kept) at once; shots beyond it are not drawn.
pub const MAX_STREAKS: usize = 256;

/// The streaks' mesh, material and instance pool.
pub struct ShotMarks {
    mesh: MeshId,
    material: MaterialId,
    pool: Vec<InstanceId>,
    /// Streaks shown last frame.
    pub shown: usize,
}

/// A unit box from −0.5 to 0.5 on each axis.
fn unit_box() -> (Vec<render::Vertex>, Vec<u16>) {
    let mut v = Vec::with_capacity(8);
    for i in 0..8u8 {
        let c = |b: u8| if i & b != 0 { 0.5 } else { -0.5 };
        let p = [c(1), c(2), c(4)];
        v.push(render::Vertex::new(p, [0.5, 0.5], [0.0, 0.0, 1.0], [255; 4]));
    }
    // Two triangles per face, outward winding.
    let faces: [[u16; 4]; 6] = [[0, 2, 6, 4], [1, 5, 7, 3], [0, 4, 5, 1], [2, 3, 7, 6], [0, 1, 3, 2], [4, 6, 7, 5]];
    let mut idx = Vec::with_capacity(36);
    for [a, b, c, d] in faces {
        idx.extend_from_slice(&[a, b, c, a, c, d]);
    }
    (v, idx)
}

/// The transform drawing a streak for a shot at `p` moving at `v`.
pub fn streak(p: [f32; 3], v: [f32; 3]) -> Mat4 {
    let speed = engine::math::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
    let dir = if speed > 1e-3 { [v[0] / speed, v[1] / speed, v[2] / speed] } else { [1.0, 0.0, 0.0] };
    let len = (speed * TRAIL_SECONDS).clamp(LENGTH[0], LENGTH[1]);
    // Two axes across the direction.
    let up = if dir[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let cross =
        |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let norm = |a: [f32; 3]| {
        let l = engine::math::sqrt(a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).max(1e-6);
        [a[0] / l, a[1] / l, a[2] / l]
    };
    let side = norm(cross(up, dir));
    let lift = cross(dir, side);
    // Centred half a streak behind the shot: the head is the shot itself.
    let c = [p[0] - dir[0] * len * 0.5, p[1] - dir[1] * len * 0.5, p[2] - dir[2] * len * 0.5];
    let mut m = Mat4::IDENTITY;
    for (col, (axis, s)) in [(dir, len), (side, WIDTH), (lift, WIDTH)].into_iter().enumerate() {
        for row in 0..3 {
            m.m[col * 4 + row] = axis[row] * s;
        }
    }
    m.m[12] = c[0];
    m.m[13] = c[1];
    m.m[14] = c[2];
    m
}

impl ShotMarks {
    /// Makes the streak's mesh, texture and material; call while the level uploads (before the commit).
    pub fn upload<B: Backend>(r: &mut Renderer<B>) -> Result<ShotMarks, RenderError> {
        let (v, i) = unit_box();
        let mesh = r.add_mesh(&MeshData { vertices: &v, indices: &i })?;
        let texture = r.add_texture(TextureData {
            width: 1,
            height: 1,
            mip_levels: 1,
            format: render::TextureFormat::Rgba8,
            data: alloc::vec![255; 4],
            clamp: false,
        })?;
        let material = r.add_material(Material {
            tint: COLOR,
            lighting: render::Lighting::Unlit,
            no_fog: true,
            no_darkening: true,
            ..Material::opaque(texture)
        })?;
        Ok(ShotMarks { mesh, material, pool: Vec::new(), shown: 0 })
    }

    /// Places one streak per projectile of `world` and hides the rest of the pool.
    pub fn draw<B: Backend>(&mut self, r: &mut Renderer<B>, world: &World) -> Result<(), RenderError> {
        let shots: Vec<([f32; 3], [f32; 3])> =
            world.shared.vehicles.shots.iter().map(|s| (s.position, s.velocity)).collect();
        self.draw_shots(r, &shots)
    }

    /// Places one streak per (position, velocity) and hides the rest of the pool.
    pub fn draw_shots<B: Backend>(
        &mut self,
        r: &mut Renderer<B>,
        shots: &[([f32; 3], [f32; 3])],
    ) -> Result<(), RenderError> {
        let shots = &shots[..shots.len().min(MAX_STREAKS)];
        while self.pool.len() < shots.len() {
            let id = r.add_instance(self.mesh, self.material, Mat4::IDENTITY)?;
            self.pool.push(id);
        }
        for (k, &id) in self.pool.iter().enumerate() {
            match shots.get(k) {
                Some(&(p, v)) => {
                    r.set_transform(id, streak(p, v))?;
                    r.set_visible(id, true)?;
                }
                None => r.set_visible(id, false)?,
            }
        }
        self.shown = shots.len();
        Ok(())
    }
}

/// Whether a streak's head is the shot (for tests): the transform's +X end.
pub fn head(m: &Mat4) -> Vec3 {
    Vec3::new(m.m[12] + m.m[0] * 0.5, m.m[13] + m.m[1] * 0.5, m.m[14] + m.m[2] * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_streak_lies_behind_its_shot_along_the_velocity() {
        let m = streak([100.0, 0.0, 50.0], [6000.0, 0.0, 0.0]);
        let h = head(&m);
        assert!((h.x - 100.0).abs() < 1e-3 && h.y.abs() < 1e-3 && (h.z - 50.0).abs() < 1e-3, "{h:?}");
        assert!((m.m[0] - 120.0).abs() < 1e-3, "6,000 × 0.02 long: {}", m.m[0]);
        let slow = streak([0.0; 3], [10.0, 0.0, 0.0]);
        assert_eq!(slow.m[0], LENGTH[0], "at least the shortest");
        let up = streak([0.0; 3], [0.0, 0.0, -3000.0]);
        assert!((up.m[2] + 60.0).abs() < 1e-3, "a falling shot's streak points down");
    }

    #[test]
    fn the_pool_follows_the_shots() {
        let mut r = Renderer::new(render::NullBackend::new(64, 64));
        let mut marks = ShotMarks::upload(&mut r).unwrap();
        r.commit().unwrap();
        let shot = ([0.0; 3], [1.0, 0.0, 0.0]);
        marks.draw_shots(&mut r, &[shot, shot, shot]).unwrap();
        assert_eq!((marks.shown, marks.pool.len(), r.instance_count()), (3, 3, 3));
        marks.draw_shots(&mut r, &[shot]).unwrap();
        assert_eq!((marks.shown, marks.pool.len()), (1, 3), "the pool keeps its instances, hidden");
        let many = alloc::vec![shot; MAX_STREAKS + 50];
        marks.draw_shots(&mut r, &many).unwrap();
        assert_eq!((marks.shown, marks.pool.len()), (MAX_STREAKS, MAX_STREAKS), "the pool is capped");
    }
}
