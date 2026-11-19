//! Procedural test scene: a grid of textured boxes, each with its own transform, mesh variant and material, so the
//! batching paths are exercised the way a level with many distinct sub-objects would exercise them.
//!
//! Shared by the PC example (`examples/cubes.rs`) and the Switch demo. Deterministic: the same parameters give the
//! same scene on every platform.

use crate::backend::Backend;
use crate::math::{Mat4, PI, Vec3, sin_cos};
use crate::renderer::{Camera, Renderer};
use crate::types::{InstanceId, Material, MaterialId, MeshData, MeshId, Pass, RenderError, TextureData, Vertex};
use alloc::vec::Vec;

/// Scene parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DemoParams {
    /// Boxes along x and z.
    pub grid_x: u32,
    pub grid_z: u32,
    /// Distinct meshes (box shapes and vertex colours). Each one becomes its own draw command per batch, like the
    /// unique sub-object meshes of a level.
    pub mesh_variants: u32,
    /// Spin every box each frame (all transforms change) instead of a static scene.
    pub animate: bool,
}

impl Default for DemoParams {
    fn default() -> DemoParams {
        // 3072 boxes and 1536 distinct meshes: twice the instances and as many unique meshes as a busy level.
        DemoParams { grid_x: 64, grid_z: 48, mesh_variants: 1536, animate: true }
    }
}

const SPACING: f32 = 2.5;

struct Placed {
    id: InstanceId,
    pos: Vec3,
    axis: Vec3,
    speed: f32,
    phase: f32,
    scale: f32,
}

/// A built scene; `update` animates it and `camera` gives the orbiting camera.
pub struct DemoScene {
    placed: Vec<Placed>,
    params: DemoParams,
    pub meshes: u32,
    pub textures: u32,
    pub materials: u32,
}

/// Small deterministic hash (one round of a 32-bit integer mixer).
fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// A box from -0.5..0.5 with the top face scaled by `taper` and height `height`, 24 vertices, 36 indices, CCW
/// winding seen from outside.
pub fn box_mesh(taper: f32, height: f32, color: [u8; 4]) -> (Vec<Vertex>, Vec<u16>) {
    let h = height * 0.5;
    // Corners: bottom (y = -h) at full size, top (y = +h) scaled by taper.
    let c = |x: f32, y: f32, z: f32| {
        let s = if y > 0.0 { taper } else { 1.0 };
        [x * 0.5 * s, y * h, z * 0.5 * s]
    };
    // Each face: 4 corners in CCW order seen from outside, and its outward normal (approximate for tapered sides).
    let faces: [([[f32; 3]; 4], [f32; 3]); 6] = [
        ([c(-1.0, -1.0, 1.0), c(1.0, -1.0, 1.0), c(1.0, 1.0, 1.0), c(-1.0, 1.0, 1.0)], [0.0, 0.0, 1.0]),
        ([c(1.0, -1.0, -1.0), c(-1.0, -1.0, -1.0), c(-1.0, 1.0, -1.0), c(1.0, 1.0, -1.0)], [0.0, 0.0, -1.0]),
        ([c(1.0, -1.0, 1.0), c(1.0, -1.0, -1.0), c(1.0, 1.0, -1.0), c(1.0, 1.0, 1.0)], [1.0, 0.0, 0.0]),
        ([c(-1.0, -1.0, -1.0), c(-1.0, -1.0, 1.0), c(-1.0, 1.0, 1.0), c(-1.0, 1.0, -1.0)], [-1.0, 0.0, 0.0]),
        ([c(-1.0, 1.0, 1.0), c(1.0, 1.0, 1.0), c(1.0, 1.0, -1.0), c(-1.0, 1.0, -1.0)], [0.0, 1.0, 0.0]),
        ([c(-1.0, -1.0, -1.0), c(1.0, -1.0, -1.0), c(1.0, -1.0, 1.0), c(-1.0, -1.0, 1.0)], [0.0, -1.0, 0.0]),
    ];
    let uvs = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let mut v = Vec::with_capacity(24);
    let mut idx = Vec::with_capacity(36);
    for (corners, n) in faces.iter() {
        let base = v.len() as u16;
        for k in 0..4 {
            v.push(Vertex::new(corners[k], uvs[k], *n, color));
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (v, idx)
}

/// Texture kinds of the demo.
#[derive(Clone, Copy)]
enum Pattern {
    Checker,
    Stripes,
    Rings,
    /// Lattice with fully transparent holes (alpha-tested).
    Lattice,
    /// Half-transparent glass with a bright rim (blended).
    Glass,
}

fn make_texture(pattern: Pattern, size: u32, a: [u8; 3], b: [u8; 3]) -> Result<TextureData, RenderError> {
    let mut px = Vec::with_capacity((size * size * 4) as usize);
    let s = size as i32;
    for y in 0..s {
        for x in 0..s {
            let (use_a, alpha) = match pattern {
                Pattern::Checker => (((x * 8 / s) + (y * 8 / s)) % 2 == 0, 255),
                Pattern::Stripes => (((x + y) * 6 / s) % 2 == 0, 255),
                Pattern::Rings => {
                    let (dx, dy) = (2 * x - s + 1, 2 * y - s + 1);
                    let r2 = dx * dx + dy * dy;
                    ((r2 * 16 / (s * s)) % 2 == 0, 255)
                }
                Pattern::Lattice => {
                    let cell = s / 4;
                    let edge = (x % cell) < cell / 4 || (y % cell) < cell / 4;
                    (edge, if edge { 255 } else { 0 })
                }
                Pattern::Glass => {
                    let rim = x < s / 16 || y < s / 16 || x >= s - s / 16 || y >= s - s / 16;
                    (rim, if rim { 230 } else { 110 })
                }
            };
            let c = if use_a { a } else { b };
            px.extend_from_slice(&[c[0], c[1], c[2], alpha]);
        }
    }
    TextureData::rgba8_with_mips(size, size, px)
}

impl DemoScene {
    /// Adds the scene's meshes, textures and materials, commits them, and places the boxes.
    pub fn build<B: Backend>(r: &mut Renderer<B>, params: &DemoParams) -> Result<DemoScene, RenderError> {
        // Meshes.
        let variants = params.mesh_variants.max(1);
        let mut meshes: Vec<MeshId> = Vec::with_capacity(variants as usize);
        for i in 0..variants {
            let h = hash(i.wrapping_mul(2654435761) ^ 0x5eed);
            let taper = 0.45 + 0.55 * unit(h);
            let height = 0.6 + 0.8 * unit(hash(h));
            let tint = hash(h ^ 0xabc);
            let color = [190 + (tint & 63) as u8, 190 + ((tint >> 8) & 63) as u8, 190 + ((tint >> 16) & 63) as u8, 255];
            let (v, idx) = box_mesh(taper, height, color);
            meshes.push(r.add_mesh(&MeshData { vertices: &v, indices: &idx })?);
        }

        // Textures: two sizes so there are two opaque pools; alpha-tested and blended ones share the 64 pool.
        let palette: [([u8; 3], [u8; 3]); 6] = [
            ([230, 90, 60], [250, 220, 180]),
            ([60, 140, 230], [210, 230, 250]),
            ([70, 180, 90], [230, 245, 200]),
            ([200, 170, 60], [90, 60, 40]),
            ([150, 80, 200], [240, 220, 250]),
            ([40, 40, 50], [200, 200, 210]),
        ];
        let patterns = [Pattern::Checker, Pattern::Stripes, Pattern::Rings];
        let mut opaque: Vec<MaterialId> = Vec::new();
        let mut cutout: Vec<MaterialId> = Vec::new();
        let mut blended: Vec<MaterialId> = Vec::new();
        let mut textures = 0;
        for (i, (a, b)) in palette.iter().enumerate() {
            for (j, &size) in [64u32, 128].iter().enumerate() {
                let t = r.add_texture(make_texture(patterns[(i + j) % patterns.len()], size, *a, *b)?)?;
                textures += 1;
                opaque.push(r.add_material(Material::opaque(t))?);
            }
        }
        for (a, b) in palette.iter().take(2) {
            let t = r.add_texture(make_texture(Pattern::Lattice, 64, *a, *b)?)?;
            textures += 1;
            cutout.push(
                r.add_material(Material { alpha_cutoff: 0.5, ..Material::opaque(t).with_pass(Pass::AlphaTest) })?,
            );
        }
        for (a, b) in palette.iter().skip(1).take(2) {
            let t = r.add_texture(make_texture(Pattern::Glass, 64, *a, *b)?)?;
            textures += 1;
            blended.push(r.add_material(Material::opaque(t).with_pass(Pass::Blend))?);
        }
        let materials = (opaque.len() + cutout.len() + blended.len()) as u32;
        r.commit()?;

        // Instances: 85% opaque, 10% alpha-tested, 5% blended.
        let mut placed = Vec::with_capacity((params.grid_x * params.grid_z) as usize);
        let (ox, oz) = ((params.grid_x as f32 - 1.0) * 0.5 * SPACING, (params.grid_z as f32 - 1.0) * 0.5 * SPACING);
        for gz in 0..params.grid_z {
            for gx in 0..params.grid_x {
                let n = gz * params.grid_x + gx;
                let h = hash(n ^ 0x9e37_79b9);
                let mesh = meshes[(hash(h) % variants) as usize];
                let pick = h % 100;
                let mat = if pick < 85 {
                    opaque[(hash(h ^ 1) as usize) % opaque.len()]
                } else if pick < 95 {
                    cutout[(hash(h ^ 2) as usize) % cutout.len()]
                } else {
                    blended[(hash(h ^ 3) as usize) % blended.len()]
                };
                let pos = Vec3::new(gx as f32 * SPACING - ox, 0.0, gz as f32 * SPACING - oz);
                let axis = Vec3::new(unit(hash(h ^ 4)) - 0.5, 1.0, unit(hash(h ^ 5)) - 0.5).normalize();
                let p = Placed {
                    id: InstanceId(0),
                    pos,
                    axis,
                    speed: 0.3 + 1.2 * unit(hash(h ^ 6)),
                    phase: unit(hash(h ^ 7)) * 2.0 * PI,
                    scale: 0.8 + 0.6 * unit(hash(h ^ 8)),
                };
                let id = r.add_instance(mesh, mat, Self::transform(&p, 0.0))?;
                placed.push(Placed { id, ..p });
            }
        }
        Ok(DemoScene { placed, params: *params, meshes: variants, textures, materials })
    }

    fn transform(p: &Placed, t: f32) -> Mat4 {
        Mat4::trs(p.pos, p.axis, p.phase + p.speed * t, Vec3::new(p.scale, p.scale, p.scale))
    }

    pub fn instance_count(&self) -> usize {
        self.placed.len()
    }

    /// Moves the boxes to time `t` (seconds). Does nothing for a static scene.
    pub fn update<B: Backend>(&self, r: &mut Renderer<B>, t: f32) -> Result<(), RenderError> {
        if !self.params.animate {
            return Ok(());
        }
        for p in &self.placed {
            r.set_transform(p.id, Self::transform(p, t))?;
        }
        Ok(())
    }

    /// Camera orbiting the grid at time `t`, looking at its centre from above.
    pub fn camera(&self, t: f32) -> Camera {
        let extent = (self.params.grid_x.max(self.params.grid_z) as f32) * SPACING;
        let (s, c) = sin_cos(t * 0.15);
        let dist = extent * 0.42;
        let mut cam = Camera::look_at(Vec3::new(s * dist, extent * 0.2, c * dist), Vec3::new(0.0, -4.0, 0.0));
        cam.fov_y = 1.05;
        cam.far = extent * 3.0;
        cam
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NullBackend;

    #[test]
    fn box_winding_faces_outward() {
        let (v, idx) = box_mesh(0.7, 1.2, [255; 4]);
        assert_eq!((v.len(), idx.len()), (24, 36));
        for tri in idx.chunks(3) {
            let p = |i: u16| Vec3::new(v[i as usize].pos[0], v[i as usize].pos[1], v[i as usize].pos[2]);
            let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
            let n = (b - a).cross(c - a);
            let centroid = (a + b + c) * (1.0 / 3.0);
            assert!(n.dot(centroid) > 0.0, "triangle {tri:?} faces inward");
        }
    }

    #[test]
    fn demo_scene_batches() {
        let params = DemoParams { grid_x: 32, grid_z: 24, mesh_variants: 200, animate: true };
        let mut r = Renderer::new(NullBackend::new(1280, 720));
        let scene = DemoScene::build(&mut r, &params).unwrap();
        assert_eq!(scene.instance_count(), 768);
        assert_eq!(scene.textures, 16);
        assert_eq!(r.texture_pool_count(), 2);
        scene.update(&mut r, 1.0).unwrap();
        let s = r.render(&scene.camera(1.0), [0.0; 4]).unwrap();
        assert!(s.visible > 100 && s.visible <= 768, "{s:?}");
        // Opaque: at most 2 pools; alpha-tested: 1 pool; blended: 1 pool (all 64x64), so 4 batches.
        assert_eq!(s.batches, 4, "{:?}", r.backend().last_batches);
        assert!(s.draw_commands <= 200 * 3 + 64, "{s:?}");
        assert_eq!(s.triangles, s.visible as u64 * 12);
        // Same input, same output.
        let first = r.backend().last_instances.clone();
        r.render(&scene.camera(1.0), [0.0; 4]).unwrap();
        assert_eq!(first, r.backend().last_instances);
    }
}
