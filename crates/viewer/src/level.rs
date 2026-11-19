//! Turning a world's [`Scene`] into renderer resources, following specs/sly1/rendering.md.
//!
//! [`prepare`] runs on the loader thread and does all the CPU work: vertex conversion, image decoding and mip
//! chains, the draw lists of every submesh (from the mesh's pass byte), the per-vertex lighting of shader kind 0,
//! blob shadows, and what has to move each frame (backdrops that follow the camera, billboards, texture
//! animation). [`upload`] runs on the render thread and hands the prepared data to the renderer in one commit.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use engine::anim::SkinnedSubmesh;
use formats::geometry::{Affine, GpuVertex, MeshSet};
use formats::options::OptionTable;
use formats::scene::Scene;
use formats::stream::{PlacementOptions, Record};
use formats::world::World;
use render::math::{Mat4, Vec3};
use render::overlay::TextOverlay;
use render::ui2d::UiLayer;
use render::{
    Backend, BlendMode, InstanceId, Lighting, Material, MaterialId, Pass, RenderError, Renderer, SkinVertex, SortMode,
    TextureData, TextureId, Vertex, VertexWeights,
};

use crate::anim::PlayerAnim;
use crate::camera::Spawn;
use crate::look::{Light, Options, WorldLook, emission, kind0_weights, unit};
use crate::texanim::Motion;

pub use stage::geometry::{
    DrawItem, DrawLook, MASK_CUTOFF, MaterialDesc, MeshRange, StaticGroup, StaticPiece, TWO_STEP_CUTOFF,
};

/// Class id of the player character (object-stream.md 6).
pub const PLAYER_CLASS: i16 = 7;
/// Class id of sky objects, which follow the camera (effects.md 6).
pub const SKY_CLASS: i16 = 84;
/// Class id of lights.
pub const LIGHT_CLASS: i16 = 81;
/// OID of the shared blob-shadow mesh (rendering.md 9.1).
pub const SHADOW_MESH_OID: i16 = 29;

/// Shader kinds drawn as images: 0 (three palettes, lit), 1, 6, 7, 8, 9, 10 (one palette) and 4 (stencil volumes).
fn kind_drawn(kind: u8) -> bool {
    matches!(kind, 0 | 1 | 4 | 6 | 7 | 8 | 9 | 10)
}

/// Classifies a submesh by its mesh's pass byte, its shader's kind and blend flags (rendering.md 3.1). `None`:
/// not drawn (passes 0, 2, 5, 9, 14 to 16, shader kinds 2, 3, 5, and kind 4 outside pass 6).
pub fn classify(kind: u8, mesh_pass: u8, blend_flags: u8) -> Option<DrawLook> {
    if !kind_drawn(kind) || (kind == 4) != (mesh_pass == 6) {
        return None;
    }
    let d = |pass, order, blend, sort, two_step| DrawLook { pass, order, blend, sort, two_step, no_darkening: false };
    let over = blend_flags == 2;
    Some(match mesh_pass {
        // Backdrops: flag 2 sorted alpha over (list 2), others unblended in submission order, drawn first (list 3).
        1 if over => d(Pass::Background, 2, BlendMode::Alpha, SortMode::FarToNear, false),
        1 => d(Pass::Background, 1, BlendMode::None, SortMode::Grouped, false),
        3 => d(Pass::Opaque, 5, BlendMode::None, SortMode::Grouped, false),
        // Cut-outs: flag 2 sorted alpha over, other flags additive; both two-step.
        4 if over => d(Pass::Blend, 7, BlendMode::Alpha, SortMode::FarToNear, true),
        4 => d(Pass::Blend, 6, BlendMode::Additive, SortMode::Grouped, true),
        6 => d(
            Pass::Volume,
            9,
            if blend_flags & 1 == 1 { BlendMode::Additive } else { BlendMode::Alpha },
            SortMode::Grouped,
            false,
        ),
        7 => d(Pass::Opaque, 10, BlendMode::None, SortMode::Grouped, false),
        8 if over => d(Pass::Blend, 12, BlendMode::Alpha, SortMode::FarToNear, true),
        8 => d(Pass::Blend, 11, BlendMode::Additive, SortMode::Grouped, true),
        // Masked areas (3.3): step 1 marks the destination alpha with the back faces then draws the front faces,
        // step 2 is alpha-tested at 0.1, step 3 fills by destination alpha then draws alpha over.
        10 => d(Pass::MaskWrite, 13, BlendMode::Alpha, SortMode::Grouped, false),
        11 => d(Pass::AlphaTest, 14, BlendMode::None, SortMode::Grouped, false),
        12 => d(Pass::MaskFill, 15, BlendMode::Alpha, SortMode::Grouped, false),
        13 => {
            let blend = if matches!(blend_flags, 2 | 6) { BlendMode::Alpha } else { BlendMode::Additive };
            d(Pass::Blend, 16, blend, SortMode::FarToNear, true)
        }
        // Map-screen pieces: after the scene, never hidden, not darkened.
        17 => DrawLook { no_darkening: true, ..d(Pass::Post, 20, BlendMode::Alpha, SortMode::Grouped, false) },
        _ => return None,
    })
}

/// A backdrop draw that follows the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyDraw {
    pub draw: u32,
    pub transform: Affine,
    /// World position of the sky object.
    pub origin: Vec3,
    /// Option 608: follow the camera's height too.
    pub follow_z: bool,
}

/// A billboard draw (rendering.md 6.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BillboardDraw {
    pub draw: u32,
    pub transform: Affine,
    /// Bounding-sphere centre in mesh space.
    pub centre: Vec3,
}

pub use stage::texanim::TexAnimDesc;

/// Counters for the report and the overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelStats {
    /// Scene draws (mesh instances) in the world.
    pub scene_instances: u32,
    /// Submesh draws placed in the renderer.
    pub draws: u32,
    /// Submesh draws left out by [`classify`].
    pub skipped_draws: u32,
    /// Triangles of all placed draws.
    pub triangles: u64,
    /// Meshes (submeshes) in the arena.
    pub meshes: u32,
    pub vertices: u32,
    pub textures: u32,
    pub opaque: u32,
    pub blended: u32,
    pub backdrops: u32,
    pub volumes: u32,
    pub post: u32,
    /// Draws lit with three images (kind 0) and the weights they needed.
    pub lit: u32,
    pub weights: u32,
    pub blob_shadows: u32,
    pub billboards: u32,
    pub animations: u32,
    pub lights: u32,
    pub mirrored: u32,
    /// Draws whose transform was degenerate (left out by the scene builder).
    pub degenerate: u32,
    pub texture_errors: u32,
    /// Submesh placements merged into static groups instead of drawn as instances, and the groups.
    pub static_pieces: u32,
    pub static_groups: u32,
}

/// A mesh whose alpha depends on the view angle (rendering.md 6.2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlintDraw {
    pub draw: u32,
    /// The glint direction in world space (unit length).
    pub direction: Vec3,
    pub glint: formats::geometry::Glint,
}

/// Effects a level may carry beyond its draws: view-angle fades, projector receivers and shadow casters, the
/// player's projector (rendering.md 6.2, 9.2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelExtras {
    pub glints: Vec<GlintDraw>,
    /// (draw, receiver bits): mesh flags bits 0x01 (shadows) and 0x02 (light patterns).
    pub receivers: Vec<(u32, u8)>,
    /// Draws drawn into the silhouette image (the player character).
    pub casters: Vec<u32>,
    pub projector: Option<render::Projector>,
    /// Static geometry merged at upload into placed meshes (Sly 2).
    pub static_groups: Vec<StaticGroup>,
}

impl PreparedLevel {
    /// The geometry for [`stage::geometry::upload`] and [`stage::geometry::place`].
    pub fn geometry(&self) -> stage::geometry::GeometryRef<'_> {
        stage::geometry::GeometryRef {
            textures: &self.textures,
            vertices: &self.vertices,
            skin: &self.skin,
            indices: &self.indices,
            meshes: &self.meshes,
            materials: &self.materials,
            draws: &self.draws,
            weights: &self.weights,
            static_groups: &self.extras.static_groups,
        }
    }
}

/// Everything the render thread needs to show a world.
#[derive(Clone, Debug)]
pub struct PreparedLevel {
    pub world: usize,
    pub name: String,
    pub vertices: Vec<Vertex>,
    /// Skinning data, one per vertex (no influence for meshes without bones).
    pub skin: Vec<SkinVertex>,
    pub indices: Vec<u16>,
    pub meshes: Vec<MeshRange>,
    pub textures: Vec<TextureData>,
    pub materials: Vec<MaterialDesc>,
    pub draws: Vec<DrawItem>,
    pub weights: Vec<VertexWeights>,
    pub look: WorldLook,
    pub skies: Vec<SkyDraw>,
    pub billboards: Vec<BillboardDraw>,
    pub animations: Vec<TexAnimDesc>,
    pub spawn: Spawn,
    /// Game-space box around the drawn instances' bounding-sphere centres, without the outermost 2 % on each
    /// side of each axis (a few worlds place an object far outside the playable area).
    pub bounds: (Vec3, Vec3),
    pub stats: LevelStats,
    /// The player character's animation, when the world has the character and its segment.
    pub player: Option<PlayerAnim>,
    /// Draws with a draw distance, which fade out (rendering.md 5 and 6.3).
    pub fades: Vec<FadeDraw>,
    /// Outline meshes: edges as `[far corner, end, end, far corner]` in mesh space.
    pub outline_meshes: Vec<Vec<[[f32; 3]; 4]>>,
    /// Outline draws (rendering.md 5).
    pub outlines: Vec<OutlineDraw>,
    /// The HUD's fonts and tables, when the disc's executable has them.
    pub hud: Option<crate::hud::HudData>,
    pub extras: LevelExtras,
}

/// A draw that fades out with distance: within the last 400 units of `limit` (plus its radius) it moves to the
/// translucent `twin` material with a falling alpha; beyond, it is hidden.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FadeDraw {
    pub draw: u32,
    pub material: u32,
    pub twin: u32,
    pub limit: f32,
    pub centre: Vec3,
    pub radius: f32,
}

/// An outline draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutlineDraw {
    /// Index into [`PreparedLevel::outline_meshes`].
    pub mesh: u32,
    pub transform: Mat4,
    /// List: 8 for pass 5, 13 for pass 9.
    pub order: u8,
    /// Option 95 or (16, 16, 16, 255).
    pub color: [u8; 4],
    /// Outline draw distance (mesh flag 0x020), and the world sphere for the fade.
    pub limit: Option<f32>,
    pub centre: Vec3,
    pub radius: f32,
}

/// Fade alpha for a draw distance (rendering.md 5): 1 inside `outer - 400`, falling with squared distance to 0
/// at `outer` = limit + radius.
pub fn distance_fade(distance: f32, limit: f32, radius: f32) -> f32 {
    let outer = limit + radius;
    let inner = (outer - 400.0).max(0.0);
    if distance <= inner {
        1.0
    } else if distance >= outer {
        0.0
    } else {
        ((outer * outer - distance * distance) / (outer * outer - inner * inner)).clamp(0.0, 1.0)
    }
}

pub use stage::prep::mat4;

/// Decodes images once per (bitmap, palette).
struct Images<'w> {
    world: &'w World<'w>,
    ids: BTreeMap<(u16, Option<u16>), Option<u32>>,
    out: Vec<TextureData>,
    errors: u32,
}

impl Images<'_> {
    fn get(&mut self, bitmap: u16, palette: Option<u16>) -> Option<u32> {
        if let Some(&id) = self.ids.get(&(bitmap, palette)) {
            return id;
        }
        let tables = &self.world.layout.materials;
        let decoded = tables.bitmaps.get(bitmap as usize).and_then(|b| {
            let pal = match palette {
                Some(p) => Some(tables.palettes.get(p as usize)?),
                None => None,
            };
            let img = self.world.payload().decode(b, pal).ok()?;
            TextureData::rgba8_with_mips(img.width, img.height, img.pixels).ok()
        });
        let id = match decoded {
            Some(t) => {
                self.out.push(t);
                Some(self.out.len() as u32 - 1)
            }
            None => {
                self.errors += 1;
                None
            }
        };
        self.ids.insert((bitmap, palette), id);
        id
    }

    /// The main image and, for kind 0, the two other palette images of frame `frame` of shader `shader`.
    fn frame(&mut self, shader: usize, frame: usize) -> Option<(u32, Option<[u32; 2]>)> {
        let sh = self.world.layout.materials.shaders.get(shader)?;
        if sh.kind == 0 {
            let slot = |s| sh.frame_texture(frame, s);
            let (b, p1) = slot(1)?;
            let main = self.get(b, p1)?;
            let pal = match (slot(0), slot(2)) {
                (Some((b0, p0)), Some((b2, p2))) => match (self.get(b0, p0), self.get(b2, p2)) {
                    (Some(t0), Some(t2)) => Some([t0, t2]),
                    _ => None,
                },
                _ => None,
            };
            Some((main, pal))
        } else {
            let (b, p) = sh.frame_texture(frame, 0)?;
            Some((self.get(b, p)?, None))
        }
    }

    fn frame_count(&self, shader: usize) -> usize {
        self.world
            .layout
            .materials
            .shaders
            .get(shader)
            .and_then(|s| s.textures.first())
            .map_or(1, |t| t.bitmaps.len().max(1))
    }
}

/// Whether a record or one of its ancestors has class `class`; returns that ancestor.
fn ancestor_of_class(records: &[Record], index: u32, class: i16) -> Option<u32> {
    let mut at = Some(index);
    while let Some(i) = at {
        let r = records.get(i as usize)?;
        if r.class == class {
            return Some(i);
        }
        at = r.parent;
    }
    None
}

/// The volume colour of a kind-4 shader as RGBA8: the second colour, halved with rounding; alpha 0x80 = 1,
/// RGB 0xFF = 1 (rendering.md 1).
pub fn volume_tint(c: [u8; 4]) -> [u8; 4] {
    let half = |v: u8| (v as u32 + 1) >> 1;
    let a = (half(c[3]) * 255 / 128).min(255) as u8;
    [half(c[0]) as u8, half(c[1]) as u8, half(c[2]) as u8, a]
}

struct Builder<'w> {
    out: PreparedLevel,
    images: Images<'w>,
    white: u32,
    materials: BTreeMap<(usize, u8, bool, Option<usize>), Option<u32>>,
    mesh_ids: BTreeMap<(u32, u32, u32), u32>,
    mesh_spheres: Vec<(Vec3, f32)>,
}

impl Builder<'_> {
    /// The material of shader `sh` drawn as `look`; `anim` is a mesh-level animation record's file offset.
    fn material(&mut self, sh: usize, look: DrawLook, anim: Option<(usize, Motion)>) -> Option<u32> {
        let key = (sh, look.order, look.no_darkening, anim.map(|a| a.0));
        if let Some(&m) = self.materials.get(&key) {
            return m;
        }
        let shader = self.images.world.layout.materials.shaders.get(sh)?.clone();
        let motion = anim.map(|a| a.1).or_else(|| Motion::from_record(&shader.animation));
        let (texture, palettes, tint) = if shader.kind == 4 {
            // Volumes paint their second colour; textured only with blend flag 3.
            let t = if shader.blend == 3 { self.images.frame(sh, 0).map(|f| f.0) } else { None };
            (t.unwrap_or(self.white), None, volume_tint(shader.color2))
        } else {
            match self.images.frame(sh, 0) {
                Some((t, p)) => (t, p, shader.color),
                None => (self.white, None, shader.color),
            }
        };
        // Kind 0 is lit per vertex: the material colour goes into the weights, not the tint.
        let tint = if palettes.is_some() { [255, 255, 255, tint[3]] } else { tint };
        let id = self.out.materials.len() as u32;
        self.out.materials.push(MaterialDesc { texture, palettes, look, tint, cutoff: None, two_colour: false });
        if let Some(motion) = motion {
            let frames = if motion.uses_frames() {
                (0..self.images.frame_count(sh)).filter_map(|f| self.images.frame(sh, f)).collect()
            } else {
                Vec::new()
            };
            self.out.animations.push(TexAnimDesc { material: id, motion, frames });
        }
        self.materials.insert(key, Some(id));
        Some(id)
    }

    /// The arena mesh of submesh `k` of mesh `mesh` of the set owned by `record`.
    fn mesh(&mut self, world: &World<'_>, record: u32, set: &MeshSet, mesh: u32, k: u32) -> Option<u32> {
        if let Some(&id) = self.mesh_ids.get(&(record, mesh, k)) {
            return Some(id);
        }
        let sm = set.meshes.get(mesh as usize)?.submeshes.get(k as usize)?;
        let (mut verts, mut idx) = (Vec::new(), Vec::new());
        sm.append_gpu(world.data, &mut verts, &mut idx);
        if idx.is_empty() {
            return None;
        }
        let id = self.out.meshes.len() as u32;
        let first_vertex = self.out.vertices.len() as u32;
        let first_index = self.out.indices.len() as u32;
        self.out.vertices.extend(verts.iter().map(|v| Vertex::new(v.position, v.uv, v.normal, v.color)));
        self.out.indices.extend_from_slice(&idx);
        let skin = submesh_skin(world.data, Some(set), mesh, k, verts.len());
        let skinned = skin.is_some();
        self.out.skin.extend(skin.unwrap_or_else(|| vec![SkinVertex::NONE; verts.len()]));
        self.out.meshes.push(MeshRange {
            first_vertex,
            vertex_count: verts.len() as u32,
            first_index,
            index_count: idx.len() as u32,
            skinned,
        });
        self.mesh_spheres.push(vertex_sphere(&verts));
        self.mesh_ids.insert((record, mesh, k), id);
        Some(id)
    }
}

/// Converts `scene` (built from `world`) for the renderer.
pub fn prepare(index: usize, name: &str, world: &World<'_>, scene: &Scene) -> Result<PreparedLevel, String> {
    let table = OptionTable::sly1();
    let records = &world.objects.records;
    let look = WorldLook::from_options(&Options::read(world.data, Some(&world.layout.world_options), &table));
    let world_tf = world.objects.world_transforms(true);
    let lights: Vec<Light> = records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.class == LIGHT_CLASS && r.in_world && !r.template)
        .map(|(i, r)| Light::from_options(&Options::read(world.data, r.options.as_ref(), &table), &world_tf[i]))
        .collect();

    let mut images = Images { world, ids: BTreeMap::new(), out: Vec::new(), errors: 0 };
    // A white image for materials without one.
    images.out.push(TextureData::rgba8(8, 8, vec![255; 8 * 8 * 4]).map_err(|e| alloc::format!("{e}"))?);
    let mut b = Builder {
        out: PreparedLevel {
            world: index,
            name: String::from(name),
            vertices: Vec::new(),
            skin: Vec::new(),
            indices: Vec::new(),
            meshes: Vec::new(),
            textures: Vec::new(),
            materials: Vec::new(),
            draws: Vec::new(),
            weights: Vec::new(),
            look,
            skies: Vec::new(),
            billboards: Vec::new(),
            animations: Vec::new(),
            spawn: Spawn { eye: Vec3::new(0.0, -1000.0, 500.0), target: Vec3::ZERO },
            bounds: (Vec3::ZERO, Vec3::ZERO),
            stats: LevelStats {
                scene_instances: scene.instances.len() as u32,
                degenerate: scene.skipped_degenerate,
                lights: lights.len() as u32,
                ..Default::default()
            },
            player: None,
            fades: Vec::new(),
            outline_meshes: Vec::new(),
            outlines: Vec::new(),
            hud: None,
            extras: LevelExtras::default(),
        },
        images,
        white: 0,
        materials: BTreeMap::new(),
        mesh_ids: BTreeMap::new(),
        mesh_spheres: Vec::new(),
    };

    // Map-screen pieces (pass 17) belong to the map objects (class 103), shown in the map view. The Hideout holds
    // all five episode maps on its walls as part of the room, so pass 17 is drawn only where there is more than one.
    let maps_in_room = records.iter().filter(|r| r.class == 103 && r.in_world).count() > 1;
    let mut centres: Vec<Vec3> = Vec::new();
    let mut player: Option<(Vec3, Vec3)> = None;
    let mut draw_of: Vec<(u32, Affine)> = Vec::new();
    let mut spheres: Vec<(Vec3, f32)> = Vec::new();

    for inst in &scene.instances {
        if hidden_below_player(records, inst.record) {
            continue;
        }
        let m = &scene.meshes[inst.mesh as usize];
        let Some(set) = records.get(m.record as usize).and_then(|r| r.mesh_set.as_ref()) else { continue };
        // The mesh that issued the draw (an instance mesh, or the real mesh itself): its pass, sort key, fog
        // factor, flags, billboard and animation apply.
        let Some(issuer) = inst.issuing(world) else { continue };
        let transform = mat4(&inst.transform);
        let c = transform.transform_point(Vec3::new(m.center[0], m.center[1], m.center[2]));
        if player.is_none() && is_player(records, inst.record) {
            // Facing: the object's local X axis, flattened.
            let x = inst.transform.cols[0];
            player = Some((c, Vec3::new(x[0], x[1], 0.0)));
        }
        let sort_key = issuer.sort_key.filter(|g| *g < 1e30).map(|g| g * g.abs());
        let fog = issuer.fog.filter(|f| *f >= 0.001).unwrap_or(1.0);
        let mesh_anim = issuer.animation.and_then(|a| Motion::from_record(&a).map(|mo| (issuer.offset, a.oid, mo)));
        let sky = ancestor_of_class(records, inst.record, SKY_CLASS);
        let normal_cols = inst.transform.normal_matrix();
        let mut placed = false;
        for si in m.submeshes.clone() {
            let k = si - m.submeshes.start;
            let sm_scene = &scene.submeshes[si as usize];
            let shader_index = sm_scene.material as usize;
            let Some(shader) = world.layout.materials.shaders.get(shader_index) else {
                b.out.stats.skipped_draws += 1;
                continue;
            };
            let Some(mut look) =
                classify(shader.kind, inst.pass, shader.blend).filter(|l| l.pass != Pass::Post || maps_in_room)
            else {
                b.out.stats.skipped_draws += 1;
                continue;
            };
            // Mesh flags bit 0x04: not darkened.
            look.no_darkening |= issuer.mesh_flags & 0x04 != 0;
            let anim = mesh_anim.filter(|a| a.1 == shader.oid).map(|a| (a.0, a.2));
            let Some(material) = b.material(shader_index, look, anim) else {
                b.out.stats.skipped_draws += 1;
                continue;
            };
            let Some(mesh) = b.mesh(world, m.record, set, m.mesh, k) else { continue };
            let draw = b.out.draws.len() as u32;

            // Kind 0: weights per vertex from the world's lights.
            let mut weights = None;
            if b.out.materials[material as usize].palettes.is_some() {
                let range = b.out.meshes[mesh as usize];
                let (sc, sr) = b.mesh_spheres[mesh as usize];
                let wc = transform.transform_point(sc);
                let wr = sr * transform.max_scale();
                let near: Vec<&Light> = lights
                    .iter()
                    .filter(|l| {
                        l.kind == crate::look::LightKind::Directional || (l.position - wc).length() < l.range + wr
                    })
                    .collect();
                // Moving objects (the animated player character) are lit once per object (rendering.md 2.3).
                let object = is_player(records, inst.record).then_some(wc);
                let e = set
                    .meshes
                    .get(m.mesh as usize)
                    .and_then(|mm| mm.submeshes.get(k as usize))
                    .map_or(0.0, |s| emission(s.self_illumination));
                let mc = shader.color;
                let start = b.out.weights.len() as u32;
                let verts = &scene.vertices[sm_scene.base_vertex as usize..][..sm_scene.vertex_count as usize];
                for v in verts {
                    let p = transform.transform_point(Vec3::new(v.position[0], v.position[1], v.position[2]));
                    let n = v.normal;
                    let nw = [
                        normal_cols[0][0] * n[0] + normal_cols[1][0] * n[1] + normal_cols[2][0] * n[2],
                        normal_cols[0][1] * n[0] + normal_cols[1][1] * n[1] + normal_cols[2][1] * n[2],
                        normal_cols[0][2] * n[0] + normal_cols[1][2] * n[1] + normal_cols[2][2] * n[2],
                    ];
                    let col = [
                        v.color[0] as f32 * mc[0] as f32 / 65025.0,
                        v.color[1] as f32 * mc[1] as f32 / 65025.0,
                        v.color[2] as f32 * mc[2] as f32 / 65025.0,
                        v.color[3] as f32 * mc[3] as f32 / 65025.0,
                    ];
                    let (ws, wm, wh) = kind0_weights(&b.out.look, &near, p, unit(nw), col, e, object);
                    b.out.weights.push(VertexWeights::new(ws, wm, wh, col[3]));
                }
                if verts.len() as u32 == range.vertex_count {
                    weights = Some(start);
                    b.out.stats.lit += 1;
                } else {
                    b.out.weights.truncate(start as usize);
                }
            }

            b.out.draws.push(DrawItem { mesh, material, transform, weights, sort_key, fog });
            // Projectors: receivers by their mesh flags; the player character casts the shadow.
            // The character does not receive its own shadow.
            if is_player(records, inst.record) {
                b.out.extras.casters.push(draw);
            } else if issuer.mesh_flags & 0x03 != 0 {
                b.out.extras.receivers.push((draw, issuer.mesh_flags & 0x03));
            }
            if let Ok(Some(g)) = issuer.glint(world.data) {
                let d = Vec3::new(g.direction[0], g.direction[1], g.direction[2]);
                let w = transform.transform_point(d) - transform.transform_point(Vec3::ZERO);
                if w.length() > 1e-6 {
                    b.out.extras.glints.push(GlintDraw { draw, direction: w.normalize(), glint: g });
                }
            }
            // Draw distance (mesh flag 0x010): opaque and cut-out draws fade through a translucent twin.
            if let Some(limit) = issuer.max_distance.filter(|d| *d < 1e30)
                && matches!(look.pass, Pass::Opaque | Pass::Blend)
                && let Some(twin_look) = classify(shader.kind, 13, 2)
                && let Some(twin) = b.material(shader_index, twin_look, anim)
            {
                let (sc, sr) = b.mesh_spheres[mesh as usize];
                b.out.fades.push(FadeDraw {
                    draw,
                    material,
                    twin,
                    limit,
                    centre: transform.transform_point(sc),
                    radius: sr * transform.max_scale(),
                });
            }
            draw_of.push((inst.record, inst.transform));
            spheres.push(b.mesh_spheres[mesh as usize]);
            if let Some(s) = sky {
                let r = &records[s as usize];
                let follow_z = Options::read(world.data, r.options.as_ref(), &table).int(608).unwrap_or(0) != 0;
                let o = world_tf[s as usize].t;
                b.out.skies.push(SkyDraw {
                    draw,
                    transform: inst.transform,
                    origin: Vec3::new(o[0], o[1], o[2]),
                    follow_z,
                });
            }
            if issuer.billboard == 1 {
                b.out.billboards.push(BillboardDraw {
                    draw,
                    transform: inst.transform,
                    centre: b.mesh_spheres[mesh as usize].0,
                });
            }
            b.out.stats.triangles += sm_scene.index_count as u64 / 3;
            match look.pass {
                Pass::Opaque => b.out.stats.opaque += 1,
                Pass::Background => b.out.stats.backdrops += 1,
                Pass::Volume => b.out.stats.volumes += 1,
                Pass::Post => b.out.stats.post += 1,
                _ => b.out.stats.blended += 1,
            }
            b.out.stats.mirrored += inst.mirrored as u32;
            placed = true;
        }
        if placed && sky.is_none() {
            centres.push(c);
        }
    }

    blob_shadows(&mut b, world, &world_tf, &table);
    outlines(&mut b, world, &table);
    b.out.extras.projector = player_projector(world, &world_tf, &table);
    if b.out.extras.projector.is_none() {
        b.out.extras.casters.clear();
    }

    let mut out = b.out;
    out.textures = b.images.out;
    out.stats.texture_errors = b.images.errors;
    out.bounds = trimmed_bounds(&centres);
    out.spawn = spawn_point(out.bounds, player);
    out.player = PlayerAnim::build(world, &draw_of, &spheres);
    out.stats.draws = out.draws.len() as u32;
    out.stats.meshes = out.meshes.len() as u32;
    out.stats.vertices = out.vertices.len() as u32;
    out.stats.textures = out.textures.len() as u32;
    out.stats.weights = out.weights.len() as u32;
    out.stats.billboards = out.billboards.len() as u32;
    out.stats.animations = out.animations.len() as u32;
    Ok(out)
}

/// The edges of an outline part: `[far corner, end, end, far corner]` from its positions and edge bytes (bytes 1
/// and 2 are the end points, 0 and 3 the far corners; geometry.md 3.2, rendering.md 5).
fn outline_edges(data: &[u8], part: &formats::geometry::OutlinePart) -> Vec<[[f32; 3]; 4]> {
    let k = part.position_count as usize;
    let at = part.offset + 1;
    let Some(pos_bytes) = data.get(at..at + 12 * k) else { return Vec::new() };
    let f = |i: usize| f32::from_le_bytes([pos_bytes[i], pos_bytes[i + 1], pos_bytes[i + 2], pos_bytes[i + 3]]);
    let pos: Vec<[f32; 3]> = (0..k).map(|i| [f(i * 12), f(i * 12 + 4), f(i * 12 + 8)]).collect();
    let Some(edges) = data.get(part.edges..part.edges + 4 * part.edge_count as usize) else { return Vec::new() };
    edges
        .as_chunks::<4>()
        .0
        .iter()
        .filter_map(|e| {
            Some([
                *pos.get(e[0] as usize)?,
                *pos.get(e[1] as usize)?,
                *pos.get(e[2] as usize)?,
                *pos.get(e[3] as usize)?,
            ])
        })
        .collect()
}

/// Outlines (rendering.md 5): every pass-5 and pass-9 mesh of every placed object whose mesh set has no bones
/// (skinned outlines would need the bones on the GPU too), except the animated player character's.
fn outlines(b: &mut Builder<'_>, world: &World<'_>, table: &OptionTable) {
    let records = &world.objects.records;
    let mut meshes: BTreeMap<(u32, usize, usize), u32> = BTreeMap::new();
    for p in world.objects.placements_where(PlacementOptions::default(), &|r: &Record| r.mesh_set.is_some()) {
        // The player character moves with its rig; its outlines would stay at rest, so they are left out.
        if is_player(records, p.record) {
            continue;
        }
        let Some(set) = records[p.record as usize].mesh_set.as_ref().filter(|s| s.bones.is_empty()) else { continue };
        let mut color: Option<[u8; 4]> = None;
        for d in set.draws() {
            let order = match d.pass {
                5 => 8,
                9 => 13,
                _ => continue,
            };
            let Some(geometry) = set.meshes.get(d.geometry) else { continue };
            let transform = p.world.mul(&d.transform);
            if transform.is_degenerate() {
                continue;
            }
            let color = *color.get_or_insert_with(|| {
                let o = Options::read(world.data, records[p.record as usize].options.as_ref(), table);
                o.color(95).unwrap_or([16, 16, 16, 255])
            });
            let limit = set.meshes.get(d.mesh).and_then(|m| m.outline_distance).filter(|v| *v < 1e30);
            let t = mat4(&transform);
            for (pi, part) in geometry.outlines.iter().enumerate() {
                let key = (p.record, d.geometry, pi);
                let mesh = match meshes.get(&key) {
                    Some(&m) => m,
                    None => {
                        let edges = outline_edges(world.data, part);
                        if edges.is_empty() || edges.len() > render::MAX_OUTLINE_EDGES {
                            continue;
                        }
                        b.out.outline_meshes.push(edges);
                        let m = b.out.outline_meshes.len() as u32 - 1;
                        meshes.insert(key, m);
                        m
                    }
                };
                let edges = &b.out.outline_meshes[mesh as usize];
                let pts: Vec<[f32; 3]> = edges.iter().flat_map(|e| [e[1], e[2]]).collect();
                let (lo, hi) = pts.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
                    (
                        [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                        [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
                    )
                });
                let c = Vec3::new((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5);
                let r = pts.iter().map(|p| (Vec3::new(p[0], p[1], p[2]) - c).length()).fold(0.0f32, f32::max);
                b.out.outlines.push(OutlineDraw {
                    mesh,
                    transform: t,
                    order,
                    color,
                    limit,
                    centre: t.transform_point(c),
                    radius: r * t.max_scale(),
                });
            }
        }
    }
}

/// Blob shadows (rendering.md 9.1): the shared shadow mesh (OID 29) under every object with option 41 > 0,
/// unrotated, scaled by (radius / 100, radius / 100, depth / 100).
fn blob_shadows(b: &mut Builder<'_>, world: &World<'_>, world_tf: &[Affine], table: &OptionTable) {
    let records = &world.objects.records;
    let Some(src) = records.iter().position(|r| r.oid == SHADOW_MESH_OID && r.mesh_set.is_some()) else { return };
    let Some(set) = records[src].mesh_set.as_ref() else { return };
    // Its submeshes: (arena mesh, material).
    let mut parts = Vec::new();
    for (mi, mesh) in set.meshes.iter().enumerate() {
        for (k, sm) in mesh.submeshes.iter().enumerate() {
            let Some(shader) = world.layout.materials.shaders.get(sm.shader as usize) else { continue };
            let Some(look) = classify(shader.kind, mesh.pass, shader.blend) else { continue };
            let (Some(mat), Some(m)) =
                (b.material(sm.shader as usize, look, None), b.mesh(world, src as u32, set, mi as u32, k as u32))
            else {
                continue;
            };
            parts.push((m, mat));
        }
    }
    if parts.is_empty() {
        return;
    }
    for (i, r) in records.iter().enumerate() {
        if !r.in_world || r.template || hidden_below_player(records, i as u32) {
            continue;
        }
        let o = Options::read(world.data, r.options.as_ref(), table);
        let Some(radius) = o.float(41).filter(|r| *r > 0.0) else { continue };
        let depth = o.float(42).unwrap_or(radius);
        let t = world_tf[i].t;
        let a = Affine::new([[radius / 100.0, 0.0, 0.0], [0.0, radius / 100.0, 0.0], [0.0, 0.0, depth / 100.0]], t);
        for &(mesh, material) in &parts {
            b.out.draws.push(DrawItem { mesh, material, transform: mat4(&a), weights: None, sort_key: None, fog: 1.0 });
            b.out.stats.volumes += 1;
        }
        b.out.stats.blob_shadows += 1;
    }
}

/// Classes below the player character whose objects the viewer leaves out: 63 (mine), 76 (decoy) and 97 (dynamic
/// shadow), object-stream.md 6. They are the character's power-ups and effects, kept with it but not shown until
/// used; drawn at rest they sit on top of the character.
const HIDDEN_PLAYER_PARTS: [i16; 3] = [63, 76, 97];

/// Whether record `index` is in a subtree below the player character rooted at one of [`HIDDEN_PLAYER_PARTS`].
fn hidden_below_player(records: &[Record], index: u32) -> bool {
    let mut hidden = false;
    let mut at = Some(index);
    while let Some(i) = at {
        let Some(r) = records.get(i as usize) else { return false };
        if r.class == PLAYER_CLASS {
            return hidden;
        }
        hidden |= HIDDEN_PLAYER_PARTS.contains(&r.class);
        at = r.parent;
    }
    false
}

/// The skinning data of submesh `k` of mesh `mesh` of a mesh set with bones, in the scene's vertex order.
fn submesh_skin(data: &[u8], set: Option<&MeshSet>, mesh: u32, k: u32, count: usize) -> Option<Vec<SkinVertex>> {
    let set = set.filter(|s| !s.bones.is_empty())?;
    let sm = set.meshes.get(mesh as usize)?.submeshes.get(k as usize)?;
    let sk = SkinnedSubmesh::from_submesh(data, sm, set.pose_count() as usize);
    if sk.influences.len() != count || !sk.is_skinned() {
        return None;
    }
    Some(sk.influences.iter().map(|v| SkinVertex::new(v.bones, v.weights)).collect())
}

/// A sphere around the vertices: the centre of their box and the distance to the farthest.
fn vertex_sphere(verts: &[GpuVertex]) -> (Vec3, f32) {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in verts {
        for k in 0..3 {
            lo[k] = lo[k].min(v.position[k]);
            hi[k] = hi[k].max(v.position[k]);
        }
    }
    let c = Vec3::new((lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, (lo[2] + hi[2]) * 0.5);
    let r = verts
        .iter()
        .map(|v| (Vec3::new(v.position[0], v.position[1], v.position[2]) - c).length())
        .fold(0.0f32, f32::max);
    (c, r)
}

/// The box between the 2nd and 98th percentile of `points` on each axis (zero for no points).
pub fn trimmed_bounds(points: &[Vec3]) -> (Vec3, Vec3) {
    if points.is_empty() {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    let axis = |f: fn(&Vec3) -> f32| {
        let mut v: Vec<f32> = points.iter().map(f).collect();
        v.sort_unstable_by(f32::total_cmp);
        let k = v.len() / 50;
        (v[k], v[v.len() - 1 - k])
    };
    let (x, y, z) = (axis(|p| p.x), axis(|p| p.y), axis(|p| p.z));
    (Vec3::new(x.0, y.0, z.0), Vec3::new(x.1, y.1, z.1))
}

/// Whether record `index` is the player character or part of it (a record below one of class [`PLAYER_CLASS`]).
/// The player character's projected shadow (options 43 to 48, rendering.md 9.2, effects.md 7): from its position,
/// with the strength of the shadow shader's material alpha.
fn player_projector(world: &World<'_>, world_tf: &[Affine], table: &OptionTable) -> Option<render::Projector> {
    let records = &world.objects.records;
    let (i, r) = records.iter().enumerate().find(|(_, r)| r.class == PLAYER_CLASS && r.in_world)?;
    let o = Options::read(world.data, r.options.as_ref(), table);
    if o.int(43).unwrap_or(0) == 0 {
        return None;
    }
    let oid = o.int(44)?;
    let shader = world.layout.materials.shaders.iter().find(|s| s.oid as i32 == oid)?;
    let t = world_tf.get(i)?.t;
    Some(render::Projector {
        origin: Vec3::new(t[0], t[1], t[2]),
        near_radius: o.float(45).unwrap_or(150.0),
        far_radius: o.float(46).unwrap_or(10.0),
        near_distance: o.float(47).unwrap_or(100.0),
        far_distance: o.float(48).unwrap_or(400.0),
        strength: shader.color[3] as f32 / 255.0,
        receivers: 0x01,
    })
}

fn is_player(records: &[Record], index: u32) -> bool {
    ancestor_of_class(records, index, PLAYER_CLASS).is_some()
}

/// The camera start: behind and above the player character, looking past it, if the world has one; otherwise
/// above the centre of the level's bounds, looking at it.
pub fn spawn_point(bounds: (Vec3, Vec3), player: Option<(Vec3, Vec3)>) -> Spawn {
    if let Some((p, facing)) = player {
        let f = facing.normalize();
        let f = if f.length() > 0.5 { f } else { Vec3::Y };
        let target = p + Vec3::Z * 100.0;
        return Spawn { eye: p - f * 700.0 + Vec3::Z * 350.0, target };
    }
    let (lo, hi) = bounds;
    let centre = (lo + hi) * 0.5;
    let size = hi - lo;
    let reach = size.x.max(size.y).max(1000.0);
    Spawn { eye: centre + Vec3::new(0.0, -0.35 * reach, 0.2 * reach), target: centre }
}

/// What [`upload`] made.
pub struct Uploaded {
    pub overlay: TextOverlay,
    /// The UI layer (HUD), when a glyph sheet was given.
    pub ui: Option<UiLayer>,
    /// The instance of each outline draw, in [`PreparedLevel::outlines`] order.
    pub outline_instances: Vec<InstanceId>,
    /// The instance of each draw, in [`PreparedLevel::draws`] order.
    pub instances: Vec<InstanceId>,
    pub materials: Vec<MaterialId>,
    pub textures: Vec<TextureId>,
    pub pools: u32,
}

/// Replaces the renderer's level with `level` (or with nothing but the overlay when `level` is `None`), in one
/// commit. The overlay is part of every level, because clearing a level clears everything.
pub fn upload<B: Backend>(
    r: &mut Renderer<B>,
    level: Option<&PreparedLevel>,
    overlay_chars: usize,
) -> Result<Uploaded, RenderError> {
    upload_with_ui(r, level, overlay_chars, None, 0)
}

/// Like [`upload`], with a UI layer of `ui_quads` quads over a glyph sheet.
pub fn upload_with_ui<B: Backend>(
    r: &mut Renderer<B>,
    level: Option<&PreparedLevel>,
    overlay_chars: usize,
    ui_sheet: Option<TextureData>,
    ui_quads: usize,
) -> Result<Uploaded, RenderError> {
    r.clear_level();
    let geo = match level {
        Some(level) => stage::geometry::upload(r, level.geometry())?,
        None => stage::geometry::Uploaded::default(),
    };
    let textures = geo.textures.clone();
    let mut outline_meshes = Vec::new();
    let mut outline_materials = BTreeMap::new();
    if let Some(level) = level {
        for edges in &level.outline_meshes {
            outline_meshes.push(r.add_outline_mesh(edges)?);
        }
        for o in &level.outlines {
            if let alloc::collections::btree_map::Entry::Vacant(v) = outline_materials.entry(o.order) {
                let m = Material {
                    order: o.order,
                    lighting: Lighting::Unlit,
                    ..Material::opaque(textures[0]).with_pass(Pass::Outline)
                };
                v.insert(r.add_material(m)?);
            }
        }
    }
    let ui = match ui_sheet {
        Some(sheet) if ui_quads > 0 => Some(UiLayer::new(r, Some(sheet), ui_quads)?),
        _ => None,
    };
    let overlay = TextOverlay::new(r, overlay_chars)?;
    r.commit()?;
    let mut outline_instances = Vec::new();
    if let Some(level) = level {
        for o in &level.outlines {
            let id = r.add_instance(outline_meshes[o.mesh as usize], outline_materials[&o.order], o.transform)?;
            r.set_tint(id, o.color)?;
            outline_instances.push(id);
        }
    }
    let mut instances = Vec::new();
    if let Some(level) = level {
        instances = stage::geometry::place(r, level.geometry(), &geo)?;
        for &(d, bits) in &level.extras.receivers {
            r.set_receiver(instances[d as usize], bits)?;
        }
        for &d in &level.extras.casters {
            r.set_shadow_caster(instances[d as usize], true)?;
        }
        let projectors: Vec<render::Projector> = level.extras.projector.into_iter().collect();
        r.set_projectors(&projectors)?;
        r.set_fog(level.look.fog);
        r.set_fog_rising(level.look.fog_rising);
    } else {
        r.set_fog(None);
        r.set_projectors(&[])?;
    }
    Ok(Uploaded {
        overlay,
        ui,
        outline_instances,
        instances,
        materials: geo.materials,
        textures,
        pools: r.texture_pool_count() as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_follows_the_mesh_pass() {
        let c = |k, p, b| classify(k, p, b).map(|l| (l.pass, l.order, l.blend, l.sort, l.two_step));
        use BlendMode::*;
        use SortMode::*;
        // Backdrops: flag 2 sorted alpha over (list 2), others unblended and first (list 1).
        assert_eq!(c(6, 1, 2), Some((Pass::Background, 2, Alpha, FarToNear, false)));
        assert_eq!(c(6, 1, 34), Some((Pass::Background, 1, None, Grouped, false)));
        assert_eq!(c(0, 3, 2), Some((Pass::Opaque, 5, None, Grouped, false)));
        assert_eq!(c(1, 7, 2), Some((Pass::Opaque, 10, None, Grouped, false)));
        // Cut-outs: two-step, flag 2 sorted alpha over, others additive.
        assert_eq!(c(0, 4, 2), Some((Pass::Blend, 7, Alpha, FarToNear, true)));
        assert_eq!(c(1, 4, 6), Some((Pass::Blend, 6, Additive, Grouped, true)));
        // Translucent: flags 2 and 6 alpha over, others additive, all sorted and two-step.
        assert_eq!(c(0, 13, 2), Some((Pass::Blend, 16, Alpha, FarToNear, true)));
        assert_eq!(c(1, 13, 6), Some((Pass::Blend, 16, Alpha, FarToNear, true)));
        assert_eq!(c(1, 13, 3), Some((Pass::Blend, 16, Additive, FarToNear, true)));
        // Volumes: flags 0 and 2 alpha over, 1 and 3 additive.
        assert_eq!(c(4, 6, 0).map(|x| (x.0, x.2)), Some((Pass::Volume, Alpha)));
        assert_eq!(c(4, 6, 3).map(|x| (x.0, x.2)), Some((Pass::Volume, Additive)));
        assert!(classify(8, 17, 2).unwrap().no_darkening);
        // Masked areas (3.3): passes 10 to 12 in lists 13 to 15.
        assert_eq!(c(9, 10, 0).map(|x| (x.0, x.1)), Some((Pass::MaskWrite, 13)));
        assert_eq!(c(0, 11, 10).map(|x| (x.0, x.1)), Some((Pass::AlphaTest, 14)));
        assert_eq!(c(10, 12, 0).map(|x| (x.0, x.1)), Some((Pass::MaskFill, 15)));
        // The mesh pass decides, not the shader: kind 1 in pass 13 is translucent whatever the shader says.
        for (k, p) in [(0, 0), (0, 2), (0, 5), (0, 9), (7, 16), (5, 3), (2, 3), (4, 3), (0, 6)] {
            assert_eq!(classify(k, p, 2), Option::None, "kind {k} pass {p}");
        }
    }

    #[test]
    fn fade_by_distance() {
        assert_eq!(distance_fade(100.0, 1000.0, 100.0), 1.0);
        assert_eq!(distance_fade(1200.0, 1000.0, 100.0), 0.0);
        let mid = distance_fade(1000.0, 1000.0, 100.0);
        let expect = (1100.0f32 * 1100.0 - 1000.0 * 1000.0) / (1100.0 * 1100.0 - 700.0 * 700.0);
        assert!((mid - expect).abs() < 1e-6);
    }

    #[test]
    fn volume_colour() {
        // 00 00 00 34: alpha (52 + 1) >> 1 = 26, 26 / 128 = 0.203.
        let t = volume_tint([0, 0, 0, 0x34]);
        assert_eq!(t[..3], [0, 0, 0]);
        assert!((t[3] as f32 / 255.0 - 0.203).abs() < 0.004);
    }

    #[test]
    fn bounds_ignore_outliers() {
        let mut pts: Vec<Vec3> = (0..100).map(|i| Vec3::new(i as f32, 0.0, -(i as f32))).collect();
        pts.push(Vec3::new(0.0, 0.0, 1e6));
        let (lo, hi) = trimmed_bounds(&pts);
        assert_eq!((lo.x, hi.x), (1.0, 97.0));
        assert!(hi.z <= 0.0);
        assert_eq!(trimmed_bounds(&[]), (Vec3::ZERO, Vec3::ZERO));
    }

    #[test]
    fn spawn_rules() {
        let s = spawn_point((Vec3::ZERO, Vec3::ZERO), Some((Vec3::new(100.0, 0.0, 0.0), Vec3::X * 2.0)));
        assert_eq!(s.target, Vec3::new(100.0, 0.0, 100.0));
        assert_eq!(s.eye, Vec3::new(-600.0, 0.0, 350.0));
        let s = spawn_point((Vec3::new(-1000.0, -1000.0, 0.0), Vec3::new(3000.0, 1000.0, 500.0)), None);
        assert_eq!(s.target, Vec3::new(1000.0, 0.0, 250.0));
        assert!(s.eye.z > s.target.z && s.eye.y < 0.0);
    }

    #[test]
    fn upload_places_one_instance_per_draw() {
        use render::NullBackend;
        let v = [
            Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0], [0.0, 0.0, 1.0], [255; 4]),
            Vertex::new([1.0, 0.0, 0.0], [1.0, 0.0], [0.0, 0.0, 1.0], [255; 4]),
            Vertex::new([0.0, 1.0, 0.0], [0.0, 1.0], [0.0, 0.0, 1.0], [255; 4]),
        ];
        let tex = || TextureData::rgba8(8, 8, vec![255; 256]).unwrap();
        let opaque = classify(1, 3, 2).unwrap();
        let glass = classify(0, 13, 3).unwrap();
        let level = PreparedLevel {
            world: 0,
            name: "test".into(),
            vertices: v.to_vec(),
            skin: vec![SkinVertex::NONE; 3],
            indices: vec![0, 1, 2],
            meshes: vec![MeshRange {
                first_vertex: 0,
                vertex_count: 3,
                first_index: 0,
                index_count: 3,
                skinned: false,
            }],
            textures: vec![tex(), tex(), tex()],
            materials: vec![
                MaterialDesc {
                    texture: 0,
                    palettes: None,
                    look: opaque,
                    tint: [255; 4],
                    cutoff: None,
                    two_colour: false,
                },
                MaterialDesc {
                    texture: 0,
                    palettes: Some([1, 2]),
                    look: glass,
                    tint: [255; 4],
                    cutoff: None,
                    two_colour: false,
                },
            ],
            draws: vec![
                DrawItem { mesh: 0, material: 0, transform: Mat4::IDENTITY, weights: None, sort_key: None, fog: 1.0 },
                DrawItem {
                    mesh: 0,
                    material: 1,
                    transform: Mat4::scale(Vec3::new(-1.0, 1.0, 1.0)),
                    weights: Some(0),
                    sort_key: Some(9999.0),
                    fog: 0.5,
                },
            ],
            weights: vec![VertexWeights::default(); 3],
            look: WorldLook { clear: [0.0; 4], fog: None, shadow_level: 0.2, mid_level: 0.0, fog_rising: false },
            skies: Vec::new(),
            billboards: Vec::new(),
            animations: Vec::new(),
            spawn: Spawn { eye: Vec3::Z, target: Vec3::ZERO },
            bounds: (Vec3::ZERO, Vec3::ZERO),
            stats: LevelStats::default(),
            player: None,
            fades: Vec::new(),
            outline_meshes: vec![vec![[[1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0]]]],
            outlines: vec![OutlineDraw {
                mesh: 0,
                transform: Mat4::IDENTITY,
                order: 8,
                color: [16, 16, 16, 255],
                limit: None,
                centre: Vec3::ZERO,
                radius: 1.0,
            }],
            hud: None,
            extras: LevelExtras::default(),
        };
        let mut r = Renderer::new(NullBackend::new(64, 64));
        let up = upload(&mut r, Some(&level), 10).unwrap();
        assert_eq!(up.materials.len(), 2);
        assert_eq!(r.material(up.materials[1]).unwrap().lighting, Lighting::Palettes);
        // Two draws, one outline, plus the overlay's hidden glyph instances.
        assert_eq!(r.instance_count(), 2 + 1 + 10);
        assert_eq!(up.outline_instances.len(), 1);
        let up = upload(&mut r, None, 4).unwrap();
        assert!(up.materials.is_empty());
        assert_eq!(r.instance_count(), 4);
    }
}
