//! Sly 2 and Sly 3 scenes turned into [`Geometry`]: the drawing state of every submesh (specs/sly2/geometry.md 9,
//! textures.md 2.4; Sly 3's passes are Sly 2's minus one), the two colours of lit kinds (rendering.md 2 and 3,
//! [`crate::light`]) and static geometry merged without changing a pixel ([`merge_static`]).
//!
//! With the world's lighting, shader kinds 0, 1 and 7 are lit with two colours per vertex
//! (`render::Lighting::TwoColour`: kinds 1 and 7 the grey palette slot 1 times A plus the coloured slot 0 times B;
//! kind 0 slot 0 times A), everything fogged over a black clear. Without it (or for other kinds) draws are unlit:
//! frame 0, palette slot 0, texture times vertex colour.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use formats::scene::Scene;
use render::math::{Mat4, Vec3};
use render::{BlendMode, Fog, Pass, SkinVertex, SortMode, TextureData, Vertex};

use crate::geometry::{DrawItem, DrawLook, Geometry, MaterialDesc, MeshRange, StaticGroup, StaticPiece};
use crate::light::{LightMode, WorldLighting, two_colours};
use crate::texanim::{Motion, TexAnimDesc};
use formats_sly2::scene::BakedColour;

/// The alpha test of shader flag 0x20 on kind 0: alpha at least 0x7E on the 0x80 scale.
pub const SLY2_ALPHA_TEST: f32 = 0x7E as f32 / 128.0;

/// Shader kinds whose flags set the drawing state (textures.md 2.4).
fn flag_kinds(kind: u8) -> bool {
    matches!(kind, 0 | 4 | 5 | 6)
}

/// The blend of a shader's flags (textures.md 2.4), for flags without 0x20.
pub fn flag_blend(flags: u8) -> BlendMode {
    let add = flags & 0x01 != 0;
    if flags & 0x10 != 0 {
        // (Cd - Cs) As + Cs, or Cd As + Cs with 0x01.
        if add { BlendMode::ReverseAdd } else { BlendMode::ReverseAlpha }
    } else if flags & 0x80 != 0 {
        // (Cs - Cd) Ad + Cd.
        BlendMode::DestAlpha
    } else if add {
        BlendMode::Additive
    } else {
        BlendMode::Alpha
    }
}

/// How a Sly 2 submesh is drawn, from its shader's kind and flags and its mesh's pass byte (geometry.md 9's working
/// assumption: 1 sky, 3, 4 and 5 opaque, 10 translucent; 8 shadow shapes are left out, like every special pass).
/// Returns the look and the alpha cutoff.
pub fn classify_sly2(kind: u8, mesh_pass: u8, flags: u8) -> Option<(DrawLook, f32)> {
    let d = |pass, order, blend, sort, two_step| {
        (DrawLook { pass, order, blend, sort, two_step, no_darkening: false }, 0.0)
    };
    let flagged = flag_kinds(kind);
    // Flag 0x20: alpha-tested, no blending. Kind 0 passes at alpha >= 0x7E with a depth test; other kinds never
    // pass, so only the failed-pixel rule draws them: with 0x04 failed pixels write colour but not depth.
    if flagged && flags & 0x20 != 0 && matches!(mesh_pass, 1 | 3..=5 | 10) {
        let keep_failed = flags & 0x04 != 0;
        return match (kind, keep_failed) {
            (0, false) => Some((d(Pass::AlphaTest, 5, BlendMode::None, SortMode::Grouped, false).0, SLY2_ALPHA_TEST)),
            (0, true) => Some((d(Pass::Blend, 5, BlendMode::None, SortMode::Grouped, true).0, SLY2_ALPHA_TEST)),
            (_, true) => Some(d(Pass::Post, 5, BlendMode::None, SortMode::Grouped, false)),
            (_, false) => None,
        };
    }
    let blend = if flagged {
        flag_blend(flags)
    } else if flags & 0x01 != 0 {
        BlendMode::Additive
    } else {
        BlendMode::Alpha
    };
    match mesh_pass {
        1 => Some(d(Pass::Background, 1, blend, SortMode::FarToNear, false)),
        3..=5 if kind != 2 && kind != 3 => {
            // Kinds that blend by their flags do so in any pass; the rest are opaque here.
            if flagged && flags & 0xB1 != 0 {
                Some(d(Pass::Blend, 16, blend, SortMode::FarToNear, false))
            } else {
                Some(d(Pass::Opaque, 5, BlendMode::None, SortMode::Grouped, false))
            }
        }
        10 if kind != 2 && kind != 3 => Some(d(Pass::Blend, 16, blend, SortMode::FarToNear, false)),
        // Kind 2, shadow hulls (3.4): the scene inside each closed hull darkened by the material colour's alpha
        // (a stencil volume painting black). The game counts all hulls of the phase and darkens the marked pixels
        // once by fixed ramps; here each hull darkens what it holds.
        7 | 8 if kind == 2 => Some(d(Pass::Volume, 8, BlendMode::Alpha, SortMode::Grouped, false)),
        // Kind 3, projected volumes (3.5): untextured, the scene inside filled with the constant colour; textured
        // (flag 0x02, light shafts), the surface drawn blended with it.
        11 | 13 if kind == 3 => {
            let order = if mesh_pass == 11 { 17 } else { 18 };
            Some(if flags & 0x02 != 0 {
                d(Pass::Blend, order, blend, SortMode::FarToNear, false)
            } else {
                d(Pass::Volume, order, blend, SortMode::Grouped, false)
            })
        }
        _ => None,
    }
}

/// The tint of a kind 2 or 3 submesh (rendering.md 3.4, 3.5, 8): kind 2 black at the material alpha (0x80 = 1.0),
/// kind 3 its second colour (`(c + 1) / 2` on the 0x80 scale, so about `c / 255`).
pub fn volume_tint(kind: u8, color: [u8; 4], color2: [u8; 4]) -> [u8; 4] {
    match kind {
        2 => [0, 0, 0, (color[3] as u16 * 2).min(255) as u8],
        _ => color2,
    }
}

/// A vertex colour on the renderer's scale: Sly 2 stores 0x80 = 1.0 (geometry.md 4.1), so double it, up to 1.0.
pub fn vertex_colour(c: [u8; 4]) -> [u8; 4] {
    c.map(|x| (x as u16 * 2).min(255) as u8)
}

/// Side of the cubic cells static geometry is merged in (game units): the culling granularity of merged meshes.
pub const STATIC_CELL: f32 = 4096.0;
/// Most vertices in one merged mesh (16-bit indices).
pub const MAX_MERGED_VERTICES: usize = 65535;

/// What a static group shares: the drawing state, the spatial cell and the winding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct GroupKey {
    order: u8,
    pass: u8,
    blend: u8,
    two_step: bool,
    no_darkening: bool,
    cutoff: u32,
    mirrored: bool,
    lit: bool,
    /// Zone mask: groups never mix masks, so the camera's visibility mask applies to whole groups.
    zone: u32,
    cell: (i32, i32, i32),
}

/// Which game a scene is from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    Sly2,
    Sly3,
}

/// The alpha cut-off of Sly 3's opaque passes (its textures.md 4 asks for one; rendering.md 0 says alpha only
/// decides depth writes, which the renderer has no state for yet).
pub const SLY3_ALPHA_CUTOFF: f32 = 0.5;

/// How a Sly 3 submesh is drawn: Sly 2's rule with the pass moved up by one (Sly 3's passes are Sly 2's minus one),
/// opaque draws alpha-tested at [`SLY3_ALPHA_CUTOFF`].
pub fn classify_sly3(kind: u8, mesh_pass: u8, flags: u8) -> Option<(DrawLook, f32)> {
    // Kind 5 in pass 15 (new in Sly 3): colour only, no depth, like kinds 4 and 6 (rendering.md 1).
    if mesh_pass == 15 && kind == 5 {
        let look = DrawLook {
            pass: Pass::Post,
            order: 19,
            blend: flag_blend(flags),
            sort: SortMode::Grouped,
            two_step: false,
            no_darkening: false,
        };
        return Some((look, 0.0));
    }
    // Sly 3's pass 14 is Sly 2's 13; the others are Sly 2's minus one.
    let pass2 = if mesh_pass == 14 { 13 } else { mesh_pass.checked_add(1)? };
    let (look, cutoff) = classify_sly2(kind, pass2, flags)?;
    if look.pass == Pass::Opaque {
        let look = DrawLook { pass: Pass::AlphaTest, ..look };
        return Some((look, SLY3_ALPHA_CUTOFF));
    }
    Some((look, cutoff))
}

/// Pre-baked colours: (scene instance, submesh) to the first record, and the records.
pub type Baked<'a> = (&'a BTreeMap<(u32, u32), u32>, &'a [BakedColour]);

/// A scene to prepare and how.
#[derive(Clone, Copy, Debug)]
pub struct SceneInput<'a> {
    pub game: Game,
    pub scene: &'a Scene,
    /// Per material: the grey palette's texture (`SceneBuild::slot1`).
    pub slot1: &'a [Option<u32>],
    /// Per material: the shader's self-illumination (`SceneBuild::self_illum`).
    pub self_illum: &'a [u16],
    /// The world's lighting; `None`: everything unlit, the old dark-blue clear colour, no fog.
    pub lighting: Option<&'a WorldLighting>,
    /// Merge static geometry ([`merge_static`]).
    pub merge: bool,
    /// Scene instances whose record (header offset) falls in one of these ranges are left out (drawn by another
    /// path, the game's characters).
    pub skip: &'a [(u32, u32)],
    /// Scene instances whose record falls in one of these ranges stay unmerged instances, always shown by zone, and
    /// are listed in [`Prepared::dynamic`]: the game moves and shows them every frame (objects attached to the
    /// characters).
    pub dynamic: &'a [(u32, u32)],
    /// Sly 3's pre-baked colours (`SceneBuild::baked`, `baked_colours`): static draws that have them take their two
    /// colours from them instead of the bake (Sly 3 rendering.md 2.3).
    pub baked: Option<Baked<'a>>,
    /// Per scene mesh: the body's flags byte (bit 0: lit per frame).
    pub mesh_flags: &'a [u8],
    /// Per scene submesh: its strip list's flags (0x20: colours from the pre-baked table).
    pub submesh_flags: &'a [i32],
    /// Texture animations of the scene's materials (Sly 2 `SceneBuild::anims`).
    pub anims: &'a [formats_sly2::scene::SceneAnim],
    /// Per material: the shader's second colour (kind 3's constant colour).
    pub color2: &'a [[u8; 4]],
    /// Per scene instance: its zone mask (Sly 2 `SceneBuild::instance_zones`; empty: every zone).
    pub zones: &'a [u32],
}

impl<'a> SceneInput<'a> {
    /// A Sly 2 build with its lighting.
    pub fn sly2(build: &'a formats_sly2::scene::SceneBuild, lighting: Option<&'a WorldLighting>) -> SceneInput<'a> {
        SceneInput {
            game: Game::Sly2,
            scene: &build.scene,
            slot1: &build.slot1,
            self_illum: &build.self_illum,
            lighting,
            merge: true,
            skip: &[],
            dynamic: &[],
            baked: Some((&build.baked, &build.baked_colours)),
            mesh_flags: &build.mesh_flags,
            submesh_flags: &build.submesh_flags,
            anims: &build.anims,
            color2: &build.color2,
            zones: &build.instance_zones,
        }
    }

    /// A Sly 3 build with its lighting.
    pub fn sly3(build: &'a formats_sly3::scene::SceneBuild, lighting: Option<&'a WorldLighting>) -> SceneInput<'a> {
        SceneInput {
            game: Game::Sly3,
            scene: &build.scene,
            slot1: &build.slot1,
            self_illum: &build.self_illum,
            lighting,
            merge: true,
            skip: &[],
            dynamic: &[],
            baked: Some((&build.baked, &build.baked_colours)),
            mesh_flags: &build.mesh_flags,
            submesh_flags: &build.submesh_flags,
            anims: &[],
            color2: &build.color2,
            zones: &[],
        }
    }
}

/// Counters of a prepared scene.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrepStats {
    pub scene_instances: u32,
    /// Draws left as instances.
    pub draws: u32,
    /// Submesh draws not drawn (passes and kinds left out).
    pub skipped_draws: u32,
    pub triangles: u64,
    pub meshes: u32,
    pub vertices: u32,
    pub textures: u32,
    pub opaque: u32,
    pub blended: u32,
    pub backdrops: u32,
    pub post: u32,
    pub weights: u32,
    pub static_pieces: u32,
    pub static_groups: u32,
}

/// A prepared scene.
#[derive(Clone, Debug, Default)]
pub struct Prepared {
    pub geometry: Geometry,
    /// Clear colour (RGBA, 0..1).
    pub clear: [f32; 4],
    pub fog: Option<Fog>,
    /// World centres of the drawn instances outside the sky (for bounds and the camera).
    pub centres: Vec<Vec3>,
    pub stats: PrepStats,
    /// Texture animations of the prepared materials ([`crate::texanim::Animator`]).
    pub anims: Vec<TexAnimDesc>,
    /// The draws of [`SceneInput::dynamic`] records, by their index in the geometry's draws.
    pub dynamic: Vec<DynamicDraw>,
}

/// A draw the game moves and shows every frame ([`SceneInput::dynamic`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicDraw {
    /// Index in [`Geometry::draws`] (and in the placed instances).
    pub draw: u32,
    /// The scene instance's record (header offset).
    pub record: u32,
    /// A kind 2 or 3 stencil volume.
    pub volume: bool,
    /// The scene mesh's bounding radius.
    pub radius: f32,
    /// The scene mesh's index in its mesh set.
    pub mesh: u32,
}

/// Converts a Sly 2 or Sly 3 scene: materials by shader and list, meshes once per submesh, every vertex of lit kinds
/// with its two colours, unsorted draws merged into static groups ([`merge_static`]).
pub fn prepare(input: SceneInput<'_>) -> Result<Prepared, String> {
    let SceneInput {
        game,
        scene,
        slot1,
        self_illum,
        lighting,
        merge,
        skip,
        dynamic,
        baked,
        mesh_flags,
        submesh_flags,
        anims,
        color2,
        zones,
    } = input;
    let anim_of: BTreeMap<u32, &formats_sly2::scene::SceneAnim> = anims.iter().map(|a| (a.material, a)).collect();
    let mut out = Prepared::default();
    let g = &mut out.geometry;
    for t in &scene.textures {
        g.textures.push(
            TextureData::rgba8_with_mips(t.image.width, t.image.height, t.image.pixels.clone())
                .map_err(|e| alloc::format!("texture {}: {e}", t.bitmap))?,
        );
    }
    let white = g.textures.len() as u32;
    g.textures.push(TextureData::rgba8(8, 8, vec![255; 8 * 8 * 4]).map_err(|e| alloc::format!("{e}"))?);
    // Lit: black (rendering.md 6); unlit keeps the first render's dark blue.
    out.clear = lighting.map_or([16.0 / 255.0, 16.0 / 255.0, 40.0 / 255.0, 1.0], |l| l.clear);
    out.fog = lighting.map(|l| l.fog);
    out.stats.scene_instances = scene.instances.len() as u32;

    let mut materials: BTreeMap<(u16, u8), u32> = BTreeMap::new();
    let mut meshes: BTreeMap<u32, u32> = BTreeMap::new();
    // Model-space bounds per prepared mesh.
    let mut mesh_bounds: Vec<(Vec3, Vec3)> = Vec::new();
    // Every draw in scene order; unsorted ones may be merged into static groups afterwards ([`merge_static`]).
    let mut entries: Vec<Entry> = Vec::new();
    for (ii, inst) in scene.instances.iter().enumerate() {
        // Dynamic records win over skipped ones: they may be nested in a skipped record (parts of a character).
        let moved = dynamic.iter().any(|&(a, b)| inst.record >= a && inst.record < b);
        if !moved && skip.iter().any(|&(a, b)| inst.record >= a && inst.record < b) {
            continue;
        }
        // Sly 2's pass numbers (Sly 3's plus one).
        let pass = match game {
            Game::Sly2 => inst.pass,
            Game::Sly3 => inst.pass.saturating_add(1),
        };
        let zone = if moved { u32::MAX } else { zones.get(ii).copied().unwrap_or(u32::MAX) };
        let Some(m) = scene.meshes.get(inst.mesh as usize) else {
            out.stats.skipped_draws += 1;
            continue;
        };
        let transform = mat4(&inst.transform);
        let centre = transform.transform_point(Vec3::new(m.center[0], m.center[1], m.center[2]));
        let mut placed = false;
        for si in m.submeshes.clone() {
            let Some(sm) = scene.submeshes.get(si as usize) else { continue };
            // Ranges outside the scene's arrays (a damaged build) are left out, not indexed.
            let in_range =
                |first: u32, n: u32, len: usize| (first as usize).checked_add(n as usize).is_some_and(|e| e <= len);
            if !in_range(sm.base_vertex, sm.vertex_count, scene.vertices.len())
                || !in_range(sm.first_index, sm.index_count, scene.indices.len())
            {
                out.stats.skipped_draws += 1;
                continue;
            }
            let Some(mat) = scene.materials.get(sm.material as usize) else { continue };
            let classified = match game {
                Game::Sly2 => classify_sly2(mat.kind, inst.pass, mat.blend),
                Game::Sly3 => classify_sly3(mat.kind, inst.pass, mat.blend),
            };
            let Some((look, cutoff)) = classified else {
                out.stats.skipped_draws += 1;
                continue;
            };
            if sm.index_count == 0 {
                continue;
            }
            // Kinds 2 and 3 (hulls and volumes) take a constant colour, not the two colours.
            let volume = matches!(mat.kind, 2 | 3);
            let lit = lighting.is_some() && !volume;
            let material = *materials.entry((sm.material, look.order)).or_insert_with(|| {
                let texture = mat.texture.unwrap_or(white);
                // Kinds 1 and 7: the grey palette (slot 1) under colour A; the others draw slot 0 under it.
                let grey = slot1.get(sm.material as usize).copied().flatten().unwrap_or(texture);
                g.materials.push(MaterialDesc {
                    texture,
                    palettes: lit.then_some([grey, grey]),
                    look,
                    // Lit: the material colour is part of the two colours.
                    tint: if volume {
                        volume_tint(mat.kind, mat.color, color2.get(sm.material as usize).copied().unwrap_or([0; 4]))
                    } else if lit {
                        [255; 4]
                    } else {
                        mat.color
                    },
                    cutoff: (cutoff > 0.0).then_some(cutoff),
                    two_colour: lit,
                });
                let index = g.materials.len() as u32 - 1;
                // Its texture animation: frames as (texture, grey images), the material's own where one is missing.
                if let Some(a) = anim_of.get(&(sm.material as u32))
                    && let Some(motion) = Motion::from_sly2(a.class, a.params, a.extra)
                {
                    let frames = a
                        .frames
                        .iter()
                        .map(|&(t, gr)| {
                            let t = t.unwrap_or(texture);
                            (
                                t,
                                lit.then(|| {
                                    [gr.unwrap_or(if slot1.get(sm.material as usize).copied().flatten().is_some() {
                                        grey
                                    } else {
                                        t
                                    }); 2]
                                }),
                            )
                        })
                        .collect();
                    out.anims.push(TexAnimDesc { material: index, motion, frames });
                }
                g.materials.len() as u32 - 1
            });
            out.stats.triangles += sm.index_count as u64 / 3;
            placed = true;
            let mesh = *meshes.entry(si).or_insert_with(|| {
                let verts = &scene.vertices[sm.base_vertex as usize..][..sm.vertex_count as usize];
                let first_vertex = g.vertices.len() as u32;
                let first_index = g.indices.len() as u32;
                // Sly 2 stores vertex colours on the 0x80 scale, Sly 3 on the 255 scale.
                let colour = |c: [u8; 4]| if game == Game::Sly2 { vertex_colour(c) } else { c };
                g.vertices.extend(verts.iter().map(|v| Vertex::new(v.position, v.uv, v.normal, colour(v.color))));
                g.skin.extend(core::iter::repeat_n(SkinVertex::NONE, verts.len()));
                g.indices.extend_from_slice(&scene.indices[sm.first_index as usize..][..sm.index_count as usize]);
                g.meshes.push(MeshRange {
                    first_vertex,
                    vertex_count: sm.vertex_count,
                    first_index,
                    index_count: sm.index_count,
                    skinned: false,
                });
                mesh_bounds.push(local_bounds(verts.iter().map(|v| v.position)));
                g.meshes.len() as u32 - 1
            });
            let grouped = look.sort == SortMode::Grouped;
            // Mirrored as the renderer decides it for an instance (the sign of the transform's determinant), so a
            // nearly flat placement keeps its winding when merged.
            let mirrored = transform.determinant3() < 0.0;
            // Animated materials stay instances: a placed mesh fixes its pieces' images and texture coordinates.
            // Stencil volumes too: each counts and paints on its own.
            let animated = anim_of.contains_key(&(sm.material as u32)) || look.pass == Pass::Volume;
            let key = (merge && grouped && !animated && !moved && sm.vertex_count as usize <= MAX_MERGED_VERTICES)
                .then(|| {
                    let cell = |v: f32| render::math::floor(v / STATIC_CELL) as i32;
                    GroupKey {
                        order: look.order,
                        pass: look.pass as u8,
                        blend: look.blend as u8,
                        two_step: look.two_step,
                        no_darkening: look.no_darkening,
                        cutoff: cutoff.to_bits(),
                        mirrored,
                        lit,
                        zone,
                        cell: (cell(centre.x), cell(centre.y), cell(centre.z)),
                    }
                });
            let (lo, hi) = mesh_bounds[mesh as usize];
            // The two colours (rendering.md 2.3): bit 0 of the mesh body's flags lights it per frame; otherwise
            // kinds 1 and 7 take the pre-baked table (strip-list flag 0x20) or the static bake, and the other kinds
            // the unlit colour (2.5). Kinds other than 1 and 7 draw colour A alone.
            let weights = lighting.filter(|_| lit).map(|l| {
                let verts = &scene.vertices[sm.base_vertex as usize..][..sm.vertex_count as usize];
                let start = g.weights.len() as u32;
                let one_pass = !matches!(mat.kind, 1 | 7);
                let moving = mesh_flags.get(inst.mesh as usize).is_some_and(|f| f & 1 != 0);
                let from_table = submesh_flags.get(si as usize).is_some_and(|f| f & 0x20 != 0);
                let table = baked.and_then(|(map, colours)| {
                    Some(&colours[*map.get(&(ii as u32, si))? as usize..][..sm.vertex_count as usize])
                });
                let illum = self_illum.get(sm.material as usize).copied().unwrap_or(0);
                let mut light = |mode| {
                    light_vertices(
                        l,
                        mode,
                        &transform,
                        centre,
                        m.radius,
                        verts,
                        mat.color,
                        illum,
                        one_pass,
                        &mut g.weights,
                    )
                };
                match (moving, one_pass, from_table.then_some(table).flatten()) {
                    (true, _, _) => light(LightMode::Moving),
                    (false, true, _) => g.weights.extend(verts.iter().map(|v| unlit_weights(v.color, mat.color))),
                    (false, false, Some(t)) => {
                        g.weights.extend(verts.iter().zip(t).map(|(v, c)| baked_weights(*c, v.color, mat.color, false)))
                    }
                    (false, false, None) => light(LightMode::Baked),
                }
                start
            });
            entries.push(Entry {
                draw: DrawItem { mesh, material, transform, weights, sort_key: None, fog: 1.0 },
                order: look.order,
                pass: look.pass,
                mirrored,
                grouped,
                key,
                zone,
                bounds: world_bounds(&transform, lo, hi),
                dynamic: moved.then_some((inst.record, volume, m.radius, m.mesh)),
            });
        }
        if placed && pass != 1 {
            out.centres.push(centre);
        }
    }
    let groups = merge_static(&entries);
    let kept: Vec<&Entry> = entries.iter().filter(|e| e.key.is_none()).collect();
    out.dynamic = kept
        .iter()
        .enumerate()
        .filter_map(|(k, e)| {
            let (record, volume, radius, mesh) = e.dynamic?;
            Some(DynamicDraw { draw: k as u32, record, volume, radius, mesh })
        })
        .collect();
    g.draw_zones = kept.iter().map(|e| e.zone).collect();
    g.draws = kept.iter().map(|e| e.draw).collect();
    out.stats.weights = g.weights.len() as u32;
    out.stats.static_pieces = groups.iter().map(|s| s.pieces.len() as u32).sum();
    out.stats.static_groups = groups.len() as u32;
    g.static_groups = groups;
    for d in &g.draws {
        match g.materials[d.material as usize].look.pass {
            Pass::Opaque | Pass::AlphaTest => out.stats.opaque += 1,
            Pass::Background => out.stats.backdrops += 1,
            Pass::Post => out.stats.post += 1,
            _ => out.stats.blended += 1,
        }
    }
    out.stats.draws = g.draws.len() as u32;
    out.stats.meshes = g.meshes.len() as u32;
    out.stats.vertices = g.vertices.len() as u32;
    out.stats.textures = g.textures.len() as u32;
    Ok(out)
}

/// The unlit colour A of a static vertex whose shader kind is not 1 or 7 (rendering.md 2.5): `(v m + 255) / 256`
/// quantised to 5 bits, alpha `(v m + 255) / 510` on the 0x80 scale; no B.
pub fn unlit_weights(v: [u8; 4], m: [u8; 4]) -> render::VertexWeights {
    let c = |k: usize| ((v[k] as u32 * m[k] as u32).div_ceil(256).min(255) as u8) & 0xF8;
    render::VertexWeights {
        shadow: ((v[3] as u32 * m[3] as u32 + 255) / 510).min(255) as u8,
        mid: [c(0), c(1), c(2)],
        high: [0; 3],
        alpha: 0,
    }
}

/// The two colours of a pre-baked record (Sly 3 geometry.md 4.2, rendering.md 2.3): A and B as stored (15-bit, on
/// the 0x80 scale), the amount as B's alpha, A's alpha the vertex alpha times the material alpha on the 0x80 scale.
pub fn baked_weights(c: BakedColour, v: [u8; 4], m: [u8; 4], one_pass: bool) -> render::VertexWeights {
    let a = formats_sly2::scene::color15(c.a);
    let b = formats_sly2::scene::color15(c.b);
    render::VertexWeights {
        shadow: ((v[3] as u32 * m[3] as u32 + 255) / 510).min(255) as u8,
        mid: [a[0], a[1], a[2]],
        high: if one_pass { [0; 3] } else { [b[0], b[1], b[2]] },
        alpha: c.amount,
    }
}

/// The column-major 4 x 4 of an affine transform.
pub fn mat4(a: &formats::geometry::Affine) -> Mat4 {
    Mat4 { m: a.to_mat4() }
}

/// Appends the two colours of `verts` placed by `transform` (rendering.md 2.3 and 2.4): baked by every bakeable light
/// near the mesh, or as a moving object by the four strongest lights at its centre (normals quantised to 1/127, as
/// the game sends them). One-pass kinds draw colour A alone: B is left out.
#[allow(clippy::too_many_arguments)]
pub fn light_vertices(
    l: &WorldLighting,
    mode: LightMode,
    transform: &render::math::Mat4,
    centre: Vec3,
    radius: f32,
    verts: &[formats::geometry::GpuVertex],
    color: [u8; 4],
    illum: u16,
    one_pass: bool,
    out: &mut Vec<render::VertexWeights>,
) {
    let lights: Vec<_> = match mode {
        LightMode::Baked => l.baked(centre, radius * transform.max_scale()).into_iter().map(|r| (r, None)).collect(),
        LightMode::Moving => l.strongest(centre).into_iter().map(|(r, d)| (r, Some(d))).collect(),
    };
    let origin = transform.transform_point(Vec3::ZERO);
    let q = |x: f32| match mode {
        LightMode::Baked => x,
        LightMode::Moving => (x * 127.0) as i32 as f32 / 127.0,
    };
    for v in verts {
        let p = transform.transform_point(Vec3::new(v.position[0], v.position[1], v.position[2]));
        let n = transform.transform_point(Vec3::new(q(v.normal[0]), q(v.normal[1]), q(v.normal[2]))) - origin;
        let n = if n.dot(n) > 1e-12 { n.normalize() } else { n };
        let mut w = two_colours(mode, v.color, color, illum, l.shadow, &lights, p, n);
        if one_pass {
            w.high = [0; 3];
        }
        out.push(w);
    }
}

/// World-space bounds (min, max).
type Bounds = (Vec3, Vec3);

/// One draw of the scene while a level is prepared.
struct Entry {
    draw: DrawItem,
    /// Material order and pass: with the winding, the renderer list it is drawn in.
    order: u8,
    pass: Pass,
    mirrored: bool,
    /// Unsorted: its list is not drawn far to near.
    grouped: bool,
    /// Its static group's key while it may be merged; `None`: drawn as an instance.
    key: Option<GroupKey>,
    /// Its instance's zone mask.
    zone: u32,
    /// World-space bounds (min, max).
    bounds: (Vec3, Vec3),
    /// A [`SceneInput::dynamic`] draw: its record, whether it is a volume, the mesh radius and index in its set.
    dynamic: Option<(u32, bool, f32, u32)>,
}

/// How far apart (game units) two pieces' bounds must be for their draw order not to matter: depth values of
/// surfaces closer than this could tie in the depth buffer far from the camera.
pub const ORDER_MARGIN: f32 = 8.0;

fn local_bounds(points: impl Iterator<Item = [f32; 3]>) -> (Vec3, Vec3) {
    let mut lo = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
    let mut hi = Vec3::new(f32::MIN, f32::MIN, f32::MIN);
    for p in points {
        lo = Vec3::new(lo.x.min(p[0]), lo.y.min(p[1]), lo.z.min(p[2]));
        hi = Vec3::new(hi.x.max(p[0]), hi.y.max(p[1]), hi.z.max(p[2]));
    }
    (lo, hi)
}

fn world_bounds(m: &render::math::Mat4, lo: Vec3, hi: Vec3) -> (Vec3, Vec3) {
    local_bounds((0..8).map(|k| {
        let p = Vec3::new(
            if k & 1 == 0 { lo.x } else { hi.x },
            if k & 2 == 0 { lo.y } else { hi.y },
            if k & 4 == 0 { lo.z } else { hi.z },
        );
        let q = m.transform_point(p);
        [q.x, q.y, q.z]
    }))
}

fn union(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> (Vec3, Vec3) {
    (
        Vec3::new(a.0.x.min(b.0.x), a.0.y.min(b.0.y), a.0.z.min(b.0.z)),
        Vec3::new(a.1.x.max(b.1.x), a.1.y.max(b.1.y), a.1.z.max(b.1.z)),
    )
}

/// Whether two draws of one list must keep their order: blended draws always; depth-tested opaque and cut-out ones
/// when their bounds come within [`ORDER_MARGIN`] (farther apart, the depth test picks the same pixels either way).
fn conflict(pass: Pass, a: &(Vec3, Vec3), b: &(Vec3, Vec3)) -> bool {
    let m = ORDER_MARGIN;
    !matches!(pass, Pass::Opaque | Pass::AlphaTest)
        || (a.0.x <= b.1.x + m
            && b.0.x <= a.1.x + m
            && a.0.y <= b.1.y + m
            && b.0.y <= a.1.y + m
            && a.0.z <= b.1.z + m
            && b.0.z <= a.1.z + m)
}

/// Merges the entries that carry a group key into static groups without changing any pixel.
///
/// The renderer draws an unsorted list (material order, pass, winding) by mesh, then by instance. A group is
/// uploaded right after the level mesh of its first piece ([`StaticGroup::after_mesh`]), so it is drawn after that
/// mesh's instances and before the next mesh's; groups after one mesh go in the order they are made. Walking the list
/// in the renderer's order, a piece joins its key's newest group only if that moves it in front of nothing it
/// [`conflict`]s with (instances left after the group's place, pieces of groups made after it); otherwise it starts
/// a new group, which moves it only past later instances of its own mesh, all of them pieces that follow it into
/// groups made later. Entries without a key are drawn as instances where they are.
fn merge_static(entries: &[Entry]) -> Vec<StaticGroup> {
    let mut lists: BTreeMap<(u8, u8, bool), Vec<usize>> = BTreeMap::new();
    for (i, e) in entries.iter().enumerate() {
        if e.grouped {
            lists.entry((e.order, e.pass as u8, e.mirrored)).or_default().push(i);
        }
    }
    let mut groups: Vec<StaticGroup> = Vec::new();
    for (_, mut order) in lists {
        // The renderer's order: mesh, then instance (entry order).
        order.sort_by_key(|&i| (entries[i].draw.mesh, i));
        // Instances kept in this list: (mesh, bounds), in order.
        let mut kept: Vec<(u32, (Vec3, Vec3))> = Vec::new();
        // The list's groups in draw order, with their bounds and members' bounds.
        let mut made: Vec<(StaticGroup, Bounds, Vec<Bounds>)> = Vec::new();
        let mut newest: BTreeMap<GroupKey, usize> = BTreeMap::new();
        for &i in &order {
            let e = &entries[i];
            let Some(key) = e.key else {
                kept.push((e.draw.mesh, e.bounds));
                continue;
            };
            let passes = |b: &(Vec3, Vec3)| conflict(e.pass, &e.bounds, b);
            let join = newest.get(&key).copied().filter(|&g| {
                let after = made[g].0.after_mesh;
                kept.iter().rev().take_while(|k| k.0 > after).all(|k| !passes(&k.1))
                    && made[g + 1..].iter().all(|(_, b, members)| !passes(b) || members.iter().all(|m| !passes(m)))
            });
            let g = join.unwrap_or_else(|| {
                made.push((
                    StaticGroup {
                        material: e.draw.material,
                        mirrored: e.mirrored,
                        after_mesh: e.draw.mesh,
                        zone: e.zone,
                        pieces: Vec::new(),
                    },
                    e.bounds,
                    Vec::new(),
                ));
                newest.insert(key, made.len() - 1);
                made.len() - 1
            });
            let (group, b, members) = &mut made[g];
            group.pieces.push(StaticPiece {
                mesh: e.draw.mesh,
                material: e.draw.material,
                transform: e.draw.transform,
                weights: e.draw.weights,
            });
            *b = union(*b, e.bounds);
            members.push(e.bounds);
        }
        groups.extend(made.into_iter().map(|(g, _, _)| g));
    }
    // Upload order: by the mesh they follow, then as made (a stable sort keeps each list's order).
    groups.sort_by_key(|g| g.after_mesh);
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_pick_the_blend() {
        assert_eq!(flag_blend(0x00), BlendMode::Alpha);
        assert_eq!(flag_blend(0x01), BlendMode::Additive);
        assert_eq!(flag_blend(0x10), BlendMode::ReverseAlpha);
        assert_eq!(flag_blend(0x11), BlendMode::ReverseAdd);
        assert_eq!(flag_blend(0x80), BlendMode::DestAlpha);
        // 0x10 wins over 0x80.
        assert_eq!(flag_blend(0x90), BlendMode::ReverseAlpha);
    }

    #[test]
    fn vertex_colours_double() {
        assert_eq!(vertex_colour([0x80, 0x40, 0xFF, 0x00]), [255, 0x80, 255, 0]);
    }

    #[test]
    fn passes_and_alpha_test() {
        let c = |k, p, f| classify_sly2(k, p, f).map(|(l, cut)| (l.pass, l.blend, l.two_step, cut));
        assert_eq!(c(1, 3, 0), Some((Pass::Opaque, BlendMode::None, false, 0.0)));
        assert_eq!(c(1, 10, 0), Some((Pass::Blend, BlendMode::Alpha, false, 0.0)));
        assert_eq!(c(1, 10, 1), Some((Pass::Blend, BlendMode::Additive, false, 0.0)));
        assert_eq!(c(4, 1, 0).map(|x| x.0), Some(Pass::Background));
        // Kind 0, 0x20: alpha test at 0x7E; with 0x04 the failed pixels still draw colour (two-step, no blending).
        assert_eq!(c(0, 3, 0x20), Some((Pass::AlphaTest, BlendMode::None, false, SLY2_ALPHA_TEST)));
        assert_eq!(c(0, 3, 0x24), Some((Pass::Blend, BlendMode::None, true, SLY2_ALPHA_TEST)));
        // Other flag kinds never pass the test: only 0x04 draws them, colour without depth.
        assert_eq!(c(5, 3, 0x20), None);
        assert_eq!(c(5, 3, 0x24).map(|x| x.0), Some(Pass::Post));
        // Shadow hulls and projected volumes: stencil volumes, or blended surfaces for textured light shafts.
        assert_eq!(c(2, 8, 0).map(|x| x.0), Some(Pass::Volume));
        assert_eq!(c(3, 11, 0).map(|x| x.0), Some(Pass::Volume));
        assert_eq!(c(3, 13, 0x03).map(|x| (x.0, x.1)), Some((Pass::Blend, BlendMode::Additive)));
        assert_eq!(c(1, 7, 0), None);
        assert_eq!(volume_tint(2, [0, 0, 0, 0x30], [0; 4]), [0, 0, 0, 0x60]);
    }

    fn entry(mesh: u32, pass: Pass, cell: i32, at: f32) -> Entry {
        let key = GroupKey {
            order: 5,
            pass: pass as u8,
            blend: 0,
            two_step: false,
            no_darkening: false,
            cutoff: 0,
            mirrored: false,
            lit: false,
            zone: u32::MAX,
            cell: (cell, 0, 0),
        };
        Entry {
            draw: DrawItem {
                mesh,
                material: mesh,
                transform: render::math::Mat4::IDENTITY,
                weights: None,
                sort_key: None,
                fog: 1.0,
            },
            order: 5,
            pass,
            mirrored: false,
            grouped: true,
            key: Some(key),
            zone: u32::MAX,
            bounds: (Vec3::new(at, 0.0, 0.0), Vec3::new(at + 10.0, 10.0, 10.0)),
            dynamic: None,
        }
    }

    fn shape(groups: &[StaticGroup]) -> Vec<(u32, Vec<u32>)> {
        groups.iter().map(|g| (g.after_mesh, g.pieces.iter().map(|p| p.mesh).collect())).collect()
    }

    #[test]
    fn merging_keeps_the_order_of_touching_pieces() {
        // Cells 0, 1, 0, all touching: the second piece of cell 0 cannot move in front of cell 1's.
        let e = [entry(0, Pass::Opaque, 0, 0.0), entry(1, Pass::Opaque, 1, 5.0), entry(2, Pass::Opaque, 0, 8.0)];
        assert_eq!(shape(&merge_static(&e)), vec![(0, vec![0]), (1, vec![1]), (2, vec![2])]);
        // Cell 1's piece far away: cell 0's pieces share one group, drawn before it.
        let e = [entry(0, Pass::Opaque, 0, 0.0), entry(1, Pass::Opaque, 1, 500.0), entry(2, Pass::Opaque, 0, 8.0)];
        assert_eq!(shape(&merge_static(&e)), vec![(0, vec![0, 2]), (1, vec![1])]);
        // Blended pieces keep their order wherever they are.
        let e = [entry(0, Pass::Blend, 0, 0.0), entry(1, Pass::Blend, 1, 500.0), entry(2, Pass::Blend, 0, 8.0)];
        assert_eq!(shape(&merge_static(&e)), vec![(0, vec![0]), (1, vec![1]), (2, vec![2])]);
    }

    #[test]
    fn merging_stays_behind_later_instances() {
        // An instance (mesh 1) between two pieces of one group it touches: the second piece starts a new group.
        let mut e = [entry(0, Pass::Opaque, 0, 0.0), entry(1, Pass::Opaque, 0, 5.0), entry(2, Pass::Opaque, 0, 8.0)];
        e[1].key = None;
        assert_eq!(shape(&merge_static(&e)), vec![(0, vec![0]), (2, vec![2])]);
        // Out of reach: one group.
        e[1].bounds = (Vec3::new(500.0, 0.0, 0.0), Vec3::new(510.0, 10.0, 10.0));
        assert_eq!(shape(&merge_static(&e)), vec![(0, vec![0, 2])]);
    }

    #[test]
    fn unlit_colour_doubles_white() {
        // rendering.md 2.5: the intro hub's sky dome, white vertices and material: A 255 -> 248, alpha 0x80.
        let w = unlit_weights([0xff; 4], [0xff; 4]);
        assert_eq!((w.mid, w.shadow, w.high, w.alpha), ([248; 3], 128, [0; 3], 0));
        // Half-white vertices: (128 * 255 + 255) / 256 = 128.
        assert_eq!(unlit_weights([0x80, 0x80, 0x80, 0xff], [0xff; 4]).mid, [128; 3]);
    }

    #[test]
    fn damaged_scenes_are_skipped_not_indexed() {
        use formats::scene::{SceneInstance, SceneMaterial, SceneMesh, SceneSubmesh};
        let mut scene = Scene::default();
        scene.materials.push(SceneMaterial { kind: 1, blend: 0, pass: 3, color: [255; 4], texture: None });
        // A submesh past the (empty) vertex and index arrays, and an instance of a mesh that does not exist.
        scene.submeshes.push(SceneSubmesh {
            first_index: 0,
            index_count: 3,
            base_vertex: 0,
            vertex_count: 3,
            material: 0,
        });
        scene.meshes.push(SceneMesh { record: 0, mesh: 0, submeshes: 0..1, center: [0.0; 3], radius: 1.0 });
        let inst = SceneInstance {
            mesh: 0,
            transform: formats::geometry::Affine::IDENTITY,
            mirrored: false,
            pass: 3,
            billboard: 0,
            record: 0,
            clone_of_proxy: None,
            issuing_mesh: 0,
        };
        scene.instances.push(inst);
        scene.instances.push(SceneInstance { mesh: 9, ..inst });
        let input = SceneInput {
            game: Game::Sly2,
            scene: &scene,
            slot1: &[],
            self_illum: &[],
            lighting: None,
            merge: true,
            skip: &[],
            dynamic: &[],
            baked: None,
            mesh_flags: &[],
            submesh_flags: &[],
            anims: &[],
            color2: &[],
            zones: &[],
        };
        let p = prepare(input).unwrap();
        assert!(p.geometry.draws.is_empty() && p.geometry.static_groups.is_empty());
        assert_eq!(p.stats.skipped_draws, 2);
    }

    #[test]
    fn dynamic_records_stay_instances_and_win_over_skips() {
        use formats::scene::{SceneInstance, SceneMaterial, SceneMesh, SceneSubmesh};
        // Three opaque instances of one triangle, records 10, 20 and 30: 20 is dynamic inside a skipped 0..25 range,
        // 30 is plain. The dynamic one stays an instance and is listed; 10 is skipped; 30 may be merged.
        let mut scene = Scene::default();
        scene.materials.push(SceneMaterial { kind: 1, blend: 0, pass: 3, color: [255; 4], texture: None });
        let v = |x: f32| formats::geometry::GpuVertex {
            position: [x, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0; 2],
            color: [0x80; 4],
        };
        scene.vertices.extend([v(0.0), v(1.0), v(2.0)]);
        scene.indices.extend([0, 1, 2]);
        scene.submeshes.push(SceneSubmesh {
            first_index: 0,
            index_count: 3,
            base_vertex: 0,
            vertex_count: 3,
            material: 0,
        });
        scene.meshes.push(SceneMesh { record: 0, mesh: 0, submeshes: 0..1, center: [1.0, 0.0, 0.0], radius: 7.0 });
        let inst = SceneInstance {
            mesh: 0,
            transform: formats::geometry::Affine::IDENTITY,
            mirrored: false,
            pass: 3,
            billboard: 0,
            record: 10,
            clone_of_proxy: None,
            issuing_mesh: 0,
        };
        for record in [10, 20, 30] {
            scene.instances.push(SceneInstance { record, ..inst });
        }
        let input = SceneInput {
            game: Game::Sly2,
            scene: &scene,
            slot1: &[],
            self_illum: &[],
            lighting: None,
            merge: true,
            skip: &[(0, 25)],
            dynamic: &[(20, 21)],
            baked: None,
            mesh_flags: &[],
            submesh_flags: &[],
            anims: &[],
            color2: &[],
            zones: &[],
        };
        let p = prepare(input).unwrap();
        assert_eq!(p.dynamic.len(), 1);
        let d = p.dynamic[0];
        assert_eq!((d.record, d.volume, d.radius), (20, false, 7.0));
        assert!((d.draw as usize) < p.geometry.draws.len());
        assert_eq!(p.geometry.draw_zones[d.draw as usize], u32::MAX);
    }

    #[test]
    fn groups_never_mix_zone_masks() {
        // Two far-apart pieces of one look, in zones 1 and 2: two groups, each with its mask.
        let mut e = [entry(0, Pass::Opaque, 0, 0.0), entry(1, Pass::Opaque, 0, 500.0)];
        for (k, x) in e.iter_mut().enumerate() {
            let zone = 1 << k;
            x.zone = zone;
            x.key = x.key.map(|key| GroupKey { zone, ..key });
        }
        let g = merge_static(&e);
        assert_eq!(g.iter().map(|g| g.zone).collect::<Vec<_>>(), [1, 2]);
    }
}
