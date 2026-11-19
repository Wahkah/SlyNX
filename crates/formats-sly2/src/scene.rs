//! Builds a `formats::scene::Scene` from a Sly 2 world, so the Sly 1 viewer and renderer can draw it.
//!
//! What is drawn (classes/placement.md 2, geometry.md 2.2 and 9, object-stream.md 1.4 and 2.3):
//!
//! - packages with the keep flag set (the level package and a few others) are drawn where they are stored;
//! - every other package is a template, drawn once per placement that names it (classes 6 and 7), at the
//!   placement's matrix; a copy placement's extra objects are drawn with it, relative to its matrix;
//! - records and meshes whose level conditions exclude the chosen level row are left out, and so are objects and
//!   placements marked "remove at start" (option 26);
//! - rigid and indexed meshes are drawn; skinned and streamed meshes carry only outline hulls and are not;
//! - a mesh is drawn with its transform's world matrix (a copy: its own matrix first), with the transform list's
//!   world matrices composed as loaded (the bind pose).
//!
//! `SceneInstance::record` and `SceneMesh::record` hold the record's header offset in the world data (Sly 2 has no
//! world-wide record numbering); `SceneInstance::issuing_mesh` is the mesh's index in that record's mesh set.
//!
//! Materials are numbered across packages: material `m` is shader `m - base` of the package whose base it falls
//! in ([`SceneBuild::material_base`]).

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use formats::geometry::Affine;
use formats::scene::{Scene, SceneInstance, SceneMaterial, SceneMesh, SceneSubmesh, SceneTexture};

use crate::geometry::{INDEXED, Mesh, RIGID};
use crate::stream::{Body, Record};
use crate::textures::{self, MaterialTables};
use crate::transforms::resolve;
use crate::world::World;

/// Options for [`build`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneOptions {
    /// The level row the world is shown as (selects conditional records, meshes and textures); `None` keeps
    /// everything.
    pub row: Option<i16>,
    /// Decode textures.
    pub decode_textures: bool,
    /// Render passes to include (bit `p` for pass `p`). Default: 1, 3, 4, 5, 8 and 10 (geometry.md 9).
    pub passes: u32,
    /// Deepest chain of placements followed.
    pub max_depth: u32,
}

impl Default for SceneOptions {
    fn default() -> Self {
        SceneOptions {
            row: None,
            decode_textures: true,
            passes: (1 << 1) | (1 << 3) | (1 << 4) | (1 << 5) | (1 << 8) | (1 << 10),
            max_depth: 8,
        }
    }
}

/// The scene and how materials map back to packages.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneBuild {
    pub scene: Scene,
    /// First material index of each package (by package index).
    pub material_base: Vec<u32>,
    /// Placements whose set ID names no package of the world.
    pub unresolved_placements: u32,
    /// Per material: the texture of frame 0, palette slot 1 (kinds 1 and 7: the grey palette; rendering.md 3),
    /// decoded with the others.
    pub slot1: Vec<Option<u32>>,
    /// Per material: its shader's self-illumination (bytes 20 and 21).
    pub self_illum: Vec<u16>,
    /// Per material: its shader's second colour (kind 3's constant colour, rendering.md 3.5).
    pub color2: Vec<[u8; 4]>,
    /// The lights (class 4) of the drawn objects, in world space (effects.md 2).
    pub lights: Vec<SceneLight>,
    /// Per scene mesh: its body's flags byte (bit 0: lit per frame, rendering.md 2.3).
    pub mesh_flags: Vec<u8>,
    /// Per scene submesh: its strip list's flags (`0x20`: colours from the pre-baked table, rendering.md 2.3).
    pub submesh_flags: Vec<i32>,
    /// Pre-baked colours (geometry.md 4.5, rendering.md 2.5): for (scene instance, submesh) whose vertex-map table
    /// (the mesh's, or its copy's) gives the submesh a set with one record per vertex, the first of its records in
    /// [`SceneBuild::baked_colours`].
    pub baked: BTreeMap<(u32, u32), u32>,
    pub baked_colours: Vec<BakedColour>,
    /// Texture animations of the used materials (rendering.md 4).
    pub anims: Vec<SceneAnim>,
}

/// First and last texture-animation classes that change frames (loop, ping-pong, random frame, eyes).
pub const ANIM_LOOP: u16 = 139;
pub const ANIM_PING_PONG: u16 = 140;
pub const ANIM_RANDOM: u16 = 141;
pub const ANIM_EYES: u16 = 142;
/// UV scrolling and UV circle.
pub const ANIM_SCROLL: u16 = 143;
pub const ANIM_CIRCLE: u16 = 144;

/// A material's texture animation (rendering.md 4): the node record's class and fields, and for the frame classes
/// each frame's textures (slot 0, and slot 1 for kinds 1 and 7).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneAnim {
    pub material: u32,
    pub class: u16,
    /// Record +4.
    pub flags: u32,
    /// Record +12 to +24.
    pub params: [f32; 4],
    /// Record +28 (first frame, or the eyes' second shader id).
    pub extra: i32,
    pub frames: Vec<(Option<u32>, Option<u32>)>,
}

/// One pre-baked colour record (Sly 2 geometry.md 4.5, Sly 3 geometry.md 4.2): colours A and B (15-bit 5-5-5-1) and
/// B's alpha (Sly 3: the amount).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BakedColour {
    pub a: u16,
    pub b: u16,
    pub amount: u8,
}

/// A 15-bit colour as RGBA8 (5 bits per channel shifted left by 3; top bit alpha 0x80).
pub fn color15(c: u16) -> [u8; 4] {
    let ch = |s: u16| (((c >> s) & 0x1F) << 3) as u8;
    [ch(0), ch(5), ch(10), if c & 0x8000 != 0 { 0x80 } else { 0 }]
}

/// Class of lights.
pub const LIGHT_CLASS: i16 = 4;

/// A light (effects.md 2), its options with their defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SceneLight {
    /// World matrix: a directional light shines along its first axis.
    pub matrix: Affine,
    /// Option 151: 0 directional, 1 point.
    pub kind: u32,
    /// Option 152: hue 0..360, saturation 0..255, value 0..255.
    pub hsv: [f32; 3],
    /// Option 153: strength 0..255.
    pub strength: f32,
    /// Option 154: cone angle in degrees.
    pub angle: f32,
    /// Option 155: full at the first distance, none at the second.
    pub range: [f32; 2],
    /// Option 157: not offered to moving objects.
    pub not_moving: bool,
    /// Option 158: not used by the static bake.
    pub not_baked: bool,
}

/// Option ids of a light's fields: kind, HSV colour, strength, cone angle, fall-off range, not offered to moving
/// objects, not baked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightOptions(pub [i16; 7]);

impl LightOptions {
    /// Sly 2's: 151 to 155, 157, 158 (effects.md 2).
    pub const SLY2: LightOptions = LightOptions([151, 152, 153, 154, 155, 157, 158]);
    /// Sly 3's: 159 to 165 (its rendering.md 2.1).
    pub const SLY3: LightOptions = LightOptions([159, 160, 161, 162, 163, 164, 165]);
}

impl SceneLight {
    // Option 156 (direction of cast shadows) is not read: it is 0 on every light on the disc and drives shadows only.

    /// The light described by `options` (defaults for missing ones).
    pub fn read(matrix: Affine, data: &[u8], options: &crate::options::OptionList) -> SceneLight {
        SceneLight::from_values(matrix, LightOptions::SLY2, |id| options.value(data, id))
    }

    /// The light whose option values (raw bytes) `value` gives, with the option ids `ids`.
    pub fn from_values<'a>(matrix: Affine, ids: LightOptions, value: impl Fn(i16) -> Option<&'a [u8]>) -> SceneLight {
        let [kind, hsv, strength, angle, range, not_moving, not_baked] = ids.0;
        let f = |id: i16, k: usize| {
            value(id).and_then(|v| v.get(k * 4..k * 4 + 4)).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        // Enumerations and flags: 1, 2 or 4 bytes.
        let u = |id: i16| {
            value(id)
                .filter(|v| matches!(v.len(), 1 | 2 | 4))
                .map(|v| v.iter().rev().fold(0u32, |a, &b| (a << 8) | b as u32))
        };
        SceneLight {
            matrix,
            kind: u(kind).unwrap_or(0),
            hsv: [f(hsv, 0).unwrap_or(0.0), f(hsv, 1).unwrap_or(0.0), f(hsv, 2).unwrap_or(0.0)],
            strength: f(strength, 0).unwrap_or(0.0),
            angle: f(angle, 0).unwrap_or(240.0),
            range: [f(range, 0).unwrap_or(200.0), f(range, 1).unwrap_or(2000.0)],
            not_moving: u(not_moving).unwrap_or(0) != 0,
            not_baked: u(not_baked).unwrap_or(0) != 0,
        }
    }
}

struct Builder<'w, 'a> {
    world: &'w World<'a>,
    opts: SceneOptions,
    out: SceneBuild,
    mesh_cache: BTreeMap<u32, u32>,
    texture_cache: BTreeMap<(usize, u16, u16), Option<u32>>,
    used_materials: Vec<bool>,
    set_index: BTreeMap<i32, usize>,
    /// Per scene mesh: each submesh's strip-list index across the body (the vertex-map table's numbering).
    submesh_index: BTreeMap<u32, Vec<u32>>,
}

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
            self_illum.push(s.self_illum);
            color2.push(s.color2);
        }
    }
    let slot1 = alloc::vec![None; materials.len()];
    let used = alloc::vec![false; materials.len()];
    let set_index = world.header.packages.iter().enumerate().map(|(i, p)| (p.set_id, i)).collect();
    let mut b = Builder {
        world,
        opts,
        out: SceneBuild {
            scene: Scene { materials, ..Scene::default() },
            material_base: base,
            unresolved_placements: 0,
            slot1,
            self_illum,
            color2,
            lights: Vec::new(),
            mesh_flags: Vec::new(),
            submesh_flags: Vec::new(),
            baked: BTreeMap::new(),
            baked_colours: Vec::new(),
            anims: Vec::new(),
        },
        mesh_cache: BTreeMap::new(),
        texture_cache: BTreeMap::new(),
        used_materials: used,
        set_index,
        submesh_index: BTreeMap::new(),
    };
    for (i, p) in world.header.packages.iter().enumerate() {
        if p.keep != 0 {
            b.package(i, &Affine::IDENTITY, 0);
        }
    }
    if opts.decode_textures {
        b.textures();
    }
    b.out
}

impl Builder<'_, '_> {
    fn package(&mut self, index: usize, root: &Affine, depth: u32) {
        let objects = &self.world.bodies[index].objects;
        self.records(index, objects, root, &[], depth);
    }

    fn records(&mut self, pkg: usize, records: &[Record], root: &Affine, enclosing: &[Affine], depth: u32) {
        for r in records {
            if !r.loaded_for(self.opts.row) || r.removed_at_start() {
                continue;
            }
            match &r.body {
                Body::Solid(s) => {
                    let world = s.transforms.world_matrices(root, enclosing);
                    for (mi, mesh) in s.meshes.meshes.iter().enumerate() {
                        if !mesh.conditions.is_empty()
                            && self.opts.row.is_some_and(|row| !mesh.conditions.contains(&row))
                        {
                            continue;
                        }
                        self.mesh(pkg, r, mi as u32, mesh, &world, enclosing, root);
                    }
                    self.records(pkg, &s.children, root, &world, depth);
                }
                Body::Placement(p) => {
                    if p.remove_at_start || depth >= self.opts.max_depth {
                        continue;
                    }
                    let parent = resolve(p.parent, &[], enclosing, root);
                    let at = parent.mul(&p.matrix);
                    match self.set_index.get(&p.set_id).copied() {
                        Some(target) => self.package(target, &at, depth + 1),
                        None => self.out.unresolved_placements += 1,
                    }
                    self.records(pkg, &p.extra, &at, &[], depth + 1);
                }
                Body::Other { header: Some(h), options: Some(o), .. } if r.class == LIGHT_CLASS => {
                    let at = resolve(h.parent, &[], enclosing, root).mul(&h.matrix);
                    // Only kinds 0 (directional) and 1 (point) are lights (effects.md 2).
                    let light = SceneLight::read(at, self.world.data, o);
                    if light.kind <= 1 {
                        self.out.lights.push(light);
                    }
                }
                Body::Other { .. } | Body::Unparsed => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn mesh(
        &mut self,
        pkg: usize,
        r: &Record,
        index: u32,
        mesh: &Mesh,
        own: &[Affine],
        enclosing: &[Affine],
        root: &Affine,
    ) {
        if !(mesh.body_type == RIGID || mesh.body_type == INDEXED) {
            return;
        }
        let pass = mesh.pass.max(0) as u32;
        if pass >= 32 || self.opts.passes & (1 << pass) == 0 {
            return;
        }
        let Some(scene_mesh) = self.scene_mesh(pkg, r, index, mesh) else { return };
        let base = resolve(mesh.transform, own, enclosing, root);
        let mut draws = alloc::vec![(base, mesh.vertex_map.clone())];
        for c in &mesh.copies {
            let t = resolve(c.transform, own, enclosing, root);
            draws.push((t.mul(&c.matrix), crate::geometry::VertexMap::read_at(self.world.data, c.vertex_map_at)));
        }
        for (t, map) in draws {
            let instance = self.out.scene.instances.len() as u32;
            self.baked(instance, scene_mesh, &map);
            self.out.scene.instances.push(SceneInstance {
                mesh: scene_mesh,
                transform: t,
                mirrored: t.determinant() < 0.0,
                pass: pass as u8,
                billboard: 0,
                record: r.header_offset,
                clone_of_proxy: None,
                issuing_mesh: index,
            });
        }
    }

    /// Records instance `instance`'s pre-baked colours from `map`: each submesh whose set has one record per vertex.
    fn baked(&mut self, instance: u32, scene_mesh: u32, map: &crate::geometry::VertexMap) {
        if map.sets.is_empty() {
            return;
        }
        let data = self.world.data;
        let subs = self.out.scene.meshes[scene_mesh as usize].submeshes.clone();
        let source = self.submesh_index.get(&scene_mesh).cloned().unwrap_or_default();
        for (si, k) in subs.zip(source) {
            let n = self.out.scene.submeshes[si as usize].vertex_count;
            let Some((count, at)) = map.set_of(k as usize) else { continue };
            if count as u32 != n {
                continue;
            }
            let start = self.out.baked_colours.len() as u32;
            for v in 0..n as usize {
                let o = at as usize + v * 5;
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
    fn scene_mesh(&mut self, pkg: usize, r: &Record, index: u32, mesh: &Mesh) -> Option<u32> {
        if let Some(&m) = self.mesh_cache.get(&mesh.offset) {
            return Some(m);
        }
        let data = self.world.data;
        let shaders = self.world.bodies[pkg].materials.shaders.len() as i32;
        let base = self.out.material_base[pkg];
        let scene = &mut self.out.scene;
        let first_sub = scene.submeshes.len() as u32;
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        // Strip lists counted across the body, and each kept submesh's.
        let mut list = 0u32;
        let mut indices = Vec::new();
        for g in &mesh.groups {
            let tris = g.triangles(data);
            let flags: Vec<i32> = match g {
                crate::geometry::Group::Rigid { lists, .. } => lists.iter().map(|l| l.flags).collect(),
                _ => Vec::new(),
            };
            let first_list = list;
            list += tris.len() as u32;
            if tris.iter().all(|(_, t)| t.is_empty()) {
                continue;
            }
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
                indices.push(first_list + li as u32);
                self.out.submesh_flags.push(flags.get(li).copied().unwrap_or(0));
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
        self.out.mesh_flags.push(mesh.body_flags);
        self.submesh_index.insert(id, indices);
        self.mesh_cache.insert(mesh.offset, id);
        Some(id)
    }

    /// Decodes the first frame of every used material's shader: palette slot 0, and slot 1 for kinds 1 and 7.
    fn textures(&mut self) {
        for (pkg, body) in self.world.bodies.iter().enumerate() {
            let base = self.out.material_base[pkg] as usize;
            let m: &MaterialTables = &body.materials;
            for (si, s) in m.shaders.iter().enumerate() {
                if !self.used_materials[base + si] {
                    continue;
                }
                let slots = if matches!(s.kind, 1 | 7) { 2 } else { 1 };
                self.out.scene.materials[base + si].texture = self.frame_texture(pkg, s, 0, 0);
                if slots == 2 {
                    self.out.slot1[base + si] = self.frame_texture(pkg, s, 0, 1);
                }
                // Texture animation (rendering.md 4): the shader's node record and, for frame-changing classes,
                // every frame's images.
                let Some(&(class, at)) = usize::try_from(s.node).ok().and_then(|n| m.nodes.get(n)) else { continue };
                let Some(raw) = self.world.data.get(at as usize..at as usize + 32) else { continue };
                let f = |o: usize| f32::from_le_bytes([raw[o], raw[o + 1], raw[o + 2], raw[o + 3]]);
                let count = u16::from_le_bytes([raw[10], raw[11]]) as usize;
                let mut frames = Vec::new();
                if (ANIM_LOOP..=ANIM_EYES).contains(&class) {
                    for k in 0..count.min(s.frames.len()) {
                        let t = self.frame_texture(pkg, s, k, 0);
                        let g = if slots == 2 { self.frame_texture(pkg, s, k, 1) } else { None };
                        frames.push((t, g));
                    }
                }
                self.out.anims.push(SceneAnim {
                    material: (base + si) as u32,
                    class,
                    flags: u32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
                    params: [f(12), f(16), f(20), f(24)],
                    extra: i32::from_le_bytes([raw[28], raw[29], raw[30], raw[31]]),
                    frames,
                });
            }
        }
    }

    /// The decoded image of frame `frame`, palette slot `slot` of shader `s` (decoded once per bitmap and palette).
    fn frame_texture(&mut self, pkg: usize, s: &crate::textures::Shader, frame: usize, slot: usize) -> Option<u32> {
        let row = self.opts.row;
        let body = &self.world.bodies[pkg];
        let m: &MaterialTables = &body.materials;
        let (bi, Some(pi)) = s.frame(frame, slot)? else { return None };
        let (bmp, pal) = (m.bitmaps.get(bi as usize)?, m.palettes.get(pi as usize)?);
        if !MaterialTables::loaded(&bmp.conditions, row) || !MaterialTables::loaded(&pal.conditions, row) {
            return None;
        }
        let key = (pkg, bi, pi);
        if let Some(&t) = self.texture_cache.get(&key) {
            return t;
        }
        let t = match textures::decode(bmp, pal, self.world.data, body.texture_bytes(self.world.data)) {
            Ok(image) => {
                let scene = &mut self.out.scene;
                scene.textures.push(SceneTexture { bitmap: bi as u32, palette: Some(pi as u32), image });
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
