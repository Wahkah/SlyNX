//! Prepared level geometry and its upload: meshes in one arena, materials, draws (one instance each), per-vertex
//! weights and static groups merged into placed meshes. The viewer's Sly 1 levels and both programs' Sly 2 and Sly 3
//! levels are uploaded through [`upload`] and [`place`].

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use render::math::Mat4;
use render::{
    Backend, BlendMode, InstanceId, Lighting, Material, MaterialId, MeshData, MeshId, Pass, PlacedPiece, RenderError,
    Renderer, SkinVertex, SortMode, TextureData, TextureId, Vertex, VertexWeights,
};

/// The alpha test of two-step drawing (Sly 1 rendering.md 3.2).
pub const TWO_STEP_CUTOFF: f32 = 0.9;
/// Alpha cutoff of the masked areas' second step (Sly 1 pass 11, rendering.md 3.1 list 14).
pub const MASK_CUTOFF: f32 = 0.1;

/// How a submesh is drawn: its list and state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawLook {
    pub pass: Pass,
    /// The list number, used as the renderer's order.
    pub order: u8,
    pub blend: BlendMode,
    pub sort: SortMode,
    pub two_step: bool,
    pub no_darkening: bool,
}

/// A converted submesh: ranges of [`Geometry::vertices`] and [`Geometry::indices`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshRange {
    pub first_vertex: u32,
    pub vertex_count: u32,
    pub first_index: u32,
    pub index_count: u32,
    /// The mesh has skinning data ([`Geometry::skin`]).
    pub skinned: bool,
}

/// A material: indices into [`Geometry::textures`] and how to draw it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialDesc {
    pub texture: u32,
    /// T0 and T2 for Sly 1's kind 0 (lit with three images); for two colours, `[grey, grey]`.
    pub palettes: Option<[u32; 2]>,
    pub look: DrawLook,
    pub tint: [u8; 4],
    /// The alpha cutoff, when not the list's own (Sly 2's alpha test).
    pub cutoff: Option<f32>,
    /// Lit with Sly 2's two colours per vertex (`render::Lighting::TwoColour`): `palettes[0]` is the grey image.
    pub two_colour: bool,
}

/// One instance to place: a mesh, a material and a transform (mesh space to game space).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawItem {
    pub mesh: u32,
    pub material: u32,
    pub transform: Mat4,
    /// First of its vertices' weights in [`Geometry::weights`].
    pub weights: Option<u32>,
    /// Far-to-near sort key.
    pub sort_key: Option<f32>,
    /// Fog factor.
    pub fog: f32,
}

/// Static, unsorted draws of one state, spatial cell and winding, merged at upload into placed meshes (one per
/// texture pool, split at 65,535 vertices): fewer draws and instance records, the same pixels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StaticGroup {
    /// A material of the group: its look (pass, list, blending, cutoff) is the group's.
    pub material: u32,
    /// The placements mirror: drawn with front faces clockwise, as their instances were.
    pub mirrored: bool,
    /// Index into [`Geometry::meshes`] of the mesh the group's placed meshes are uploaded right after: where it
    /// falls in the renderer's draw order (by mesh). Groups are listed in upload order.
    pub after_mesh: u32,
    /// In draw order.
    pub pieces: Vec<StaticPiece>,
}

/// One placement of a submesh in a [`StaticGroup`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticPiece {
    /// Index into [`Geometry::meshes`] (model space).
    pub mesh: u32,
    /// Its own material: texture and tint.
    pub material: u32,
    pub transform: Mat4,
    /// First of its vertices' weights in [`Geometry::weights`] (two colours).
    pub weights: Option<u32>,
}

/// A level's geometry, borrowed: what [`upload`] and [`place`] read.
#[derive(Clone, Copy, Debug)]
pub struct GeometryRef<'a> {
    pub textures: &'a [TextureData],
    pub vertices: &'a [Vertex],
    /// One per vertex (no influence for meshes without bones).
    pub skin: &'a [SkinVertex],
    pub indices: &'a [u16],
    pub meshes: &'a [MeshRange],
    pub materials: &'a [MaterialDesc],
    pub draws: &'a [DrawItem],
    pub weights: &'a [VertexWeights],
    pub static_groups: &'a [StaticGroup],
}

/// A level's geometry, owned.
#[derive(Clone, Debug, Default)]
pub struct Geometry {
    pub textures: Vec<TextureData>,
    pub vertices: Vec<Vertex>,
    pub skin: Vec<SkinVertex>,
    pub indices: Vec<u16>,
    pub meshes: Vec<MeshRange>,
    pub materials: Vec<MaterialDesc>,
    pub draws: Vec<DrawItem>,
    pub weights: Vec<VertexWeights>,
    pub static_groups: Vec<StaticGroup>,
}

impl Geometry {
    pub fn as_ref(&self) -> GeometryRef<'_> {
        GeometryRef {
            textures: &self.textures,
            vertices: &self.vertices,
            skin: &self.skin,
            indices: &self.indices,
            meshes: &self.meshes,
            materials: &self.materials,
            draws: &self.draws,
            weights: &self.weights,
            static_groups: &self.static_groups,
        }
    }
}

/// What [`upload`] added (before the commit): handles by prepared index, and the static groups' placed meshes.
#[derive(Clone, Debug, Default)]
pub struct Uploaded {
    /// Per prepared mesh; `None` for meshes drawn only as parts of static groups.
    pub meshes: Vec<Option<MeshId>>,
    pub textures: Vec<TextureId>,
    pub materials: Vec<MaterialId>,
    /// Per draw: its weights' base in the renderer.
    pub weight_bases: Vec<Option<u32>>,
    /// (mesh, material, mirrored, weights base) per placed mesh.
    pub static_draws: Vec<(MeshId, MaterialId, bool, Option<u32>)>,
}

/// The renderer material of a prepared one.
pub fn render_material(m: &MaterialDesc, textures: &[TextureId], lit: bool) -> Material {
    let l = m.look;
    Material {
        texture: textures[m.texture as usize],
        pass: l.pass,
        order: l.order,
        tint: m.tint,
        alpha_cutoff: m.cutoff.unwrap_or(if l.two_step {
            TWO_STEP_CUTOFF
        } else if l.pass == Pass::AlphaTest {
            MASK_CUTOFF
        } else {
            0.0
        }),
        blend: l.blend,
        lighting: match (lit, m.two_colour) {
            (false, _) => Lighting::Unlit,
            (true, false) => Lighting::Palettes,
            (true, true) => Lighting::TwoColour,
        },
        two_step: l.two_step,
        sort: l.sort,
        palettes: m.palettes.filter(|_| lit).map(|[a, b]| [textures[a as usize], textures[b as usize]]),
        no_darkening: l.no_darkening,
        no_fog: false,
    }
}

/// Adds the level's textures, meshes (each static group's placed meshes right after the mesh it follows, which is
/// its place in the draw order), materials and weights. Call it after `clear_level` and before `commit`.
pub fn upload<B: Backend>(r: &mut Renderer<B>, g: GeometryRef<'_>) -> Result<Uploaded, RenderError> {
    let mut out = Uploaded::default();
    // Meshes drawn only as parts of static groups are not uploaded on their own.
    let mut used = alloc::vec![g.static_groups.is_empty(); g.meshes.len()];
    for d in g.draws {
        used[d.mesh as usize] = true;
    }
    // Textures first: the static groups' placed meshes need their pools.
    out.textures.reserve(g.textures.len());
    for t in g.textures {
        out.textures.push(r.add_texture(t.clone())?);
    }
    let mut looks = BTreeMap::new();
    let mut groups = g.static_groups.iter().peekable();
    out.meshes.reserve(g.meshes.len());
    for (k, (m, &used)) in g.meshes.iter().zip(&used).enumerate() {
        if !used {
            out.meshes.push(None);
        } else {
            let data = MeshData {
                vertices: &g.vertices[m.first_vertex as usize..][..m.vertex_count as usize],
                indices: &g.indices[m.first_index as usize..][..m.index_count as usize],
            };
            out.meshes.push(Some(if m.skinned {
                r.add_skinned_mesh(&data, &g.skin[m.first_vertex as usize..][..m.vertex_count as usize])?
            } else {
                r.add_mesh(&data)?
            }));
        }
        let here = core::iter::from_fn(|| groups.next_if(|s| s.after_mesh as usize <= k));
        upload_static(r, g, here, &out.textures, &mut looks, &mut out.static_draws)?;
    }
    upload_static(r, g, groups, &out.textures, &mut looks, &mut out.static_draws)?;
    for m in g.materials {
        // Kind 0 needs its three images in one pool; if they ended up split, draw the mid-tone image alone.
        let id = match r.add_material(render_material(m, &out.textures, m.palettes.is_some())) {
            Ok(id) => id,
            // Two colours: the colour image stands in for a grey image in another pool (as in static groups).
            Err(_) if m.two_colour => {
                let m = MaterialDesc { palettes: Some([m.texture; 2]), ..*m };
                r.add_material(render_material(&m, &out.textures, true))?
            }
            Err(_) => r.add_material(render_material(m, &out.textures, false))?,
        };
        out.materials.push(id);
    }
    for d in g.draws {
        let base = match d.weights {
            Some(w) => {
                let n = g.meshes[d.mesh as usize].vertex_count as usize;
                Some(r.add_vertex_weights(&g.weights[w as usize..][..n])?)
            }
            None => None,
        };
        out.weight_bases.push(base);
    }
    Ok(out)
}

/// Places the static groups' meshes and one instance per draw (after `commit`); returns the draws' instances, in
/// [`GeometryRef::draws`] order.
pub fn place<B: Backend>(
    r: &mut Renderer<B>,
    g: GeometryRef<'_>,
    up: &Uploaded,
) -> Result<Vec<InstanceId>, RenderError> {
    for &(mesh, material, mirrored, weights) in &up.static_draws {
        let id = r.add_instance(mesh, material, Mat4::IDENTITY)?;
        r.set_mirrored(id, mirrored)?;
        if weights.is_some() {
            r.set_weights(id, weights)?;
        }
    }
    let mut instances = Vec::with_capacity(g.draws.len());
    for (d, base) in g.draws.iter().zip(&up.weight_bases) {
        let mesh = up.meshes[d.mesh as usize].ok_or(RenderError::InvalidHandle)?;
        let material = up.materials[d.material as usize];
        let id = r.add_instance(mesh, material, d.transform)?;
        if base.is_some() && r.material(material).is_some_and(|m| m.palettes.is_some()) {
            r.set_weights(id, *base)?;
        }
        if d.sort_key.is_some() {
            r.set_sort_key(id, d.sort_key)?;
        }
        if d.fog != 1.0 {
            r.set_fog_factor(id, d.fog)?;
        }
        instances.push(id);
    }
    Ok(instances)
}

/// Builds the placed meshes of static groups (after the level's textures): per group, one mesh per texture pool and
/// 65,535 vertices, with a material of the group's look in that pool (shared through `looks`). Adds (mesh, material,
/// mirrored, weights base) per draw to `out`. A group whose look is lit with two colours gets the pieces' weights
/// in vertex order (missing ones zero).
fn upload_static<'a, B: Backend>(
    r: &mut Renderer<B>,
    level: GeometryRef<'_>,
    groups: impl Iterator<Item = &'a StaticGroup>,
    textures: &[TextureId],
    looks: &mut BTreeMap<(u32, u32), MaterialId>,
    out: &mut Vec<(MeshId, MaterialId, bool, Option<u32>)>,
) -> Result<(), RenderError> {
    // One placed mesh and its weights.
    let mut place = |r: &mut Renderer<B>,
                     v: &[Vertex],
                     i: &[u16],
                     p: &[PlacedPiece],
                     w: &[VertexWeights],
                     material: MaterialId,
                     mirrored: bool|
     -> Result<(), RenderError> {
        let mesh = r.add_placed_mesh(&MeshData { vertices: v, indices: i }, p)?;
        let weights = if w.is_empty() { None } else { Some(r.add_vertex_weights(w)?) };
        out.push((mesh, material, mirrored, weights));
        Ok(())
    };
    for g in groups {
        // The pieces by pool, in draw order.
        let mut by_pool: BTreeMap<u32, Vec<&StaticPiece>> = BTreeMap::new();
        for p in &g.pieces {
            let t = textures[level.materials[p.material as usize].texture as usize];
            let pool = r.texture_pool(t).ok_or(RenderError::InvalidHandle)?;
            by_pool.entry(pool).or_default().push(p);
        }
        for (pool, pieces) in by_pool {
            let first = &level.materials[pieces[0].material as usize];
            let look = &level.materials[g.material as usize];
            // The grey image of a piece, when it shares the piece's pool.
            let grey = |r: &Renderer<B>, m: &MaterialDesc| {
                let t = textures[m.palettes?[0] as usize];
                (r.texture_pool(t) == Some(pool)).then_some(t)
            };
            let lit = look.two_colour;
            let material = match looks.get(&(g.material, pool)) {
                Some(&m) => m,
                None => {
                    // The pieces' grey images ride in their placements; the material only needs one in its pool.
                    let palettes = lit.then_some([first.texture; 2]);
                    let desc = MaterialDesc { texture: first.texture, tint: [255; 4], palettes, ..*look };
                    let m = r.add_material(render_material(&desc, textures, lit))?;
                    looks.insert((g.material, pool), m);
                    m
                }
            };
            let mut vertices: Vec<Vertex> = Vec::new();
            let mut indices: Vec<u16> = Vec::new();
            let mut placed: Vec<PlacedPiece> = Vec::new();
            let mut weights: Vec<VertexWeights> = Vec::new();
            for p in pieces {
                let src = level.meshes[p.mesh as usize];
                if vertices.len() + src.vertex_count as usize > 65535 {
                    place(r, &vertices, &indices, &placed, &weights, material, g.mirrored)?;
                    vertices.clear();
                    indices.clear();
                    placed.clear();
                    weights.clear();
                }
                let base = vertices.len() as u16;
                vertices.extend_from_slice(&level.vertices[src.first_vertex as usize..][..src.vertex_count as usize]);
                let tri = &level.indices[src.first_index as usize..][..src.index_count as usize];
                indices.extend(tri.iter().map(|&i| base + i));
                let n = src.vertex_count as usize;
                if lit {
                    match p.weights {
                        Some(w) => weights.extend_from_slice(&level.weights[w as usize..][..n]),
                        None => weights.extend(core::iter::repeat_n(VertexWeights::default(), n)),
                    }
                }
                let m = &level.materials[p.material as usize];
                placed.push(PlacedPiece {
                    vertices: base as u32..vertices.len() as u32,
                    transform: p.transform,
                    texture: textures[m.texture as usize],
                    tint: m.tint,
                    grey: if lit { grey(r, m).or(Some(textures[m.texture as usize])) } else { None },
                });
            }
            if !placed.is_empty() {
                place(r, &vertices, &indices, &placed, &weights, material, g.mirrored)?;
            }
        }
    }
    Ok(())
}
