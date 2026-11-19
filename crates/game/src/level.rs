//! A loaded world on the renderer: meshes, textures and materials from the world's scene, and one renderer instance
//! per drawn submesh of every engine drawable, so objects move and hide independently.
//!
//! Materials follow textures.md 4: shader kinds 2 to 10 and render passes other than 3, 4, 7, 8 and 13 are not drawn
//! yet; pass 13 is blended (blend flags 2 and 6 alpha, others additive); passes 4 and 8, and textures with
//! transparent texels, are alpha-tested; the rest is opaque. Colour is texture × vertex colour × material colour,
//! unlit.

use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Range;

use engine::anim::skin::SkinnedSubmesh;
use engine::game::render_scene::SkinFrame;
use engine::world::World;
use formats::geometry::Affine;
use formats::scene::Scene;
use render::math::{Mat4, Vec3};
use render::{
    Backend, BlendMode, BoneMatrix, InstanceId, Material, MaterialId, MeshData, MeshId, Pass, RenderError, Renderer,
    SkinVertex, TextureData, Vertex,
};

/// A bounding sphere: centre and radius.
type Sphere = (Vec3, f32);

/// Alpha-test cutoff.
pub const ALPHA_CUTOFF: f32 = 0.5;

/// How a submesh is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draw {
    /// Not drawn yet.
    Skip,
    /// Opaque.
    Opaque,
    /// Alpha-tested.
    Cutout,
    /// Blended.
    Blend(BlendMode),
}

/// Classifies a (shader, mesh) pair (textures.md 4, 4.1, 4.2).
pub fn classify(kind: u8, shader_pass: u8, mesh_pass: u8, blend_flags: u8, texture_has_alpha: bool) -> Draw {
    let drawn = |p: u8| matches!(p, 3 | 4 | 7 | 8 | 13);
    if (2..=10).contains(&kind) || !drawn(shader_pass) || !drawn(mesh_pass) {
        return Draw::Skip;
    }
    if shader_pass == 13 || mesh_pass == 13 {
        return Draw::Blend(if matches!(blend_flags, 2 | 6) { BlendMode::Alpha } else { BlendMode::Additive });
    }
    if matches!(shader_pass, 4 | 8) || matches!(mesh_pass, 4 | 8) || texture_has_alpha {
        return Draw::Cutout;
    }
    Draw::Opaque
}

/// The column-major 4 × 4 of an affine transform.
pub fn mat4(a: &Affine) -> Mat4 {
    Mat4 { m: a.to_mat4() }
}

/// What the level put on the renderer.
#[derive(Default)]
pub struct LevelGpu {
    /// Per engine drawable: its range of `instances`.
    pub drawables: Vec<Range<u32>>,
    /// Renderer instances with their mesh-to-object transform.
    pub instances: Vec<(InstanceId, Affine)>,
    /// Statistics.
    pub stats: LevelStats,
    /// Mesh index (in its mesh set) of each instance's draw.
    pub mesh_of_instance: Vec<u16>,
    /// Instances faded by the camera last frame.
    faded: Vec<u32>,
    /// Per instance: its mesh's rest sphere when the mesh is skinned (it gets the drawable's bone matrices).
    skinned: Vec<Option<Sphere>>,
    bones: Vec<BoneMatrix>,
}

/// Upload statistics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelStats {
    /// Renderer meshes (submeshes uploaded once).
    pub meshes: u32,
    /// Textures, the white one included.
    pub textures: u32,
    /// Materials.
    pub materials: u32,
    /// Renderer instances.
    pub instances: u32,
    /// Submesh draws not drawn yet (special shaders and passes).
    pub skipped: u32,
}

pub(crate) fn has_alpha(pixels: &[u8]) -> bool {
    pixels.as_chunks::<4>().0.iter().any(|p| p[3] < 250)
}

/// Replaces the renderer's level with `world`'s drawables. `scene` is the world's scene ([`Scene::build`]), which
/// holds the converted meshes, materials and textures. Call [`LevelGpu::sync`] every frame after.
pub fn upload<B: Backend>(
    r: &mut Renderer<B>,
    world: &World,
    scene: &Scene,
    extra: impl FnOnce(&mut Renderer<B>) -> Result<(), RenderError>,
) -> Result<LevelGpu, RenderError> {
    r.clear_level();
    let mut out = LevelGpu::default();

    // Textures, then a white one for materials without a texture.
    let mut textures = Vec::with_capacity(scene.textures.len() + 1);
    let mut alpha = Vec::with_capacity(scene.textures.len());
    for t in &scene.textures {
        alpha.push(has_alpha(&t.image.pixels));
        textures.push(r.add_texture(TextureData::rgba8_with_mips(
            t.image.width,
            t.image.height,
            t.image.pixels.clone(),
        )?)?);
    }
    let white = r.add_texture(TextureData::rgba8(8, 8, vec![255; 8 * 8 * 4])?)?;
    out.stats.textures = textures.len() as u32 + 1;

    // Meshes and materials are made on first use.
    let mut meshes: Vec<Option<(MeshId, Option<Sphere>)>> = vec![None; scene.submeshes.len()];
    let mut materials: BTreeMap<(u16, u8), MaterialId> = BTreeMap::new();
    let mesh_of: BTreeMap<(u32, u32), u32> =
        scene.meshes.iter().enumerate().map(|(i, m)| ((m.record, m.mesh), i as u32)).collect();

    // First pass: decide every draw, upload what is used.
    struct Planned {
        drawable: u32,
        mesh: MeshId,
        material: MaterialId,
        local: Affine,
        mesh_index: u16,
        skinned: Option<Sphere>,
        receiver: u8,
    }
    let mut planned: Vec<Planned> = Vec::new();
    for (d, dr) in world.drawables().iter().enumerate() {
        let Some(set) = world.level.record(dr.record).mesh_set.as_ref() else { continue };
        for draw in set.draws() {
            if draw.transform.is_degenerate() {
                continue;
            }
            let Some(&sm) = mesh_of.get(&(dr.record, draw.geometry as u32)) else { continue };
            for si in scene.meshes[sm as usize].submeshes.clone() {
                let sub = &scene.submeshes[si as usize];
                if sub.index_count == 0 {
                    continue;
                }
                let Some(sh) = scene.materials.get(sub.material as usize) else {
                    out.stats.skipped += 1;
                    continue;
                };
                let a = sh.texture.is_some_and(|t| alpha[t as usize]);
                let (slot, pass, blend) = match classify(sh.kind, sh.pass, draw.pass, sh.blend, a) {
                    Draw::Skip => {
                        out.stats.skipped += 1;
                        continue;
                    }
                    Draw::Opaque => (0, Pass::Opaque, BlendMode::Alpha),
                    Draw::Cutout => (1, Pass::AlphaTest, BlendMode::Alpha),
                    Draw::Blend(BlendMode::Alpha) => (2, Pass::Blend, BlendMode::Alpha),
                    Draw::Blend(b) => (3, Pass::Blend, b),
                };
                let material = match materials.get(&(sub.material, slot)) {
                    Some(&m) => m,
                    None => {
                        let texture = sh.texture.map_or(white, |t| textures[t as usize]);
                        let m = r.add_material(Material {
                            tint: sh.color,
                            alpha_cutoff: ALPHA_CUTOFF,
                            blend,
                            lighting: render::types::Lighting::Unlit,
                            ..Material::opaque(texture).with_pass(pass)
                        })?;
                        materials.insert((sub.material, slot), m);
                        m
                    }
                };
                let mesh = match meshes[si as usize] {
                    Some(m) => m,
                    None => {
                        let verts: Vec<Vertex> = scene.vertices[sub.base_vertex as usize..]
                            [..sub.vertex_count as usize]
                            .iter()
                            .map(|v| Vertex::new(v.position, v.uv, v.normal, v.color))
                            .collect();
                        let idx = &scene.indices[sub.first_index as usize..][..sub.index_count as usize];
                        let k = si - scene.meshes[sm as usize].submeshes.start;
                        let data = MeshData { vertices: &verts, indices: idx };
                        let m = match submesh_skin(&world.level.bytes, set, draw.geometry, k as usize, verts.len()) {
                            Some(skin) => (r.add_skinned_mesh(&data, &skin)?, Some(vertex_sphere(&verts))),
                            None => (r.add_mesh(&data)?, None),
                        };
                        meshes[si as usize] = Some(m);
                        m
                    }
                };
                planned.push(Planned {
                    drawable: d as u32,
                    mesh: mesh.0,
                    material,
                    local: draw.transform,
                    mesh_index: draw.mesh as u16,
                    skinned: mesh.1,
                    // Projector receivers: the issuing mesh's flags 0x01 (shadows) and 0x02 (light patterns).
                    receiver: set.meshes.get(draw.mesh).map_or(0, |m| m.mesh_flags & 0x03),
                });
            }
        }
    }
    extra(r)?;
    r.commit()?;
    out.stats.meshes = meshes.iter().filter(|m| m.is_some()).count() as u32;
    out.stats.materials = materials.len() as u32;

    // Instances, grouped by drawable.
    out.drawables = vec![0..0; world.drawables().len()];
    let wt = world.world_transforms();
    for p in planned {
        let dr = world.drawables()[p.drawable as usize];
        let w = wt[dr.object.index() as usize];
        let id = r.add_instance(p.mesh, p.material, mat4(&w.mul(&p.local)))?;
        r.set_visible(id, dr.visible)?;
        if p.receiver != 0 {
            r.set_receiver(id, p.receiver)?;
        }
        // The player character's instances cast its projected shadow.
        if world.player.is_some_and(|pl| is_under(world, dr.object, pl)) {
            r.set_shadow_caster(id, true)?;
        }
        let k = out.instances.len() as u32;
        let range = &mut out.drawables[p.drawable as usize];
        if range.start == range.end {
            *range = k..k + 1;
        } else {
            range.end = k + 1;
        }
        out.instances.push((id, p.local));
        out.mesh_of_instance.push(p.mesh_index);
        out.skinned.push(p.skinned);
    }
    out.stats.instances = out.instances.len() as u32;
    Ok(out)
}

/// The skinning data of submesh `k` of mesh `mesh` of a mesh set with bones, one per scene vertex (strip entry), or
/// `None` when the set has no bones or the submesh no weights.
fn submesh_skin(
    data: &[u8],
    set: &formats::geometry::MeshSet,
    mesh: usize,
    k: usize,
    count: usize,
) -> Option<Vec<SkinVertex>> {
    if set.bones.is_empty() {
        return None;
    }
    let sm = set.meshes.get(mesh)?.submeshes.get(k)?;
    let sk = SkinnedSubmesh::from_submesh(data, sm, set.pose_count() as usize);
    if sk.influences.len() != count || !sk.is_skinned() {
        return None;
    }
    Some(sk.influences.iter().map(|v| SkinVertex::new(v.bones, v.weights)).collect())
}

/// A sphere around the vertices: the centre of their box and the distance to the farthest.
fn vertex_sphere(verts: &[Vertex]) -> (Vec3, f32) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in verts {
        for k in 0..3 {
            lo[k] = lo[k].min(v.pos[k]);
            hi[k] = hi[k].max(v.pos[k]);
        }
    }
    let c = Vec3::new((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5);
    let r = verts.iter().map(|v| (Vec3::new(v.pos[0], v.pos[1], v.pos[2]) - c).length()).fold(0.0f32, f32::max);
    (c, r)
}

/// Whether `h` is `root` or below it.
fn is_under(world: &World, h: engine::object::Handle, root: engine::object::Handle) -> bool {
    let mut at = Some(h);
    while let Some(x) = at {
        if x == root {
            return true;
        }
        at = world.parent(x);
    }
    false
}

/// The player character's projected shadow (options 43 to 48, rendering.md 9.2, effects.md 7): at its position,
/// with the strength of the shadow shader's material alpha; `None` when it casts none.
pub fn player_projector(world: &World) -> Option<render::Projector> {
    let p = world.player?;
    let o = world.options(p)?;
    if !o.bool_or(43, false) {
        return None;
    }
    let oid = o.i32(44)?;
    let shader = world.level.layout.materials.shaders.iter().find(|s| i32::from(s.oid) == oid)?;
    let t = world.position(p)?;
    Some(render::Projector {
        origin: Vec3::new(t[0], t[1], t[2]),
        near_radius: o.f32(45).unwrap_or(150.0),
        far_radius: o.f32(46).unwrap_or(10.0),
        near_distance: o.f32(47).unwrap_or(100.0),
        far_distance: o.f32(48).unwrap_or(400.0),
        strength: f32::from(shader.color[3]) / 255.0,
        receivers: 0x01,
    })
}

/// A skinning matrix (columns + translation) as the renderer's three bone rows.
pub fn bone_rows(a: &Affine) -> BoneMatrix {
    let c = &a.cols;
    BoneMatrix {
        rows: [
            [c[0][0], c[1][0], c[2][0], a.t[0]],
            [c[0][1], c[1][1], c[2][1], a.t[1]],
            [c[0][2], c[1][2], c[2][2], a.t[2]],
        ],
    }
}

/// A sphere around everything the skinning matrices can make of the rest sphere `(c, r)`.
fn posed_sphere((c, r): (Vec3, f32), matrices: &[Affine]) -> (Vec3, f32) {
    let mut reach = 0.0f32;
    for m in matrices {
        let moved = m.transform_point([c.x, c.y, c.z]);
        let d = Vec3::new(moved[0] - c.x, moved[1] - c.y, moved[2] - c.z).length();
        let stretch = m.cols.iter().map(|v| Vec3::new(v[0], v[1], v[2]).length()).fold(1.0f32, f32::max);
        reach = reach.max(d + r * (stretch - 1.0));
    }
    (c, r + reach)
}

impl LevelGpu {
    /// Gives the instances of skinned submeshes their drawable's bone matrices for this frame (the render scene's
    /// skins), with a culling sphere that covers the pose.
    pub fn skin<B: Backend>(&mut self, r: &mut Renderer<B>, skins: &SkinFrame) -> Result<(), RenderError> {
        for (d, range) in &skins.skins {
            let Some(inst) = self.drawables.get(*d as usize).cloned() else { continue };
            let Some(matrices) = skins.matrices.get(range.start as usize..range.end as usize) else { continue };
            if matrices.is_empty() {
                continue;
            }
            self.bones.clear();
            self.bones.extend(matrices.iter().map(bone_rows));
            for k in inst {
                let Some(rest) = self.skinned[k as usize] else { continue };
                let id = self.instances[k as usize].0;
                r.set_bones(id, &self.bones)?;
                r.set_bounding_sphere(id, Some(posed_sphere(rest, matrices)))?;
            }
        }
        Ok(())
    }

    /// Hides the meshes the camera fades because they block the view (camera.md 7), and shows again those it
    /// stopped fading.
    pub fn fade<B: Backend>(
        &mut self,
        r: &mut Renderer<B>,
        world: &World,
        faded: &[(u32, u16)],
    ) -> Result<(), RenderError> {
        let mut now: Vec<u32> = Vec::new();
        if !faded.is_empty() {
            for (d, dr) in world.drawables().iter().enumerate() {
                if !faded.iter().any(|&(rec, _)| rec == dr.record) {
                    continue;
                }
                let range = self.drawables[d].clone();
                for k in range {
                    let m = self.mesh_of_instance[k as usize];
                    if faded.contains(&(dr.record, m)) {
                        now.push(k);
                    }
                }
            }
        }
        for &k in &self.faded {
            if !now.contains(&k) {
                let (id, _) = self.instances[k as usize];
                let d = self.drawables.iter().position(|r| r.contains(&k));
                let vis = d.is_some_and(|d| world.drawables()[d].visible);
                r.set_visible(id, vis)?;
            }
        }
        for &k in &now {
            r.set_visible(self.instances[k as usize].0, false)?;
        }
        self.faded = now;
        Ok(())
    }

    /// Updates the instances of the drawables that changed (`changed` from the render scene).
    pub fn sync<B: Backend>(&self, r: &mut Renderer<B>, world: &World, changed: &[u32]) -> Result<(), RenderError> {
        let wt = world.world_transforms();
        for &d in changed {
            let Some(range) = self.drawables.get(d as usize) else { continue };
            let dr = world.drawables()[d as usize];
            let w = wt[dr.object.index() as usize];
            for &(id, local) in &self.instances[range.start as usize..range.end as usize] {
                r.set_transform(id, mat4(&w.mul(&local)))?;
                r.set_visible(id, dr.visible)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification() {
        assert_eq!(classify(0, 3, 3, 2, false), Draw::Opaque);
        assert_eq!(classify(0, 3, 3, 2, true), Draw::Cutout);
        assert_eq!(classify(1, 4, 4, 2, false), Draw::Cutout);
        assert_eq!(classify(0, 13, 13, 2, false), Draw::Blend(BlendMode::Alpha));
        assert_eq!(classify(1, 13, 13, 3, false), Draw::Blend(BlendMode::Additive));
        assert_eq!(classify(1, 3, 13, 6, false), Draw::Blend(BlendMode::Alpha));
        for kind in 2..=10 {
            assert_eq!(classify(kind, 3, 3, 2, false), Draw::Skip);
        }
        for pass in [0, 1, 2, 5, 6, 9, 10, 11, 12, 14, 15, 16, 17] {
            assert_eq!(classify(0, pass, 3, 2, false), Draw::Skip);
            assert_eq!(classify(0, 3, pass, 2, false), Draw::Skip);
        }
    }
}
