//! The Sly 2 characters on the renderer: their skinned meshes (which the scene build leaves out) added with
//! `add_skinned_mesh` and moved every frame with `set_bones`, as the Sly 1 level does for its skins.
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
use formats::geometry::Affine;
use formats_sly2::stream::Body;
use render::{Backend, InstanceId, MaterialId, MeshData, RenderError, Renderer, SkinVertex, Vertex};

use crate::level::{bone_rows, mat4};

/// Character classes.
pub const CHARACTERS: [i16; 3] = [53, 54, 55];

/// One skinned draw waiting for upload: its vertices, indices, influences, scene material and bones.
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
}

/// A transform of an object: its world matrix times its loaded transform-list entry `t` (`t` < 0: the object's).
fn transform_of(world: &World, h: Handle, t: i16) -> Option<Affine> {
    let w = world.world_transform(h)?;
    let pose = world.sly2.as_deref()?.pose_of(h.index());
    Some(match usize::try_from(t).ok().and_then(|t| pose.get(t)) {
        Some(p) => w.mul(p),
        None => w,
    })
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
        for m in s.meshes.meshes.iter().filter(|m| m.body_type == 0) {
            for g in &m.groups {
                let weights = g.weights(data, m.bones.len());
                let verts = g.vertices(data);
                if verts.is_empty() || verts.len() > 65_535 {
                    continue;
                }
                let skin: Vec<SkinVertex> = (0..verts.len())
                    .map(|v| match &weights {
                        Some(w) => {
                            let mut bones = [0u8; 4];
                            let mut ws = [0.0f32; 4];
                            for (k, &wk) in w.vertex(v).iter().take(4).enumerate() {
                                bones[k] = w.slots.get(k).copied().unwrap_or(0);
                                ws[k] = f32::from(wk) / 255.0;
                            }
                            SkinVertex::new(bones, ws)
                        }
                        None => SkinVertex::NONE,
                    })
                    .collect();
                let vertices: Vec<Vertex> =
                    verts.iter().map(|v| Vertex::new(v.position, v.uv, v.normal, v.color)).collect();
                for (shader, tris) in g.triangles(data) {
                    if tris.is_empty() || shader < 0 {
                        continue;
                    }
                    let indices: Vec<u16> = tris.iter().flatten().map(|&i| u16::from(i)).collect();
                    out.push(SkinnedDraw {
                        object: h,
                        vertices: vertices.clone(),
                        indices,
                        skin: skin.clone(),
                        material: base + shader as u32,
                        bones: m.bones.clone(),
                        transform: m.transform,
                    });
                }
            }
        }
    }
    out
}

/// One placed skinned instance: the renderer instance, the character, its palette, bone table and mesh transform.
struct Part {
    id: InstanceId,
    object: Handle,
    palette: SkinPalette,
    /// The bone table resolved once at placement: each bone's object and transform reference.
    bones: Vec<Option<(Handle, i16)>>,
    transform: i16,
    /// The character's world matrix and visibility when the bones were last given (nothing to do while unchanged).
    last: Option<(Affine, bool)>,
}

/// The characters' skinned instances.
#[derive(Default)]
pub struct CharacterSkins {
    parts: Vec<Part>,
    matrices: Vec<Affine>,
    rows: Vec<render::BoneMatrix>,
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

    /// Places the instances (after `commit`) with their materials, and builds the palettes from the loaded pose.
    pub fn place<B: Backend>(
        r: &mut Renderer<B>,
        world: &World,
        draws: &[SkinnedDraw],
        meshes: &[render::MeshId],
        material: impl Fn(u32) -> Option<MaterialId>,
    ) -> Result<CharacterSkins, RenderError> {
        let mut out = CharacterSkins::default();
        for (d, &m) in draws.iter().zip(meshes) {
            let Some(mat) = material(d.material) else { continue };
            let id = r.add_instance(m, mat, mat4(&Affine::IDENTITY))?;
            let mesh_bind = transform_of(world, d.object, d.transform).unwrap_or(Affine::IDENTITY);
            let palette = SkinPalette::new(&d.bones, mesh_bind, |o, t| {
                world.find_sly2_id(o, Some(d.object)).and_then(|h| transform_of(world, h, t))
            });
            let bones = d.bones.iter().map(|&(o, t)| world.find_sly2_id(o, Some(d.object)).map(|h| (h, t))).collect();
            out.parts.push(Part { id, object: d.object, palette, bones, transform: d.transform, last: None });
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
        for Part { id, object, palette, bones, transform, last } in &mut self.parts {
            // Without clips on the characters their bones move only with the character's object.
            let now = (world.world_transform(*object).unwrap_or(Affine::IDENTITY), world.in_world(*object));
            if *last == Some(now) {
                continue;
            }
            *last = Some(now);
            let mesh = transform_of(world, *object, *transform).unwrap_or(Affine::IDENTITY);
            r.set_visible(*id, now.1)?;
            if palette.is_empty() {
                // No bones: the mesh follows its transform.
                r.set_transform(*id, mat4(&mesh))?;
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
            self.rows.clear();
            self.rows.extend(self.matrices.iter().map(bone_rows));
            r.set_bones(*id, &self.rows)?;
        }
        Ok(())
    }
}
