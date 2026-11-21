//! The Sly 2 and Sly 3 characters on the renderer: their skinned meshes (which the scene build leaves out) added
//! with `add_skinned_mesh` and moved every frame with `set_bones`, as the Sly 1 level does for its skins. Sly 3's
//! crew ([`crew_draws`]) is drawn the same way from its records (`formats_sly3`).
//!
//! Only what the game draws as the character is kept: rigid (strips) meshes whose level conditions (Sly 2) or
//! filter (Sly 3) admit the world, in a drawn pass with a drawn shader kind. Shadow hulls and projected volumes
//! (kinds 2 and 3, passes 7, 8, 11 and 13) and outline bodies are left out; the caller decides per draw from
//! [`SkinnedDraw::pass`] and the scene material. The caller lights them as moving objects
//! every frame: [`CharacterSkins::update`] puts a light block (`stage::light::light_block`) after each part's bone
//! matrices and the vertex shader computes the two colours from it (`Renderer::set_shader_light`).
//!
//! For each character object (classes 53 to 55) every drawn mesh of its mesh set (`formats_sly2::geometry`, rigid
//! body type 0; types 1 and 2 carry only outline hulls or streamed data) becomes one renderer mesh per vertex group
//! and strip list, with its bone weights when the mesh has a bone table (geometry.md 5); a mesh without bones
//! follows its transform. The scene build's static draws of the characters are left out of the static level. Its bones are the
//! mesh's bone table: (object ID, transform reference), resolved as the object's world matrix times its loaded
//! transform list entry (`Sly2Level::pose_of`). The palette (`engine::anim::sly2::SkinPalette`) maps the mesh's
//! bind pose to the bones' current matrices; the instances are placed at the identity, so the bone matrices carry the
//! whole world placement. Without clips playing on the characters the pose is the loaded one, moved with the
//! character's object as the gang moves.

use alloc::vec::Vec;

use engine::anim::sly2::SkinPalette;
use engine::object::Handle;
use engine::world::World;
use formats::geometry::{Affine, GpuVertex};
use formats_sly2::stream::Body;
use render::{Backend, InstanceId, MaterialId, MeshData, RenderError, Renderer, SkinVertex, Vertex};

use crate::level::{bone_rows, mat4};

/// Character classes.
pub const CHARACTERS: [i16; 3] = [53, 54, 55];

/// One skinned draw waiting for upload: its vertices, indices, influences, scene material and bones.
#[derive(Clone)]
pub struct SkinnedDraw {
    /// The character object.
    pub object: Handle,
    /// Vertices.
    pub vertices: Vec<Vertex>,
    /// Triangle indices.
    pub indices: Vec<u16>,
    /// Influences, one per vertex.
    pub skin: Vec<SkinVertex>,
    /// Scene material index.
    pub material: u32,
    /// The mesh's bone table.
    pub bones: Vec<(i32, i16)>,
    /// The mesh's transform reference.
    pub transform: i16,
    /// The mesh's render pass, in Sly 2's numbering (Sly 3's plus one).
    pub pass: u8,
    /// The vertices as stored (position, normal, colour) for lighting.
    pub source: Vec<GpuVertex>,
    /// Sly 2 / Sly 3 pose hook: a gameplay-state accessory, hidden until its state shows it. Set only where the
    /// specs identify one: Sly 2 Sly's paraglider canopy and strings (characters.md 9.3, meshes on his transforms 3
    /// to 8), shown while `Sly2Poser::gliding` holds him. Sly 3's accessory records (characters.md 12.3,
    /// `engine::anim::sly3_pose::ACCESSORY_RECORDS`) name no meshes or states yet, so none is set there.
    pub accessory: bool,
}

/// A transform of an object: its world matrix times its loaded transform-list entry `t` (`t` < 0: the object's).
pub fn transform_of(world: &World, h: Handle, t: i16) -> Option<Affine> {
    let w = world.world_transform(h)?;
    let pose = match (world.sly2.as_deref(), world.sly3.as_deref()) {
        (Some(l), _) => l.pose_of(h.index()),
        (None, Some(l)) => l.pose_of(h.index()),
        _ => &[],
    };
    Some(match usize::try_from(t).ok().and_then(|t| pose.get(t)) {
        Some(p) => w.mul(p),
        None => w,
    })
}

/// The sphere around a mesh's vertices (centre of the box, farthest vertex).
fn rest_sphere(v: &[Vertex]) -> ([f32; 3], f32) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in v {
        for k in 0..3 {
            lo[k] = lo[k].min(p.pos[k]);
            hi[k] = hi[k].max(p.pos[k]);
        }
    }
    if v.is_empty() {
        return ([0.0; 3], 0.0);
    }
    let c = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5];
    let r2 =
        v.iter().map(|p| (0..3).map(|k| (p.pos[k] - c[k]) * (p.pos[k] - c[k])).sum::<f32>()).fold(0.0f32, f32::max);
    (c, render::math::sqrt(r2))
}

/// A world-space sphere around everything the skinning matrices make of the rest sphere `(c, r)`: centred on the
/// mean of the moved centres, reaching each moved centre plus the radius stretched by that matrix.
fn posed_sphere((c, r): ([f32; 3], f32), matrices: &[Affine]) -> (render::math::Vec3, f32) {
    use render::math::Vec3;
    if matrices.is_empty() {
        return (Vec3::new(c[0], c[1], c[2]), r);
    }
    let moved: Vec<Vec3> = matrices
        .iter()
        .map(|m| {
            let p = m.transform_point(c);
            Vec3::new(p[0], p[1], p[2])
        })
        .collect();
    let centre = moved.iter().fold(Vec3::ZERO, |a, &p| a + p) * (1.0 / moved.len() as f32);
    let mut radius = 0.0f32;
    for (m, p) in matrices.iter().zip(&moved) {
        let stretch = m.cols.iter().map(|v| Vec3::new(v[0], v[1], v[2]).length()).fold(0.0f32, f32::max);
        radius = radius.max((*p - centre).length() + r * stretch);
    }
    (centre, radius)
}

/// The object with ID `id` in the same copy as `context` (Sly 2 or Sly 3).
fn find_id(world: &World, id: i32, context: Handle) -> Option<Handle> {
    if world.sly2.is_some() { world.find_sly2_id(id, Some(context)) } else { world.find_sly3_id(id, Some(context)) }
}

/// Influences of `n` vertices from per-vertex (slot, weight) rows: `slots` are bone-table indices, `weight(v, k)`
/// the weight of slot `k` at vertex `v` (0..255).
fn influences(n: usize, slots: &[u8], weight: impl Fn(usize, usize) -> u8) -> Vec<SkinVertex> {
    (0..n)
        .map(|v| {
            let mut bones = [0u8; 4];
            let mut ws = [0.0f32; 4];
            for (k, &slot) in slots.iter().take(4).enumerate() {
                bones[k] = slot;
                ws[k] = f32::from(weight(v, k)) / 255.0;
            }
            SkinVertex::new(bones, ws)
        })
        .collect()
}

/// One draw per strip list with triangles.
#[allow(clippy::too_many_arguments)]
fn push_lists(
    out: &mut Vec<SkinnedDraw>,
    h: Handle,
    verts: &[GpuVertex],
    skin: &[SkinVertex],
    lists: Vec<(i32, Vec<[u8; 3]>)>,
    base: u32,
    bones: &[(i32, i16)],
    transform: i16,
    pass: u8,
) {
    let vertices: Vec<Vertex> = verts.iter().map(|v| Vertex::new(v.position, v.uv, v.normal, v.color)).collect();
    for (shader, tris) in lists {
        if tris.is_empty() || shader < 0 {
            continue;
        }
        out.push(SkinnedDraw {
            object: h,
            vertices: vertices.clone(),
            indices: tris.iter().flatten().map(|&i| u16::from(i)).collect(),
            skin: skin.to_vec(),
            material: base + shader as u32,
            bones: bones.to_vec(),
            transform,
            pass,
            source: verts.to_vec(),
            accessory: false, // Sly 2 / Sly 3 pose hook: no accessory identified by the specs yet.
        });
    }
}

/// The skinned draws of the Sly 3 crew (classes 52 to 60, `engine::gang::sly3::Member`) in `world`;
/// `material_base` is the scene build's first material per group.
pub fn crew_draws(world: &World, material_base: &[u32]) -> Vec<SkinnedDraw> {
    use formats_sly3::geometry::{Group, STRIPS};
    use formats_sly3::stream::Body as Body3;
    let mut out = Vec::new();
    let data = &world.level.bytes;
    let Some(level) = world.sly3.as_deref() else { return out };
    for h in world.handles() {
        if !world.class(h).is_some_and(|c| engine::gang::sly3::Member::from_class(c).is_some()) {
            continue;
        }
        let Some(r) = world.sly3_record(h) else { continue };
        let Body3::Solid(s) = &r.body else { continue };
        let base = world.sly3_object(h).and_then(|o| material_base.get(o.group as usize)).copied().unwrap_or(0);
        for m in s.meshes.meshes.iter().filter(|m| m.body_type == STRIPS && !m.hidden) {
            if !m.filter.matches(&level.context) {
                continue;
            }
            let pass = m.pass.max(0) as u8;
            for g in &m.groups {
                let Group::Strips { weights, .. } = g else { continue };
                let verts = g.vertices(data);
                if verts.is_empty() {
                    continue;
                }
                let skin = match *weights {
                    Some((slots_at, at, k)) if !m.bones.is_empty() => {
                        let slots = data.get(slots_at as usize..slots_at as usize + k as usize).unwrap_or(&[]);
                        let k = k as usize;
                        influences(verts.len(), slots, |v, j| data.get(at as usize + v * k + j).copied().unwrap_or(0))
                    }
                    _ => alloc::vec![SkinVertex::NONE; verts.len()],
                };
                // Sly 3's passes are Sly 2's minus one.
                let pass2 = pass.saturating_add(1);
                push_lists(&mut out, h, &verts, &skin, g.triangles(data), base, &m.bones, m.transform, pass2);
            }
        }
    }
    out
}

/// The skinned draws of the characters in `world`; `material_base` is the scene build's first material per package.
pub fn character_draws(world: &World, material_base: &[u32]) -> Vec<SkinnedDraw> {
    let mut out = Vec::new();
    let data = &world.level.bytes;
    for h in world.handles() {
        if !world.class(h).is_some_and(|c| CHARACTERS.contains(&c)) {
            continue;
        }
        let Some(r) = world.sly2_record(h) else { continue };
        let Body::Solid(s) = &r.body else { continue };
        let base = world.sly2_object(h).and_then(|o| material_base.get(o.package as usize)).copied().unwrap_or(0);
        // Per-row variants (costumes): only the meshes whose level conditions admit this row.
        let row = world.info.index as i16;
        for m in s.meshes.meshes.iter().filter(|m| m.body_type == 0) {
            if !m.conditions.is_empty() && !m.conditions.contains(&row) {
                continue;
            }
            let pass = m.pass.max(0) as u8;
            for g in &m.groups {
                let verts = g.vertices(data);
                if verts.is_empty() {
                    continue;
                }
                let skin = match g.weights(data, m.bones.len()) {
                    Some(w) => influences(verts.len(), &w.slots, |v, k| w.vertex(v).get(k).copied().unwrap_or(0)),
                    None => alloc::vec![SkinVertex::NONE; verts.len()],
                };
                let before = out.len();
                push_lists(&mut out, h, &verts, &skin, g.triangles(data), base, &m.bones, m.transform, pass);
                // Sly 2 pose hook: Sly's paraglider meshes (characters.md 9.3).
                if glider_mesh(world, h, m) {
                    out[before..].iter_mut().for_each(|d| d.accessory = true);
                }
            }
        }
    }
    out
}

/// Sly 2 pose hook: whether a mesh of a character is Sly's paraglider (canopy, strings and their outline and shadow):
/// a mesh of Sly's body (class 53) placed on, or weighted to, his transforms 3 to 8 (characters.md 9.3).
pub fn glider_mesh(world: &World, h: Handle, m: &formats_sly2::geometry::Mesh) -> bool {
    const GLIDER: core::ops::RangeInclusive<i16> = 3..=8;
    let Some(id) = world.sly2_id(h) else { return false };
    world.class(h) == Some(CHARACTERS[0])
        && (GLIDER.contains(&m.transform) || m.bones.iter().any(|&(o, t)| o == id && GLIDER.contains(&t)))
}

/// One placed skinned instance: the renderer instance, the character, its palette, bone table and mesh transform.
struct Part {
    id: InstanceId,
    object: Handle,
    palette: SkinPalette,
    /// The bone table resolved once at placement: each bone's object and transform reference.
    bones: Vec<Option<(Handle, i16)>>,
    transform: i16,
    /// The mesh's rest sphere in mesh space (centre, radius).
    sphere: ([f32; 3], f32),
    /// Lit in the shader as a moving object.
    light: Option<PartLight>,
    /// The character's world matrix, visibility and pose version (the sum of its own and its bone objects'
    /// `Sly2Poser` versions) when the bones were last given: nothing to do while unchanged.
    last: Option<(Affine, bool, u32)>,
    /// Sly 2 pose hook: an accessory, drawn only while its state shows it ([`SkinnedDraw::accessory`]).
    accessory: bool,
}

/// The characters' skinned instances.
#[derive(Default)]
pub struct CharacterSkins {
    parts: Vec<Part>,
    matrices: Vec<Affine>,
    rows: Vec<render::BoneMatrix>,
    /// The world's lights: parts with a [`PartLight`] are relit in the vertex shader as they move.
    lighting: Option<stage::light::WorldLighting>,
}

/// How a part is lit as a moving object (rendering.md 2.3): its material colour, self-illumination, and whether it
/// draws colour A alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartLight {
    pub color: [u8; 4],
    pub illum: u16,
    pub one_pass: bool,
}

impl CharacterSkins {
    /// Adds the meshes (call before `commit`); `material` turns a scene material index into a renderer material.
    pub fn add_meshes<B: Backend>(
        r: &mut Renderer<B>,
        draws: &[SkinnedDraw],
    ) -> Result<Vec<render::MeshId>, RenderError> {
        draws
            .iter()
            .map(|d| r.add_skinned_mesh(&MeshData { vertices: &d.vertices, indices: &d.indices }, &d.skin))
            .collect()
    }

    /// Places the instances (after `commit`) with their materials (`material` of the draw's index, `None` leaves it
    /// out) and vertex weights, and builds the palettes from the loaded pose.
    pub fn place<B: Backend>(
        r: &mut Renderer<B>,
        world: &World,
        draws: &[SkinnedDraw],
        meshes: &[render::MeshId],
        material: impl Fn(usize) -> Option<MaterialId>,
        lighting: Option<&stage::light::WorldLighting>,
        light: &[Option<PartLight>],
    ) -> Result<CharacterSkins, RenderError> {
        let mut out = CharacterSkins { lighting: lighting.cloned(), ..CharacterSkins::default() };
        for (i, (d, &m)) in draws.iter().zip(meshes).enumerate() {
            let Some(mat) = material(i) else { continue };
            let id = r.add_instance(m, mat, mat4(&Affine::IDENTITY))?;
            let light = light.get(i).copied().flatten().filter(|_| lighting.is_some());
            let mesh_bind = transform_of(world, d.object, d.transform).unwrap_or(Affine::IDENTITY);
            let palette = SkinPalette::new(&d.bones, mesh_bind, |o, t| {
                find_id(world, o, d.object).and_then(|h| transform_of(world, h, t))
            });
            let bones = d.bones.iter().map(|&(o, t)| find_id(world, o, d.object).map(|h| (h, t))).collect();
            let sphere = rest_sphere(&d.vertices);
            out.parts.push(Part {
                id,
                object: d.object,
                palette,
                bones,
                transform: d.transform,
                sphere,
                light,
                last: None,
                accessory: d.accessory, // Sly 2 pose hook
            });
        }
        Ok(out)
    }

    /// Instances placed.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether none were placed.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Gives every instance this frame's bone matrices from the characters' current matrices.
    pub fn update<B: Backend>(&mut self, r: &mut Renderer<B>, world: &World) -> Result<(), RenderError> {
        for Part { id, object, palette, bones, transform, sphere, light, last, accessory } in &mut self.parts {
            // The bones move with the character's object and with the clips posing it or its bone objects.
            let version = world.sly2.as_deref().map_or(0, |l| {
                let v = |h: Handle| world.slot(h).map_or(0, |s| l.poser.version_of(s));
                bones.iter().flatten().fold(v(*object), |a, &(h, _)| a.wrapping_add(v(h)))
            });
            // Sly 3 pose hook: the crew's poses (`engine::anim::sly3_pose`) version the same way.
            let version = version.wrapping_add(world.sly3.as_deref().map_or(0, |l| {
                let v = |h: Handle| world.slot(h).map_or(0, |s| l.poser.version_of(s));
                bones.iter().flatten().fold(v(*object), |a, &(h, _)| a.wrapping_add(v(h)))
            }));
            // Sly 2 pose hook: an accessory shows only while its state does (the paraglider while gliding).
            let shown = !*accessory
                || world.sly2.as_deref().zip(world.slot(*object)).is_some_and(|(l, s)| l.poser.gliding.contains(&s));
            let now =
                (world.world_transform(*object).unwrap_or(Affine::IDENTITY), world.in_world(*object) && shown, version);
            if *last == Some(now) {
                continue;
            }
            *last = Some(now);
            let mesh = transform_of(world, *object, *transform).unwrap_or(Affine::IDENTITY);
            r.set_visible(*id, now.1)?;
            // The light block of a lit part: the four strongest lights at the character's place (it moved), after the
            // bone matrices; the vertex shader computes the two colours from it.
            let block = match (light.as_ref(), self.lighting.as_ref()) {
                (Some(p), Some(l)) => {
                    let c = now.0.t;
                    let rows = stage::light::light_block(
                        l,
                        render::math::Vec3::new(c[0], c[1], c[2]),
                        p.color,
                        p.illum,
                        p.one_pass,
                    );
                    Some(rows)
                }
                _ => None,
            };
            let push_block = |rows: &mut Vec<render::BoneMatrix>| {
                if let Some(b) = &block {
                    for m in b.chunks(3) {
                        rows.push(render::BoneMatrix { rows: [m[0], m[1], m[2]] });
                    }
                }
            };
            if palette.is_empty() {
                // No bones: the mesh follows its transform (a lit part still needs a bone list for its light block:
                // one identity bone, which its unweighted vertices ignore).
                r.set_transform(*id, mat4(&mesh))?;
                if block.is_some() {
                    self.rows.clear();
                    self.rows.push(render::BoneMatrix::IDENTITY);
                    push_block(&mut self.rows);
                    r.set_bones(*id, &self.rows)?;
                    r.set_shader_light(*id, Some(1))?;
                }
                continue;
            }
            palette.matrices(
                &mesh,
                |i| {
                    let (h, t) = (*bones.get(i)?)?;
                    transform_of(world, h, t)
                },
                &mut self.matrices,
            );
            // Culling: the instance sits at the identity, so its sphere must follow the posed mesh.
            r.set_bounding_sphere(*id, Some(posed_sphere(*sphere, &self.matrices)))?;
            self.rows.clear();
            self.rows.extend(self.matrices.iter().map(bone_rows));
            let n = self.rows.len() as u32;
            // The light block rides after the bones only while both fit the renderer's bone limit; a part with
            // more bones keeps its weights' colours.
            let lit = block.is_some()
                && self.rows.len() + render::frame::LIGHT_BLOCK_MATRICES as usize <= render::MAX_BONES_PER_INSTANCE;
            if lit {
                push_block(&mut self.rows);
            }
            r.set_bones(*id, &self.rows)?;
            r.set_shader_light(*id, lit.then_some(n))?;
        }
        Ok(())
    }
}
