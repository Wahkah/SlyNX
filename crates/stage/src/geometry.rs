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
    /// The zone mask its pieces share ([`ZoneVisibility`]).
    pub zone: u32,
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
    /// Per draw: its zone mask (empty: every draw is always shown).
    pub draw_zones: &'a [u32],
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
    /// Per draw: its zone mask (empty: always shown).
    pub draw_zones: Vec<u32>,
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
            draw_zones: &self.draw_zones,
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
    /// (mesh, material, mirrored, weights base, zone mask) per placed mesh.
    pub static_draws: Vec<(MeshId, MaterialId, bool, Option<u32>, u32)>,
}

/// The camera's mask that shows every zone (the camera outside every visibility volume).
pub const ALL_ZONES: u32 = 0xFF;

/// Draws shown only while their zone mask shares a bit with the camera's visibility mask (Sly 2 camera.md 0; the
/// engine's `World::shared.visibility_mask`). Draws whose mask covers every zone are not tracked.
#[derive(Clone, Debug, Default)]
pub struct ZoneVisibility {
    entries: Vec<(InstanceId, u32)>,
    mask: u32,
}

impl ZoneVisibility {
    /// The instances of a placed level (`draws` from [`place`], `statics` from [`place_all`]) and their masks.
    pub fn new(g: GeometryRef<'_>, up: &Uploaded, draws: &[InstanceId], statics: &[InstanceId]) -> ZoneVisibility {
        let mut entries = Vec::new();
        for (&id, &zone) in draws.iter().zip(g.draw_zones) {
            if zone & ALL_ZONES != ALL_ZONES {
                entries.push((id, zone));
            }
        }
        for (&id, s) in statics.iter().zip(&up.static_draws) {
            if s.4 & ALL_ZONES != ALL_ZONES {
                entries.push((id, s.4));
            }
        }
        ZoneVisibility { entries, mask: ALL_ZONES }
    }

    /// Instances tracked.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Shows and hides the tracked instances for the camera's `mask` ([`ALL_ZONES`] shows all); does nothing while
    /// the mask is unchanged.
    pub fn apply<B: Backend>(&mut self, r: &mut Renderer<B>, mask: u32) -> Result<(), RenderError> {
        if mask == self.mask {
            return Ok(());
        }
        self.mask = mask;
        for &(id, zone) in &self.entries {
            r.set_visible(id, mask == ALL_ZONES || zone & mask != 0)?;
        }
        Ok(())
    }
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
    check(g)?;
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

/// Checks that every index between the prepared arrays is in range, so a damaged level fails with an error instead
/// of a panic.
pub fn check(g: GeometryRef<'_>) -> Result<(), RenderError> {
    let bad = |what| Err(RenderError::BadMesh(what));
    let (nm, nt, nmat) = (g.meshes.len(), g.textures.len(), g.materials.len());
    for m in g.meshes {
        let v = (m.first_vertex as usize).checked_add(m.vertex_count as usize);
        let i = (m.first_index as usize).checked_add(m.index_count as usize);
        if v.is_none_or(|v| v > g.vertices.len()) || i.is_none_or(|i| i > g.indices.len()) {
            return bad("prepared mesh outside the level's arrays");
        }
        if m.skinned && v.is_none_or(|v| v > g.skin.len()) {
            return bad("skinned mesh without skin data");
        }
    }
    for m in g.materials {
        if m.texture as usize >= nt || m.palettes.is_some_and(|p| p.iter().any(|&t| t as usize >= nt)) {
            return bad("material texture out of range");
        }
    }
    let piece_ok = |mesh: u32, material: u32| (mesh as usize) < nm && (material as usize) < nmat;
    for d in g.draws {
        if !piece_ok(d.mesh, d.material) {
            return bad("draw mesh or material out of range");
        }
        // Its weights: one per vertex of its mesh, inside the level's weights.
        let n = g.meshes[d.mesh as usize].vertex_count as usize;
        if d.weights.is_some_and(|w| (w as usize).checked_add(n).is_none_or(|e| e > g.weights.len())) {
            return bad("draw weights outside the level's weights");
        }
    }
    for s in g.static_groups {
        if (s.material as usize) >= nmat || s.pieces.iter().any(|p| !piece_ok(p.mesh, p.material)) {
            return bad("static group mesh or material out of range");
        }
    }
    Ok(())
}

/// Places the static groups' meshes and one instance per draw (after `commit`); returns the draws' instances, in
/// [`GeometryRef::draws`] order.
pub fn place<B: Backend>(
    r: &mut Renderer<B>,
    g: GeometryRef<'_>,
    up: &Uploaded,
) -> Result<Vec<InstanceId>, RenderError> {
    place_all(r, g, up).map(|(draws, _)| draws)
}

/// [`place`], also returning the placed meshes' instances (in [`Uploaded::static_draws`] order).
pub fn place_all<B: Backend>(
    r: &mut Renderer<B>,
    g: GeometryRef<'_>,
    up: &Uploaded,
) -> Result<(Vec<InstanceId>, Vec<InstanceId>), RenderError> {
    let mut statics = Vec::with_capacity(up.static_draws.len());
    for &(mesh, material, mirrored, weights, _) in &up.static_draws {
        let id = r.add_instance(mesh, material, Mat4::IDENTITY)?;
        r.set_mirrored(id, mirrored)?;
        if weights.is_some() {
            r.set_weights(id, weights)?;
        }
        statics.push(id);
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
    Ok((instances, statics))
}

/// Most vertices in one placed mesh (16-bit indices).
pub const MAX_PLACED_VERTICES: usize = 65535;

/// A prepared mesh's vertices and indices; an error if its ranges lie outside the level's arrays.
fn mesh_slices<'a>(level: GeometryRef<'a>, m: &MeshRange) -> Result<(&'a [Vertex], &'a [u16]), RenderError> {
    let bad = || RenderError::BadMesh("prepared mesh outside the level's arrays");
    let v =
        level.vertices.get(m.first_vertex as usize..).and_then(|v| v.get(..m.vertex_count as usize)).ok_or_else(bad)?;
    let i =
        level.indices.get(m.first_index as usize..).and_then(|i| i.get(..m.index_count as usize)).ok_or_else(bad)?;
    Ok((v, i))
}

/// Appends one piece to a placed mesh being built, its indices moved past the vertices already there; returns the
/// first vertex. Refuses a piece whose indices point past its own vertices or that would not fit 16-bit indices.
fn append_piece(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
    src_vertices: &[Vertex],
    src_indices: &[u16],
) -> Result<u16, RenderError> {
    if vertices.len() + src_vertices.len() > MAX_PLACED_VERTICES {
        return Err(RenderError::BadMesh("placed piece past 65,535 vertices"));
    }
    if src_indices.iter().any(|&i| i as usize >= src_vertices.len()) {
        return Err(RenderError::BadMesh("piece index past its vertices"));
    }
    let base = vertices.len() as u16;
    vertices.extend_from_slice(src_vertices);
    // Below 65,535 by the checks above.
    indices.extend(src_indices.iter().map(|&i| base + i));
    Ok(base)
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
    out: &mut Vec<(MeshId, MaterialId, bool, Option<u32>, u32)>,
) -> Result<(), RenderError> {
    // One placed mesh and its weights.
    let mut place = |r: &mut Renderer<B>,
                     v: &[Vertex],
                     i: &[u16],
                     p: &[PlacedPiece],
                     w: &[VertexWeights],
                     material: MaterialId,
                     mirrored: bool,
                     zone: u32|
     -> Result<(), RenderError> {
        let mesh = r.add_placed_mesh(&MeshData { vertices: v, indices: i }, p)?;
        let weights = if w.is_empty() { None } else { Some(r.add_vertex_weights(w)?) };
        out.push((mesh, material, mirrored, weights, zone));
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
                let src = *level.meshes.get(p.mesh as usize).ok_or(RenderError::InvalidHandle)?;
                let (src_vertices, src_indices) = mesh_slices(level, &src)?;
                if !placed.is_empty() && vertices.len() + src_vertices.len() > MAX_PLACED_VERTICES {
                    place(r, &vertices, &indices, &placed, &weights, material, g.mirrored, g.zone)?;
                    vertices.clear();
                    indices.clear();
                    placed.clear();
                    weights.clear();
                }
                let base = append_piece(&mut vertices, &mut indices, src_vertices, src_indices)?;
                let n = src_vertices.len();
                if lit {
                    match p.weights.and_then(|w| level.weights.get(w as usize..)?.get(..n)) {
                        Some(w) => weights.extend_from_slice(w),
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
                place(r, &vertices, &indices, &placed, &weights, material, g.mirrored, g.zone)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verts(n: usize) -> Vec<Vertex> {
        alloc::vec![Vertex::new([0.0; 3], [0.0; 2], [0.0, 0.0, 1.0], [255; 4]); n]
    }

    #[test]
    fn pieces_append_within_16_bit_indices() {
        let (mut v, mut i) = (Vec::new(), Vec::new());
        assert_eq!(append_piece(&mut v, &mut i, &verts(3), &[0, 1, 2]).unwrap(), 0);
        assert_eq!(append_piece(&mut v, &mut i, &verts(3), &[2, 1, 0]).unwrap(), 3);
        assert_eq!(i, [0, 1, 2, 5, 4, 3]);
        // An index past the piece's own vertices is refused, not wrapped into the next piece.
        assert!(append_piece(&mut v, &mut i, &verts(3), &[0, 1, 3]).is_err());
        // A piece that would pass 65,535 vertices is refused instead of overflowing the indices.
        let big = verts(MAX_PLACED_VERTICES);
        assert!(append_piece(&mut v, &mut i, &big, &[0, 1, 2]).is_err());
        let (mut v2, mut i2) = (Vec::new(), Vec::new());
        assert!(append_piece(&mut v2, &mut i2, &verts(MAX_PLACED_VERTICES + 1), &[0, 1, 2]).is_err());
        assert_eq!(append_piece(&mut v2, &mut i2, &big, &[0, 1, 65534]).unwrap(), 0);
    }

    #[test]
    fn damaged_levels_fail_the_check() {
        let mut g = Geometry::default();
        g.textures.push(TextureData::rgba8(1, 1, alloc::vec![255; 4]).unwrap());
        g.vertices = verts(3);
        g.indices = alloc::vec![0, 1, 2];
        g.meshes.push(MeshRange { first_vertex: 0, vertex_count: 3, first_index: 0, index_count: 3, skinned: false });
        let look = DrawLook {
            pass: Pass::Opaque,
            order: 5,
            blend: BlendMode::None,
            sort: SortMode::Grouped,
            two_step: false,
            no_darkening: false,
        };
        g.materials.push(MaterialDesc {
            texture: 0,
            palettes: None,
            look,
            tint: [255; 4],
            cutoff: None,
            two_colour: false,
        });
        let draw =
            DrawItem { mesh: 0, material: 0, transform: Mat4::IDENTITY, weights: None, sort_key: None, fog: 1.0 };
        g.draws.push(draw);
        assert!(check(g.as_ref()).is_ok());
        g.draws.push(DrawItem { mesh: 9, ..draw });
        assert!(check(g.as_ref()).is_err());
        g.draws.pop();
        // Weights past the level's: refused (three vertices, two weights).
        g.weights = alloc::vec![VertexWeights::default(); 2];
        g.draws[0].weights = Some(0);
        assert!(check(g.as_ref()).is_err());
        g.weights.push(VertexWeights::default());
        assert!(check(g.as_ref()).is_ok());
        g.draws[0].weights = Some(u32::MAX);
        assert!(check(g.as_ref()).is_err());
        g.draws[0].weights = None;
        g.meshes[0].vertex_count = 4;
        assert!(check(g.as_ref()).is_err());
        // The upload itself refuses it (no panic).
        let mut r = Renderer::new(render::NullBackend::new(64, 64));
        assert!(upload(&mut r, g.as_ref()).is_err());
    }

    #[test]
    fn zone_masks_hide_and_show() {
        let mut g = Geometry::default();
        g.textures.push(TextureData::rgba8(1, 1, alloc::vec![255; 4]).unwrap());
        g.vertices = verts(3);
        g.indices = alloc::vec![0, 1, 2];
        g.meshes.push(MeshRange { first_vertex: 0, vertex_count: 3, first_index: 0, index_count: 3, skinned: false });
        let look = DrawLook {
            pass: Pass::Opaque,
            order: 5,
            blend: BlendMode::None,
            sort: SortMode::Grouped,
            two_step: false,
            no_darkening: false,
        };
        g.materials.push(MaterialDesc {
            texture: 0,
            palettes: None,
            look,
            tint: [255; 4],
            cutoff: None,
            two_colour: false,
        });
        let draw =
            DrawItem { mesh: 0, material: 0, transform: Mat4::IDENTITY, weights: None, sort_key: None, fog: 1.0 };
        g.draws = alloc::vec![draw; 3];
        // Zone 1, zone 2, and every zone.
        g.draw_zones = alloc::vec![1, 2, u32::MAX];
        let mut r = Renderer::new(render::NullBackend::new(64, 64));
        let up = upload(&mut r, g.as_ref()).unwrap();
        r.commit().unwrap();
        let (draws, statics) = place_all(&mut r, g.as_ref(), &up).unwrap();
        let mut z = ZoneVisibility::new(g.as_ref(), &up, &draws, &statics);
        assert_eq!(z.len(), 2, "draws in every zone are not tracked");
        let cam = render::Camera {
            eye: render::math::Vec3::new(0.0, -5.0, 0.0),
            target: render::math::Vec3::ZERO,
            up: render::math::Vec3::Z,
            fov_y: 1.0,
            near: 0.1,
            far: 100.0,
        };
        let shown = |r: &mut Renderer<render::NullBackend>| r.render(&cam, [0.0; 4]).unwrap().visible;
        z.apply(&mut r, 1).unwrap();
        let one = shown(&mut r);
        z.apply(&mut r, ALL_ZONES).unwrap();
        assert_eq!(shown(&mut r), one + 1, "the zone-2 draw comes back");
        z.apply(&mut r, 4).unwrap();
        assert_eq!(shown(&mut r), one - 1, "only the all-zone draw is left");
    }
}
