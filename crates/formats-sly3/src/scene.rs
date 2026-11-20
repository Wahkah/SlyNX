//! Builds a `formats::scene::Scene` from a Sly 3 world.
//!
//! What is drawn (world-file.md 3.2 and 6, classes/placement.md 2, geometry.md 2.1 and 7, object-stream.md 1.3
//! and 2.3):
//!
//! - shared groups are drawn where they are stored; every other group is a template, drawn once per placement that
//!   names it (class 8, modes −1, 0 and 1; mode-2 spawners are left to gameplay), at the placement's matrix; a copy
//!   placement's extra objects are drawn with it, relative to its matrix (a fresh placement's data is skipped, as
//!   the game skips it);
//! - mode-1 records and filtered meshes whose world filter or conditions fail in the chosen [`Context`] are left
//!   out, and so are objects and placements marked "remove at start" (option 27), hidden meshes and groupers'
//!   objects (kept out of the world until gameplay adds them);
//! - strip and indexed meshes are drawn; outline and effect-outline meshes carry only outline hulls and are not;
//! - a mesh is drawn with its transform's world matrix (a copy: its own matrix first), with the transform list's
//!   world matrices composed as loaded (the bind pose); a zone's objects are placed by the zone's matrix.
//!
//! Vertex colours are the stored ones; colour tables (geometry.md 4.2) are not applied. Textures are frame 0,
//! palette slot 0, graded when the palette asks for it (textures.md 5).
//!
//! `SceneInstance::record` and `SceneMesh::record` hold the record's header offset in the world data;
//! `SceneInstance::issuing_mesh` is the mesh's index in that record's mesh set. Materials are numbered across
//! groups: material `m` is shader `m - base` of the group whose base it falls in ([`SceneBuild::material_base`]).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use formats::geometry::Affine;
use formats::scene::{Scene, SceneInstance, SceneMaterial, SceneMesh, SceneSubmesh, SceneTexture};
use formats_sly2::transforms::resolve;

use crate::filter::Context;
use crate::geometry::{INDEXED, Mesh, STRIPS};
use crate::stream::{Body, Layout, Record};
use crate::textures::{self, GRADING};
use crate::world::World;

/// Options for [`build`].
#[derive(Clone, Debug, PartialEq)]
pub struct SceneOptions {
    /// The selection the world is shown with; [`Context::any`] keeps everything.
    pub context: Context,
    /// Decode textures.
    pub decode_textures: bool,
    /// Apply colour grading to palettes that ask for it.
    pub grade: bool,
    /// Render passes to include (bit `p` for pass `p`). Default: 0, 2, 3, 4, 7 and 9 (Sly 2's default minus one).
    pub passes: u32,
    /// Deepest chain of placements followed.
    pub max_depth: u32,
    /// Most records visited, placed copies included ([`MAX_SCENE_RECORDS`] by default): placements that place
    /// placements can fan out without bound in corrupt data.
    pub record_budget: u32,
    /// Light classes: 4 up to its last derived class (the executable's class table, object-stream.md 3.1;
    /// [`light_classes`]). Default: class 4 alone.
    pub lights: (i16, i16),
}

/// Class of lights.
pub const LIGHT_CLASS: i16 = 4;

/// One record of a colour table (geometry.md 4.2): colours A and B and the amount.
pub use formats_sly2::scene::BakedColour;

/// Strip-list flag: the colours come from the colour table (geometry.md 4.2).
pub const LIST_TABLE_COLOURS: i32 = 0x20;

/// The light classes of the executable's class table: class 4 and the classes derived from it (IDs up to its last
/// derived class).
pub fn light_classes(classes: &[crate::exe::ClassEntry]) -> (i16, i16) {
    let last = classes.iter().find(|c| c.class == LIGHT_CLASS as i32).map_or(LIGHT_CLASS as i32, |c| c.last_derived);
    (LIGHT_CLASS, last.clamp(LIGHT_CLASS as i32, i16::MAX as i32) as i16)
}

impl Default for SceneOptions {
    fn default() -> Self {
        SceneOptions {
            context: Context::any(),
            decode_textures: true,
            grade: true,
            passes: (1 << 0) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 7) | (1 << 9),
            max_depth: 8,
            record_budget: MAX_SCENE_RECORDS,
            lights: (LIGHT_CLASS, LIGHT_CLASS),
        }
    }
}

/// The scene and how materials map back to groups.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneBuild {
    pub scene: Scene,
    /// First material index of each group (by group index).
    pub material_base: Vec<u32>,
    /// Placements whose group id names no group of the world.
    pub unresolved_placements: u32,
    /// Records left out once [`SceneOptions::record_budget`] was spent.
    pub over_budget: u32,
    /// Per material: the texture of frame 0, palette slot 1 (kinds 1 and 7: the grey palette), decoded with the
    /// others.
    pub slot1: Vec<Option<u32>>,
    /// Per material: its shader's parameter bytes 20 and 21 as a little-endian `u16` (Sly 2's self-illumination).
    pub self_illum: Vec<u16>,
    /// Per material: its shader's second colour (bytes 12 to 15; kind 3's constant colour).
    pub color2: Vec<[u8; 4]>,
    /// Pre-baked colours (geometry.md 4.2, rendering.md 2.3): for (scene instance, submesh) whose strip list takes
    /// its colours from the mesh's (or copy's) colour table in effect, the first of its vertices' records in
    /// [`SceneBuild::baked_colours`], one per vertex of the submesh.
    pub baked: BTreeMap<(u32, u32), u32>,
    pub baked_colours: Vec<BakedColour>,
    /// Per scene submesh: its vertex group in the mesh and whether its strip list's flags have bit `0x20` (colours
    /// from the colour table).
    pub submesh_source: Vec<(u16, bool)>,
    /// Per scene submesh: its strip list's flags.
    pub submesh_flags: Vec<i32>,
    /// Per scene mesh: its strips body's flags byte (bit 0: lit per frame, as Sly 2's rendering.md 2.3).
    pub mesh_flags: Vec<u8>,
    /// The lights (class 4 and derived classes, options 159 to 165, rendering.md 2.1) of the drawn objects, in world
    /// space.
    pub lights: Vec<formats_sly2::scene::SceneLight>,
}

struct Builder<'w, 'a> {
    world: &'w World<'a>,
    opts: SceneOptions,
    out: SceneBuild,
    mesh_cache: BTreeMap<u32, u32>,
    texture_cache: BTreeMap<(usize, u16, u16), Option<u32>>,
    used_materials: Vec<bool>,
    by_id: BTreeMap<i32, usize>,
    /// Records visited so far.
    visited: u32,
}

/// Default record budget of a scene build: far above any world on the disc.
pub const MAX_SCENE_RECORDS: u32 = 1 << 20;

/// Builds the scene of `world`.
pub fn build(world: &World<'_>, opts: SceneOptions) -> SceneBuild {
    let mut base = Vec::with_capacity(world.bodies.len());
    let mut materials = Vec::new();
    let mut self_illum = Vec::new();
    let mut color2 = Vec::new();
    for b in &world.bodies {
        base.push(materials.len() as u32);
        for s in &b.materials.shaders {
            materials.push(SceneMaterial { kind: s.kind, blend: s.flags, pass: s.pass, color: s.color, texture: None });
            self_illum.push(u16::from_le_bytes(s.params));
            let c = world.data.get(s.offset as usize + 12..s.offset as usize + 16).unwrap_or(&[0; 4]);
            color2.push([c[0], c[1], c[2], c[3]]);
        }
    }
    let slot1 = alloc::vec![None; materials.len()];
    let used = alloc::vec![false; materials.len()];
    let by_id = world.groups.iter().enumerate().map(|(i, g)| (g.id, i)).collect();
    let decode = opts.decode_textures;
    let mut b = Builder {
        world,
        opts,
        out: SceneBuild {
            scene: Scene { materials, ..Scene::default() },
            material_base: base,
            unresolved_placements: 0,
            over_budget: 0,
            slot1,
            self_illum,
            color2,
            baked: BTreeMap::new(),
            baked_colours: Vec::new(),
            submesh_source: Vec::new(),
            submesh_flags: Vec::new(),
            mesh_flags: Vec::new(),
            lights: Vec::new(),
        },
        mesh_cache: BTreeMap::new(),
        texture_cache: BTreeMap::new(),
        used_materials: used,
        by_id,
        visited: 0,
    };
    for (i, g) in world.groups.iter().enumerate() {
        if g.shared != 0 {
            b.group(i, &Affine::IDENTITY, 0);
        }
    }
    if decode {
        b.textures();
    }
    b.out
}

impl Builder<'_, '_> {
    fn group(&mut self, index: usize, root: &Affine, depth: u32) {
        let objects = &self.world.bodies[index].objects;
        self.records(index, objects, root, &[], depth);
    }

    fn records(&mut self, grp: usize, records: &[Record], root: &Affine, enclosing: &[Affine], depth: u32) {
        for r in records {
            if !r.loaded(&self.opts.context) || r.removed_at_start() {
                continue;
            }
            if self.visited >= self.opts.record_budget {
                self.out.over_budget += 1;
                continue;
            }
            self.visited += 1;
            match &r.body {
                Body::Solid(s) => {
                    let world = s.transforms.world_matrices(root, enclosing);
                    for (mi, mesh) in s.meshes.meshes.iter().enumerate() {
                        if mesh.hidden || !mesh.filter.matches(&self.opts.context) {
                            continue;
                        }
                        self.mesh(grp, r, mi as u32, mesh, &world, enclosing, root);
                    }
                    self.records(grp, &s.children, root, &world, depth);
                }
                Body::Placement(p) => {
                    if p.mode == 2 || depth >= self.opts.max_depth {
                        continue;
                    }
                    let parent = resolve(p.parent, &[], enclosing, root);
                    let at = parent.mul(&p.matrix);
                    match self.by_id.get(&p.group_id).copied() {
                        Some(target) => self.group(target, &at, depth + 1),
                        None => self.out.unresolved_placements += 1,
                    }
                    if !p.fresh {
                        self.records(grp, &p.objects, &at, &[], depth + 1);
                    }
                }
                Body::Other { layout: Layout::Zone, header: Some(h), children, .. } => {
                    let parent = resolve(h.parent, &[], enclosing, root);
                    let at = parent.mul(&h.matrix);
                    self.records(grp, children, &at, &[], depth);
                }
                Body::Other { header: Some(h), options: Some(o), .. }
                    if (self.opts.lights.0..=self.opts.lights.1).contains(&r.class) =>
                {
                    let at = resolve(h.parent, &[], enclosing, root).mul(&h.matrix);
                    let data = self.world.data;
                    let light = formats_sly2::scene::SceneLight::from_values(
                        at,
                        formats_sly2::scene::LightOptions::SLY3,
                        |id| o.value(data, id),
                    );
                    // Only kinds 0 (directional) and 1 (point) are lights.
                    if light.kind <= 1 {
                        self.out.lights.push(light);
                    }
                }
                _ => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn mesh(
        &mut self,
        grp: usize,
        r: &Record,
        index: u32,
        mesh: &Mesh,
        own: &[Affine],
        enclosing: &[Affine],
        root: &Affine,
    ) {
        if !(mesh.body_type == STRIPS || mesh.body_type == INDEXED) {
            return;
        }
        let pass = mesh.pass.max(0) as u32;
        if pass >= 32 || self.opts.passes & (1 << pass) == 0 {
            return;
        }
        let Some(scene_mesh) = self.scene_mesh(grp, r, index, mesh) else { return };
        let base = resolve(mesh.transform, own, enclosing, root);
        let billboard = mesh.billboard.max(0) as u8;
        // The base and every shown copy, each with its colour table in effect (the last whose condition matches).
        let mut draws = alloc::vec![(base, mesh.color_table(&self.opts.context))];
        for c in mesh.copies.iter().filter(|c| !c.hidden) {
            let t = resolve(c.transform, own, enclosing, root);
            let table = c.color_tables.iter().rev().find(|t| {
                crate::filter::Filter { slots: Vec::new(), conditions: alloc::vec![t.condition] }
                    .matches(&self.opts.context)
            });
            draws.push((t.mul(&c.matrix), table));
        }
        for (t, table) in draws {
            let instance = self.out.scene.instances.len() as u32;
            if let Some(table) = table {
                self.baked(instance, scene_mesh, table);
            }
            self.out.scene.instances.push(SceneInstance {
                mesh: scene_mesh,
                transform: t,
                mirrored: t.determinant() < 0.0,
                pass: pass as u8,
                billboard,
                record: r.header_offset,
                clone_of_proxy: None,
                issuing_mesh: index,
            });
        }
    }

    /// Records instance `instance`'s pre-baked colours from `table`: for each submesh whose strip list takes them,
    /// the first set for its vertex group, when it has one record per vertex.
    fn baked(&mut self, instance: u32, scene_mesh: u32, table: &crate::geometry::ColorTable) {
        let data = self.world.data;
        for si in self.out.scene.meshes[scene_mesh as usize].submeshes.clone() {
            let Some(&(group, from_table)) = self.out.submesh_source.get(si as usize) else { continue };
            let n = self.out.scene.submeshes[si as usize].vertex_count;
            let Some(set) = table.sets.iter().find(|s| s.group == group) else { continue };
            if !from_table || set.count as u32 != n {
                continue;
            }
            let start = self.out.baked_colours.len() as u32;
            for k in 0..n as usize {
                let o = set.records_at as usize + k * 5;
                let Some(b) = data.get(o..o + 5) else { return };
                self.out.baked_colours.push(BakedColour {
                    a: u16::from_le_bytes([b[0], b[1]]),
                    b: u16::from_le_bytes([b[2], b[3]]),
                    amount: b[4],
                });
            }
            self.out.baked.insert((instance, si), start);
        }
    }

    /// The scene mesh of `mesh` (built once per mesh in the world data).
    fn scene_mesh(&mut self, grp: usize, r: &Record, index: u32, mesh: &Mesh) -> Option<u32> {
        if let Some(&m) = self.mesh_cache.get(&mesh.offset) {
            return Some(m);
        }
        let data = self.world.data;
        let shaders = self.world.bodies[grp].materials.shaders.len() as i32;
        let base = self.out.material_base[grp];
        let scene = &mut self.out.scene;
        let first_sub = scene.submeshes.len() as u32;
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for (gi, g) in mesh.groups.iter().enumerate() {
            let tris = g.triangles(data);
            if tris.iter().all(|(_, t)| t.is_empty()) {
                continue;
            }
            // Per strip list: its flags (0x20: colours from the colour table).
            let flags: Vec<i32> = match g {
                crate::geometry::Group::Strips { lists, .. } => lists.iter().map(|l| l.flags).collect(),
                _ => Vec::new(),
            };
            let base_vertex = scene.vertices.len() as u32;
            let verts = g.vertices(data);
            for v in &verts {
                for k in 0..3 {
                    lo[k] = lo[k].min(v.position[k]);
                    hi[k] = hi[k].max(v.position[k]);
                }
            }
            scene.vertices.extend_from_slice(&verts);
            for (li, (shader, t)) in tris.into_iter().enumerate() {
                if t.is_empty() || shader < 0 || shader >= shaders {
                    continue;
                }
                let f = flags.get(li).copied().unwrap_or(0);
                self.out.submesh_source.push((gi as u16, f & LIST_TABLE_COLOURS != 0));
                self.out.submesh_flags.push(f);
                let first_index = scene.indices.len() as u32;
                for tri in &t {
                    scene.indices.extend(tri.iter().map(|&i| i as u16));
                }
                let material = base + shader as u32;
                self.used_materials[material as usize] = true;
                scene.submeshes.push(SceneSubmesh {
                    first_index,
                    index_count: (t.len() * 3) as u32,
                    base_vertex,
                    vertex_count: verts.len() as u32,
                    material: material as u16,
                });
            }
        }
        let last_sub = scene.submeshes.len() as u32;
        if last_sub == first_sub {
            return None;
        }
        let (center, radius) = match mesh.sphere {
            Some(s) => s,
            None => {
                let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
                let d = [hi[0] - c[0], hi[1] - c[1], hi[2] - c[2]];
                (c, sqrt_f32(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]))
            }
        };
        let id = scene.meshes.len() as u32;
        scene.meshes.push(SceneMesh {
            record: r.header_offset,
            mesh: index,
            submeshes: first_sub..last_sub,
            center,
            radius,
        });
        self.out.mesh_flags.push(mesh.flags);
        self.mesh_cache.insert(mesh.offset, id);
        Some(id)
    }

    /// Decodes the first frame (palette slot 0) of every used material's shader.
    fn textures(&mut self) {
        let ctx = self.opts.context.clone();
        let grading = if self.opts.grade { Some(&GRADING) } else { None };
        for (grp, body) in self.world.bodies.iter().enumerate() {
            let base = self.out.material_base[grp] as usize;
            let m = &body.materials;
            for (si, s) in m.shaders.iter().enumerate() {
                if !self.used_materials[base + si] {
                    continue;
                }
                let slots = if matches!(s.kind, 1 | 7) { 2 } else { 1 };
                for slot in 0..slots {
                    let Some((bi, Some(pi))) = s.frame(0, slot) else { continue };
                    let (Some(bmp), Some(pal)) = (m.bitmaps().get(bi as usize), m.palettes().get(pi as usize)) else {
                        continue;
                    };
                    if !bmp.filter.matches(&ctx) || !pal.filter.matches(&ctx) {
                        continue;
                    }
                    let key = (grp, bi, pi);
                    let tex = match self.texture_cache.get(&key) {
                        Some(&t) => t,
                        None => {
                            let decoded = textures::decode(
                                bmp,
                                pal,
                                self.world.data,
                                body.texture_bytes(self.world.data),
                                grading,
                            );
                            let t = match decoded {
                                Ok(image) => {
                                    let scene = &mut self.out.scene;
                                    scene.textures.push(SceneTexture {
                                        bitmap: bi as u32,
                                        palette: Some(pi as u32),
                                        image,
                                    });
                                    Some(scene.textures.len() as u32 - 1)
                                }
                                Err(_) => {
                                    self.out.scene.texture_errors.push((bi as u32, Some(pi as u32)));
                                    None
                                }
                            };
                            self.texture_cache.insert(key, t);
                            t
                        }
                    };
                    if slot == 0 {
                        self.out.scene.materials[base + si].texture = tex;
                    } else {
                        self.out.slot1[base + si] = tex;
                    }
                }
            }
        }
    }
}

/// `no_std` square root (Newton's method), enough for a bounding radius.
fn sqrt_f32(v: f32) -> f32 {
    if v <= 0.0 || !v.is_finite() {
        return 0.0;
    }
    let mut x = f32::from_bits((v.to_bits() >> 1) + 0x1FC0_0000);
    for _ in 0..4 {
        x = 0.5 * (x + v / x);
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_root_and_defaults() {
        assert!((sqrt_f32(16.0) - 4.0).abs() < 1e-5);
        assert_eq!(sqrt_f32(-1.0), 0.0);
        let o = SceneOptions::default();
        assert!(o.passes & (1 << 2) != 0 && o.passes & (1 << 6) == 0);
    }
}
