//! Mesh sets (geometry.md): filtered meshes, the mesh header with the bone table, the four body types, colour tables
//! and copies. Strip decoding is Sly 2's ([`formats_sly2::geometry::strip_triangles`]).

use alloc::vec::Vec;

use formats::error::{Error, Result};
use formats::geometry::{Affine, GpuVertex};
use formats::reader::{Reader, le_f32};
use formats_sly2::geometry::strip_triangles_at;
use formats_sly2::transforms::read_matrix;

use crate::filter::{Context, Filter};

/// Body types (geometry.md 3.1).
pub const STRIPS: i8 = 0;
pub const OUTLINE: i8 = 1;
pub const EFFECT_OUTLINE: i8 = 2;
pub const INDEXED: i8 = 3;

/// One strip list: vertex count, shader index, flags, and its first index within the group's index list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StripList {
    pub count: u32,
    pub shader: i32,
    pub flags: i32,
    pub first: u32,
}

/// Bone slots and weights of a group: (offset of the slot bytes, offset of the weights, weights per vertex).
pub type Weights = (u32, u32, u8);

/// A vertex group. Offsets are absolute in the world data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Group {
    /// Strips (4.1).
    Strips {
        lists: Vec<StripList>,
        positions: u8,
        pos_at: u32,
        uv_at: u32,
        normal_at: u32,
        color_at: u32,
        weights: Option<Weights>,
        indices_at: u32,
        breaks_at: u32,
    },
    /// Indexed (4.5).
    Indexed {
        shader: u16,
        count: u8,
        positions: u8,
        pos_at: u32,
        weights: Option<Weights>,
        indices_at: u32,
        breaks_at: u32,
    },
    /// Outline (4.3) or effect outline (4.4, with a shader).
    Outline { shader: Option<u16>, vertices: u8, pos_at: u32, weights: Option<Weights>, edges: u8, edges_at: u32 },
}

/// One set of a colour table: a vertex group and its records (5 bytes each: colour A, colour B, amount).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorSet {
    pub group: u16,
    pub count: u16,
    pub records_at: u32,
}

/// A colour table (4.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColorTable {
    pub condition: i32,
    pub sets: Vec<ColorSet>,
}

/// A mesh copy (6).
#[derive(Clone, Debug, PartialEq)]
pub struct MeshCopy {
    pub transform: i16,
    pub matrix: Affine,
    pub hidden: bool,
    pub color_tables: Vec<ColorTable>,
}

/// One mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// World filter and conditions of a conditional mesh (empty: unconditional).
    pub filter: Filter,
    pub body_type: i8,
    pub transform: i16,
    pub pass: i8,
    pub draw_order: f32,
    /// Not drawn until gameplay shows it.
    pub hidden: bool,
    pub sphere: Option<([f32; 3], f32)>,
    pub max_distance: Option<f32>,
    /// Bone table: (object ID, transform reference).
    pub bones: Vec<(i32, i16)>,
    /// The mesh has no body.
    pub no_body: bool,
    pub fog: f32,
    pub billboard: i8,
    /// Strips body flags (0 for other bodies).
    pub flags: u8,
    pub groups: Vec<Group>,
    pub color_tables: Vec<ColorTable>,
    /// Outline colour (outline bodies).
    pub outline_color: Option<u32>,
    pub copies: Vec<MeshCopy>,
    /// Offset of the mesh header.
    pub offset: u32,
}

/// A mesh set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshSet {
    pub no_bounds: bool,
    pub animations: Vec<(u16, u32)>,
    pub slots: i16,
    pub meshes: Vec<Mesh>,
    pub tree_nodes: i16,
}

fn break_words(n: usize) -> usize {
    n.div_ceil(32) * 4
}

/// Reads colour tables (4.2).
pub fn color_tables(r: &mut Reader<'_>) -> Result<Vec<ColorTable>> {
    let n = r.u8()?;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let condition = r.i32()?;
        let s = r.u16()?;
        let mut sets = Vec::with_capacity(s as usize);
        for _ in 0..s {
            let group = r.u16()?;
            let count = r.u16()?;
            let records_at = r.pos() as u32;
            r.skip_n(count as usize, 5)?;
            sets.push(ColorSet { group, count, records_at });
        }
        out.push(ColorTable { condition, sets });
    }
    Ok(out)
}

fn weights(r: &mut Reader<'_>, bones: usize, positions: usize) -> Result<Option<Weights>> {
    if bones == 0 {
        return Ok(None);
    }
    let slots_at = r.pos() as u32;
    r.skip(4)?;
    let k = bones.min(4);
    let at = r.pos() as u32;
    r.skip(k * positions)?;
    Ok(Some((slots_at, at, k as u8)))
}

fn read_strips(r: &mut Reader<'_>, bones: usize, mesh: &mut Mesh) -> Result<()> {
    let flags = r.u8()?;
    mesh.flags = flags;
    r.skip(1)?;
    if r.i8()? != -1 {
        r.skip(32)?;
    }
    mesh.color_tables = color_tables(r)?;
    if flags & 0x20 != 0 {
        r.skip(4)?;
        if r.i8()? != -1 {
            r.skip(16 + 48)?;
        }
        r.skip(3)?;
    }
    if flags & 0x08 != 0 {
        r.skip(36)?;
    }
    let n = r.i16()?;
    for _ in 0..n.max(0) {
        let l = r.u8()?;
        let mut lists = Vec::with_capacity(l as usize);
        let mut first = 0u32;
        for _ in 0..l {
            let count = r.u32()?;
            let shader = r.i32()?;
            let flags = r.i32()?;
            if count > 0x10000 {
                return Err(r.corrupt("strip-list vertex count"));
            }
            lists.push(StripList { count, shader, flags, first });
            first += count;
        }
        let p = r.u8()? as usize;
        let pos_at = r.pos() as u32;
        r.skip(p * 12)?;
        let uv_at = r.pos() as u32;
        r.skip(p * 8)?;
        let normal_at = r.pos() as u32;
        r.skip(p * 12)?;
        let color_at = r.pos() as u32;
        r.skip(p * 4)?;
        r.skip(1)?;
        let weights = weights(r, bones, p)?;
        let indices_at = r.pos() as u32;
        r.skip(first as usize)?;
        let breaks_at = r.pos() as u32;
        r.skip(break_words(first as usize))?;
        mesh.groups.push(Group::Strips {
            lists,
            positions: p as u8,
            pos_at,
            uv_at,
            normal_at,
            color_at,
            weights,
            indices_at,
            breaks_at,
        });
    }
    Ok(())
}

fn read_outline_groups(r: &mut Reader<'_>, bones: usize, shaded: bool, mesh: &mut Mesh) -> Result<()> {
    let n = r.i16()?;
    for _ in 0..n.max(0) {
        let shader = if shaded { Some(r.u16()?) } else { None };
        let vertices = r.u8()?;
        let pos_at = r.pos() as u32;
        r.skip(vertices as usize * 12)?;
        let weights = if bones > 0 {
            let k = r.u8()?;
            let slots_at = r.pos() as u32;
            r.skip(k as usize)?;
            let at = r.pos() as u32;
            r.skip(k as usize * vertices as usize)?;
            Some((slots_at, at, k))
        } else {
            None
        };
        let edges = r.u8()?;
        let edges_at = r.pos() as u32;
        r.skip(edges as usize * 4)?;
        mesh.groups.push(Group::Outline { shader, vertices, pos_at, weights, edges, edges_at });
    }
    Ok(())
}

fn read_indexed(r: &mut Reader<'_>, bones: usize, mesh: &mut Mesh) -> Result<()> {
    let n = r.i16()?;
    for _ in 0..n.max(0) {
        let shader = r.u16()?;
        let count = r.u8()?;
        let positions = r.u8()?;
        let pos_at = r.pos() as u32;
        r.skip(positions as usize * 12)?;
        let weights = weights(r, bones, positions as usize)?;
        let indices_at = r.pos() as u32;
        r.skip(count as usize)?;
        let breaks_at = r.pos() as u32;
        r.skip(break_words(count as usize))?;
        mesh.groups.push(Group::Indexed { shader, count, positions, pos_at, weights, indices_at, breaks_at });
    }
    Ok(())
}

fn read_mesh(r: &mut Reader<'_>, no_bounds: bool, filter: Filter) -> Result<Mesh> {
    let offset = r.pos() as u32;
    let body_type = r.i8()?;
    let copies = r.u16()?;
    let transform = r.i16()?;
    let pass = r.i8()?;
    let draw_order = r.f32()?;
    r.skip(4)?;
    let hidden = r.i8()? & 1 != 0;
    let (sphere, max_distance) = if no_bounds {
        (None, None)
    } else {
        let c = r.vec3()?;
        let radius = r.f32()?;
        r.skip(4)?;
        (Some((c, radius)), Some(r.f32()?))
    };
    let b = r.u8()?;
    let bones = (0..b).map(|_| Ok((r.i32()?, r.i16()?))).collect::<Result<Vec<_>>>()?;
    let no_body = r.i8()? != 0;
    let mut mesh = Mesh {
        filter,
        body_type,
        transform,
        pass,
        draw_order,
        hidden,
        sphere,
        max_distance,
        bones,
        no_body,
        fog: 0.0,
        billboard: -1,
        flags: 0,
        groups: Vec::new(),
        color_tables: Vec::new(),
        outline_color: None,
        copies: Vec::new(),
        offset,
    };
    if no_body {
        if copies != 0 {
            return Err(Error::Corrupt { offset: offset as usize, what: "copies on a mesh without a body" });
        }
        return Ok(mesh);
    }
    mesh.fog = r.f32()?;
    mesh.billboard = r.i8()?;
    let nb = mesh.bones.len();
    match body_type {
        STRIPS => read_strips(r, nb, &mut mesh)?,
        OUTLINE => {
            mesh.outline_color = Some(r.u32()?);
            read_outline_groups(r, nb, false, &mut mesh)?;
        }
        EFFECT_OUTLINE => read_outline_groups(r, nb, true, &mut mesh)?,
        INDEXED => read_indexed(r, nb, &mut mesh)?,
        _ => return Err(Error::Corrupt { offset: offset as usize, what: "unknown mesh body type" }),
    }
    for _ in 0..copies {
        r.skip(2)?;
        if !no_bounds {
            r.skip(4)?;
        }
        let transform = r.i16()?;
        let matrix = read_matrix(r)?;
        r.skip(4)?;
        let hidden = r.i8()? & 1 != 0;
        let color_tables = color_tables(r)?;
        mesh.copies.push(MeshCopy { transform, matrix, hidden, color_tables });
    }
    Ok(mesh)
}

impl MeshSet {
    pub fn read(r: &mut Reader<'_>) -> Result<MeshSet> {
        let no_bounds = r.i8()? != 0;
        r.skip(16)?;
        let n = r.i8()?;
        let mut animations = Vec::new();
        for _ in 0..n.max(0) {
            let class = r.u16()?;
            if class == 0xFFFF {
                animations.push((class, 0));
            } else {
                animations.push((class, r.pos() as u32));
                r.skip(36)?;
            }
        }
        let n = r.u8()? as usize;
        r.skip_n(n, 4)?;
        let slots = r.i16()?;
        let mut used = 0i32;
        let mut meshes = Vec::new();
        while used < slots as i32 {
            let f = r.u8()?;
            let c = r.u8()?;
            if f != 0 || c != 0 {
                let filter = Filter::read(r, f as usize, c as usize)?;
                let size = r.i32()?;
                let start = r.pos();
                let mesh = read_mesh(r, no_bounds, filter)?;
                if size < 0 || r.pos() - start != size as usize {
                    return Err(Error::Corrupt { offset: start, what: "filtered mesh size mismatch" });
                }
                meshes.push(mesh);
                used += 1;
            } else {
                let mesh = read_mesh(r, no_bounds, Filter::default())?;
                used += 1 + mesh.copies.len() as i32;
                meshes.push(mesh);
            }
        }
        let tree_nodes = r.i16()?;
        for _ in 0..tree_nodes.max(0) {
            for _ in 0..4 {
                let kind = (r.i8()? << 4) >> 4;
                if kind == -1 {
                    break;
                }
                r.skip(12 + 4 + 4 + 1)?;
                if kind == 0 || kind == 1 {
                    r.skip(2)?;
                }
            }
        }
        Ok(MeshSet { no_bounds, animations, slots, meshes, tree_nodes })
    }
}

fn break_bit(data: &[u8], breaks_at: usize, i: usize) -> bool {
    let w = breaks_at + (i / 32) * 4;
    data.get(w..w + 4).is_some_and(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) & (1 << (i % 32)) != 0)
}

impl Group {
    /// Triangles of every strip list: (shader index, triangles of position indices). Outlines have none.
    pub fn triangles(&self, data: &[u8]) -> Vec<(i32, Vec<[u8; 3]>)> {
        match self {
            Group::Strips { lists, indices_at, breaks_at, positions, .. } => lists
                .iter()
                .map(|l| {
                    let start = *indices_at as usize + l.first as usize;
                    let first = l.first as usize;
                    let bit = |i| break_bit(data, *breaks_at as usize, first + i);
                    (l.shader, strip_triangles_at(data, start, l.count as usize, *positions as usize, bit))
                })
                .collect(),
            Group::Indexed { shader, count, indices_at, breaks_at, positions, .. } => {
                let bit = |i| break_bit(data, *breaks_at as usize, i);
                let t = strip_triangles_at(data, *indices_at as usize, *count as usize, *positions as usize, bit);
                alloc::vec![(*shader as i32, t)]
            }
            Group::Outline { .. } => Vec::new(),
        }
    }

    /// Outline edges (4 bytes each), 0 for other groups.
    pub fn edges(&self) -> usize {
        match self {
            Group::Outline { edges, .. } => *edges as usize,
            _ => 0,
        }
    }

    /// Per-vertex weights: for vertex `v`, the bone slots and the weights (each vertex's weights together).
    pub fn vertex_weights(&self, data: &[u8], v: usize) -> Option<(Vec<u8>, Vec<u8>)> {
        let (slots_at, at, k) = match self {
            Group::Strips { weights, .. } | Group::Indexed { weights, .. } | Group::Outline { weights, .. } => {
                (*weights)?
            }
        };
        let k = k as usize;
        let slots = data.get(slots_at as usize..slots_at as usize + k)?.to_vec();
        let w = data.get(at as usize + v * k..at as usize + v * k + k)?.to_vec();
        Some((slots, w))
    }

    /// Vertices for the GPU: position, normal, UV and colour (strips); positions only otherwise.
    pub fn vertices(&self, data: &[u8]) -> Vec<GpuVertex> {
        let f = |o: usize| le_f32(data, o).unwrap_or(0.0);
        let v3 = |o: usize| [f(o), f(o + 4), f(o + 8)];
        let plain = |n: usize, pos_at: u32| -> Vec<GpuVertex> {
            (0..n)
                .map(|i| GpuVertex {
                    position: v3(pos_at as usize + i * 12),
                    normal: [0.0; 3],
                    uv: [0.0; 2],
                    color: [0xFF; 4],
                })
                .collect()
        };
        match self {
            Group::Strips { positions, pos_at, uv_at, normal_at, color_at, .. } => (0..*positions as usize)
                .map(|i| {
                    let c = *color_at as usize + i * 4;
                    GpuVertex {
                        position: v3(*pos_at as usize + i * 12),
                        normal: v3(*normal_at as usize + i * 12),
                        uv: [f(*uv_at as usize + i * 8), f(*uv_at as usize + i * 8 + 4)],
                        color: [data[c], data[c + 1], data[c + 2], data[c + 3]],
                    }
                })
                .collect(),
            Group::Indexed { positions, pos_at, .. } => plain(*positions as usize, *pos_at),
            Group::Outline { vertices, pos_at, .. } => plain(*vertices as usize, *pos_at),
        }
    }
}

impl Mesh {
    /// Triangles of all groups (copies not multiplied).
    pub fn triangle_count(&self, data: &[u8]) -> usize {
        self.groups.iter().flat_map(|g| g.triangles(data)).map(|(_, t)| t.len()).sum()
    }

    /// Outline edges of all groups.
    pub fn edge_count(&self) -> usize {
        self.groups.iter().map(|g| g.edges()).sum()
    }

    /// The colour table in effect (4.2): the last one whose condition matches.
    pub fn color_table(&self, ctx: &Context) -> Option<&ColorTable> {
        self.color_tables
            .iter()
            .rev()
            .find(|t| Filter { slots: Vec::new(), conditions: alloc::vec![t.condition] }.matches(ctx))
    }
}

/// A 15-bit colour of a colour table as RGBA8 (5 bits per channel shifted left by 3; top bit alpha 0x80).
pub fn color15(c: u16) -> [u8; 4] {
    let ch = |s: u16| (((c >> s) & 0x1F) << 3) as u8;
    [ch(0), ch(5), ch(10), if c & 0x8000 != 0 { 0x80 } else { 0 }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_and_breaks() {
        assert_eq!(color15(0x801F), [0xF8, 0, 0, 0x80]);
        assert_eq!(color15(0x7FE0), [0, 0xF8, 0xF8, 0]);
        let d = [0b111u8, 0, 0, 0];
        assert!(break_bit(&d, 0, 2) && !break_bit(&d, 0, 3));
    }

    /// geometry.md 8: the first v_apt mesh's strip lists (1, 1, 2, 3) with breaks on the first three and (0, 1, 2)
    /// with breaks on the first two give (2, 1, 3) and (0, 1, 2).
    #[test]
    fn strip_vector() {
        let mut d = alloc::vec![1u8, 1, 2, 3, 0, 1, 2, 0];
        let breaks: u32 = 0b0111 | (0b011 << 4);
        d.extend_from_slice(&breaks.to_le_bytes());
        let g = Group::Strips {
            lists: alloc::vec![
                StripList { count: 4, shader: 50, flags: 35, first: 0 },
                StripList { count: 3, shader: 50, flags: 35, first: 4 }
            ],
            positions: 4,
            pos_at: 0,
            uv_at: 0,
            normal_at: 0,
            color_at: 0,
            weights: None,
            indices_at: 0,
            breaks_at: 8,
        };
        let t = g.triangles(&d);
        assert_eq!(t[0].1, [[2, 1, 3]]);
        assert_eq!(t[1].1, [[0, 1, 2]]);
    }
}
