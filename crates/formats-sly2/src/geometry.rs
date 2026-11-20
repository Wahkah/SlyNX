//! Mesh sets (geometry.md): meshes with slots, per-level variants and copies; rigid, skinned, streamed and indexed
//! bodies; strips and outline edges.

use alloc::vec::Vec;

use formats::error::{Error, Result};
use formats::geometry::{Affine, GpuVertex};
use formats::reader::{Reader, le_f32};

use crate::transforms::read_matrix;

/// Body types.
pub const RIGID: i8 = 0;
pub const SKINNED: i8 = 1;
pub const STREAMED: i8 = 2;
pub const INDEXED: i8 = 3;

/// One strip list of a rigid group: its vertex count, shader index and flags, and where its indices start within
/// the group's concatenated index list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StripList {
    pub count: u32,
    pub shader: i32,
    pub flags: i32,
    pub first: u32,
}

/// A vertex group. Offsets are absolute in the world data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Group {
    /// Rigid (4.1): positions, UVs, normals, colours, strip lists sharing the positions.
    Rigid {
        lists: Vec<StripList>,
        positions: u8,
        pos_at: u32,
        uv_at: u32,
        normal_at: u32,
        color_at: u32,
        indices_at: u32,
        breaks_at: u32,
        /// Skin data (geometry.md 5): offset of the four slot bytes, 0 without a bone table.
        skin_at: u32,
    },
    /// Indexed (4.2): positions only, one strip list.
    Indexed { shader: u16, count: u8, positions: u8, pos_at: u32, indices_at: u32, breaks_at: u32, skin_at: u32 },
    /// Skinned (4.3) or streamed (4.4, with a shader): an outline hull, no triangles.
    Outline {
        shader: Option<u16>,
        vertices: u8,
        pos_at: u32,
        edges: u8,
        edges_at: u32,
        /// Skin data (geometry.md 5): offset of the slot count byte, 0 without a bone table.
        skin_at: u32,
    },
}

/// The bone weights of a vertex group (geometry.md 5): per vertex, `per_vertex` (slot, weight) pairs. A slot is an
/// index into the mesh's bone table; a weight `w` means `w / 255`, and every vertex's weights sum to 255.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkinWeights {
    /// Bone-table index of each slot.
    pub slots: Vec<u8>,
    /// Weights, vertex by vertex, one per slot.
    pub weights: Vec<u8>,
}

impl SkinWeights {
    /// Slots per vertex.
    pub fn per_vertex(&self) -> usize {
        self.slots.len()
    }

    /// Vertex `v`'s weights, one per slot.
    pub fn vertex(&self, v: usize) -> &[u8] {
        let k = self.slots.len();
        self.weights.get(v * k..v * k + k).unwrap_or(&[])
    }
}

/// A mesh copy (geometry.md 7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshCopy {
    pub transform: i16,
    pub matrix: Affine,
    /// Offset of the copy's own vertex-map table (4.5; [`VertexMap::read_at`]).
    pub vertex_map_at: u32,
}

/// A vertex-map table (4.5): pre-baked colour sets, each `(count, offset of its 5-byte records)`, and the set index
/// of each submesh (strip list, in body order; negative: none).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VertexMap {
    pub sets: Vec<(u16, u32)>,
    pub by_submesh: Vec<i16>,
}

impl VertexMap {
    /// The table at `at` in `data`; empty if it is cut short.
    pub fn read_at(data: &[u8], at: u32) -> VertexMap {
        if at as usize > data.len() {
            return VertexMap::default();
        }
        vertex_map(&mut Reader::at(data, at as usize)).unwrap_or_default()
    }

    /// The set (count, records offset) of submesh `k` (strip lists counted across the body in order).
    pub fn set_of(&self, k: usize) -> Option<(u16, u32)> {
        let i = usize::try_from(*self.by_submesh.get(k)?).ok()?;
        self.sets.get(i).copied()
    }
}

/// One mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// Level rows of a conditional mesh (empty: unconditional).
    pub conditions: Vec<i16>,
    pub body_type: i8,
    pub transform: i16,
    pub pass: i8,
    pub max_distance: f32,
    pub sphere: Option<([f32; 3], f32)>,
    /// Bone table: (object ID, transform reference).
    pub bones: Vec<(i32, i16)>,
    pub groups: Vec<Group>,
    pub copies: Vec<MeshCopy>,
    /// Offset of the mesh header.
    pub offset: u32,
    /// Rigid bodies: the flags byte (4.1; bit 0: lit per frame, rendering.md 2.3). 0 for other bodies.
    pub body_flags: u8,
    /// Rigid bodies: the pre-baked colours (4.5).
    pub vertex_map: VertexMap,
}

/// A mesh set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshSet {
    pub no_bounds: bool,
    pub slots: i16,
    pub meshes: Vec<Mesh>,
    pub tree_nodes: i16,
}

/// A bone table, the vertex groups, the flags byte and the vertex-map table of a body (rigid bodies only have the last
/// two).
type BodyParts = (Vec<(i32, i16)>, Vec<Group>, u8, VertexMap);

fn bone_table(r: &mut Reader<'_>) -> Result<Vec<(i32, i16)>> {
    let b = r.u8()?;
    (0..b).map(|_| Ok((r.i32()?, r.i16()?))).collect()
}

/// Vertex-map table (4.5).
fn vertex_map(r: &mut Reader<'_>) -> Result<VertexMap> {
    let mut out = VertexMap::default();
    let m = r.u16()?;
    if m > 0 {
        for _ in 0..m {
            let c = r.u16()?;
            out.sets.push((c, r.pos() as u32));
            r.skip_n(c as usize, 5)?;
        }
        let c = r.u16()? as usize;
        for _ in 0..c {
            out.by_submesh.push(r.i16()?);
        }
    }
    Ok(out)
}

fn break_words(n: usize) -> usize {
    n.div_ceil(32) * 4
}

fn read_rigid(r: &mut Reader<'_>) -> Result<BodyParts> {
    let flags = r.u8()?;
    r.skip(2)?;
    let bones = bone_table(r)?;
    if r.i8()? != -1 {
        r.skip(32)?;
    }
    let map = vertex_map(r)?;
    if flags & 0x04 != 0 {
        r.skip(4)?;
        if r.i8()? != -1 {
            r.skip(16 + 48)?;
        }
        r.skip(3)?;
    }
    let n = r.i16()?;
    let mut groups = Vec::with_capacity(n.max(0) as usize);
    for _ in 0..n.max(0) {
        let l = r.u8()?;
        let mut lists = Vec::with_capacity(l as usize);
        let mut first = 0u32;
        for _ in 0..l {
            let count = r.i32()?;
            let shader = r.i32()?;
            let flags = r.i32()?;
            // At most 255 positions, so far fewer strip entries; anything larger is corrupt.
            if !(0..=MAX_STRIP_ENTRIES as i32).contains(&count) {
                return Err(r.corrupt("strip-list vertex count out of range"));
            }
            lists.push(StripList { count: count as u32, shader, flags, first });
            first = first.checked_add(count as u32).ok_or_else(|| r.corrupt("strip lists too long"))?;
        }
        let p = r.u8()?;
        let pos_at = r.pos() as u32;
        r.skip(p as usize * 12)?;
        let uv_at = r.pos() as u32;
        r.skip(p as usize * 8)?;
        let normal_at = r.pos() as u32;
        r.skip(p as usize * 12)?;
        let color_at = r.pos() as u32;
        r.skip(p as usize * 4)?;
        r.skip(1)?;
        let skin_at = if bones.is_empty() { 0 } else { r.pos() as u32 };
        if !bones.is_empty() {
            r.skip(4 + bones.len().min(4) * p as usize)?;
        }
        let indices_at = r.pos() as u32;
        r.skip(first as usize)?;
        let breaks_at = r.pos() as u32;
        r.skip(break_words(first as usize))?;
        groups.push(Group::Rigid {
            lists,
            positions: p,
            pos_at,
            uv_at,
            normal_at,
            color_at,
            indices_at,
            breaks_at,
            skin_at,
        });
    }
    Ok((bones, groups, flags, map))
}

fn read_indexed(r: &mut Reader<'_>) -> Result<BodyParts> {
    let bones = bone_table(r)?;
    let n = r.i16()?;
    let mut groups = Vec::new();
    for _ in 0..n.max(0) {
        let shader = r.u16()?;
        let count = r.u8()?;
        let positions = r.u8()?;
        let pos_at = r.pos() as u32;
        r.skip(positions as usize * 12)?;
        let skin_at = if bones.is_empty() { 0 } else { r.pos() as u32 };
        if !bones.is_empty() {
            r.skip(4 + bones.len().min(4) * positions as usize)?;
        }
        let indices_at = r.pos() as u32;
        r.skip(count as usize)?;
        let breaks_at = r.pos() as u32;
        r.skip(break_words(count as usize))?;
        groups.push(Group::Indexed { shader, count, positions, pos_at, indices_at, breaks_at, skin_at });
    }
    Ok((bones, groups, 0, VertexMap::default()))
}

fn read_outline(r: &mut Reader<'_>, streamed: bool) -> Result<BodyParts> {
    let bones = bone_table(r)?;
    let n = r.i16()?;
    let mut groups = Vec::new();
    for _ in 0..n.max(0) {
        let shader = if streamed { Some(r.u16()?) } else { None };
        let vertices = r.u8()?;
        let pos_at = r.pos() as u32;
        r.skip(vertices as usize * 12)?;
        let skin_at = if bones.is_empty() { 0 } else { r.pos() as u32 };
        if !bones.is_empty() {
            let k = r.u8()? as usize;
            r.skip(k + k * vertices as usize)?;
        }
        let edges = r.u8()?;
        let edges_at = r.pos() as u32;
        r.skip(edges as usize * 4)?;
        groups.push(Group::Outline { shader, vertices, pos_at, edges, edges_at, skin_at });
    }
    Ok((bones, groups, 0, VertexMap::default()))
}

fn read_mesh(r: &mut Reader<'_>, no_bounds: bool, conditions: Vec<i16>) -> Result<Mesh> {
    let offset = r.pos() as u32;
    let body_type = r.i8()?;
    let copies = r.u16()?;
    let transform = r.i16()?;
    let pass = r.i8()?;
    let max_distance = r.f32()?;
    r.skip(4 + 4 + 1 + 1)?;
    let sphere = if no_bounds {
        None
    } else {
        let c = r.vec3()?;
        let radius = r.f32()?;
        r.skip(8)?;
        Some((c, radius))
    };
    let (bones, groups, body_flags, vertex_map_table) = match body_type {
        RIGID => read_rigid(r)?,
        SKINNED => read_outline(r, false)?,
        STREAMED => read_outline(r, true)?,
        INDEXED => read_indexed(r)?,
        _ => return Err(Error::Corrupt { offset: offset as usize, what: "unknown mesh body type" }),
    };
    let mut list = Vec::with_capacity(copies as usize);
    for _ in 0..copies {
        r.skip(1)?;
        if !no_bounds {
            r.skip(4)?;
        }
        let transform = r.i16()?;
        let matrix = read_matrix(r)?;
        r.skip(4 + 1)?;
        let vertex_map_at = r.pos() as u32;
        vertex_map(r)?;
        list.push(MeshCopy { transform, matrix, vertex_map_at });
    }
    Ok(Mesh {
        conditions,
        body_type,
        transform,
        pass,
        max_distance,
        sphere,
        bones,
        groups,
        copies: list,
        offset,
        body_flags,
        vertex_map: vertex_map_table,
    })
}

impl MeshSet {
    pub fn read(r: &mut Reader<'_>) -> Result<MeshSet> {
        let no_bounds = r.i8()? != 0;
        r.skip(16)?;
        let n = r.u8()? as usize;
        r.skip_n(n, 34)?;
        let n = r.u8()? as usize;
        r.skip_n(n, 4)?;
        let slots = r.i16()?;
        let mut total = slots as i32;
        let mut used = 0i32;
        let mut meshes = Vec::new();
        while used < total {
            let c = r.i8()?;
            if c > 0 {
                let conditions = (0..c).map(|_| r.i16()).collect::<Result<Vec<_>>>()?;
                let size = r.i32()?;
                let start = r.pos();
                let mesh = read_mesh(r, no_bounds, conditions)?;
                if size < 0 || r.pos() - start != size as usize {
                    return Err(Error::Corrupt { offset: start, what: "conditional mesh size mismatch" });
                }
                meshes.push(mesh);
                total -= 1;
            } else {
                let mesh = read_mesh(r, no_bounds, Vec::new())?;
                used += 1 + mesh.copies.len() as i32;
                meshes.push(mesh);
            }
        }
        let tree_nodes = r.i16()?;
        for _ in 0..tree_nodes.max(0) {
            for _ in 0..4 {
                let kind = r.i8()?;
                if kind == -1 {
                    break;
                }
                r.skip(12 + 4 + 4 + 1)?;
                if kind == 0 || kind == 1 {
                    r.skip(2)?;
                }
            }
        }
        Ok(MeshSet { no_bounds, slots, meshes, tree_nodes })
    }
}

/// Decodes a strip (geometry.md 8): vertex `i` (from 2) closes `(i−2, i−1, i)` unless its break bit is set, with
/// the winding alternating with `i`; triangles repeating a position are dropped. `bit(i)` reads the break bit of
/// strip vertex `i`. Returns triangles of position indices.
/// The triangles of strip list `count` entries long at `start` in `data`, keeping only those whose indices are below
/// `positions` (the group's vertex count); empty if the list lies outside `data`.
pub fn strip_triangles_at(
    data: &[u8],
    start: usize,
    count: usize,
    positions: usize,
    bit: impl Fn(usize) -> bool,
) -> Vec<[u8; 3]> {
    let Some(idx) = start.checked_add(count).and_then(|end| data.get(start..end)) else { return Vec::new() };
    let mut t = strip_triangles(idx, bit);
    t.retain(|tri| tri.iter().all(|&i| (i as usize) < positions));
    t
}

/// Most entries in one strip list.
pub const MAX_STRIP_ENTRIES: u32 = 0x10000;

pub fn strip_triangles(indices: &[u8], bit: impl Fn(usize) -> bool) -> Vec<[u8; 3]> {
    let mut out = Vec::new();
    for i in 2..indices.len() {
        if bit(i) {
            continue;
        }
        let (a, b, c) = (indices[i - 2], indices[i - 1], indices[i]);
        if a == b || b == c || a == c {
            continue;
        }
        out.push(if i % 2 == 0 { [a, b, c] } else { [b, a, c] });
    }
    out
}

fn break_bit(data: &[u8], breaks_at: usize, i: usize) -> bool {
    let w = breaks_at + (i / 32) * 4;
    data.get(w..w + 4).is_some_and(|b| u32::from_le_bytes(b.try_into().unwrap()) & (1 << (i % 32)) != 0)
}

impl Group {
    /// Triangles of every strip list of the group: (shader index, triangles of position indices).
    pub fn triangles(&self, data: &[u8]) -> Vec<(i32, Vec<[u8; 3]>)> {
        match self {
            Group::Rigid { lists, indices_at, breaks_at, positions, .. } => lists
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

    /// Vertices for the GPU: position, normal, UV and colour (rigid); positions only otherwise.
    /// The group's bone weights (geometry.md 5), `None` for a mesh without a bone table. `bones` is the mesh's
    /// bone-table length: rigid and indexed groups have four slot bytes and min(bones, 4) weights per vertex.
    pub fn weights(&self, data: &[u8], bones: usize) -> Option<SkinWeights> {
        let (Group::Rigid { skin_at, .. } | Group::Indexed { skin_at, .. } | Group::Outline { skin_at, .. }) = *self;
        if skin_at == 0 || bones == 0 {
            return None;
        }
        let (at, slot_bytes, per_vertex, vertices) = match *self {
            Group::Rigid { skin_at, positions, .. } | Group::Indexed { skin_at, positions, .. } => {
                (skin_at as usize, 4, bones.min(4), positions as usize)
            }
            Group::Outline { skin_at, vertices, .. } => {
                let k = *data.get(skin_at as usize)? as usize;
                (skin_at as usize + 1, k, k, vertices as usize)
            }
        };
        let slots = data.get(at..at + slot_bytes)?;
        let w = data.get(at + slot_bytes..at + slot_bytes + per_vertex * vertices)?;
        Some(SkinWeights { slots: slots[..per_vertex.min(slot_bytes)].to_vec(), weights: w.to_vec() })
    }

    pub fn vertices(&self, data: &[u8]) -> Vec<GpuVertex> {
        let f = |o: usize| le_f32(data, o).unwrap_or(0.0);
        let v3 = |o: usize| [f(o), f(o + 4), f(o + 8)];
        match self {
            Group::Rigid { positions, pos_at, uv_at, normal_at, color_at, .. } => (0..*positions as usize)
                .map(|i| GpuVertex {
                    position: v3(*pos_at as usize + i * 12),
                    normal: v3(*normal_at as usize + i * 12),
                    uv: [f(*uv_at as usize + i * 8), f(*uv_at as usize + i * 8 + 4)],
                    color: data[*color_at as usize + i * 4..*color_at as usize + i * 4 + 4].try_into().unwrap(),
                })
                .collect(),
            Group::Indexed { positions, pos_at, .. } => (0..*positions as usize)
                .map(|i| GpuVertex {
                    position: v3(*pos_at as usize + i * 12),
                    normal: [0.0; 3],
                    uv: [0.0; 2],
                    color: [0x80; 4],
                })
                .collect(),
            Group::Outline { vertices, pos_at, .. } => (0..*vertices as usize)
                .map(|i| GpuVertex {
                    position: v3(*pos_at as usize + i * 12),
                    normal: [0.0; 3],
                    uv: [0.0; 2],
                    color: [0x80; 4],
                })
                .collect(),
        }
    }
}

impl Mesh {
    /// Triangles of all groups (each mesh once, copies not multiplied).
    pub fn triangle_count(&self, data: &[u8]) -> usize {
        self.groups.iter().flat_map(|g| g.triangles(data)).map(|(_, t)| t.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips() {
        // geometry.md 10: indices 0, 1, 2, 3 with breaks on 0 and 1 give (0, 1, 2) and (2, 1, 3).
        let t = strip_triangles(&[0, 1, 2, 3], |i| i < 2);
        assert_eq!(t, [[0, 1, 2], [2, 1, 3]]);
        // A break on 3 starts a new strip; a repeated index is degenerate.
        assert_eq!(strip_triangles(&[0, 1, 2, 3, 4, 4, 5], |i| i < 2 || i == 3), [[0, 1, 2], [2, 3, 4]]);
    }

    #[test]
    fn break_bits() {
        let data = [0b0000_0011u8, 0, 0, 0, 0x01, 0, 0, 0];
        assert!(break_bit(&data, 0, 0) && break_bit(&data, 0, 1) && !break_bit(&data, 0, 2));
        assert!(break_bit(&data, 0, 32) && !break_bit(&data, 0, 33));
    }

    #[test]
    fn strip_indices_past_the_positions_are_dropped() {
        // Positions 0..3: the strip (0, 1, 2, 7) gives (0, 1, 2) and drops (1, 2, 7).
        let data = [0u8, 1, 2, 7];
        assert_eq!(strip_triangles_at(&data, 0, 4, 3, |_| false), [[0, 1, 2]]);
        // A list outside the data, or one whose end overflows: no triangles, no panic.
        assert!(strip_triangles_at(&data, 2, 4, 255, |_| false).is_empty());
        assert!(strip_triangles_at(&data, usize::MAX, 4, 255, |_| false).is_empty());
    }

    #[test]
    fn huge_strip_lists_are_refused() {
        // A rigid body: flags 0, 2 bytes, no bones, no fog block, empty vertex map, one group with one strip list of
        // 2^31 - 1 entries.
        let mut b = alloc::vec![0u8, 0, 0, 0, 0xFF, 0, 0, 1, 0, 1];
        b.extend_from_slice(&i32::MAX.to_le_bytes());
        b.extend_from_slice(&[0; 8]);
        assert!(read_rigid(&mut Reader::new(&b)).is_err());
    }
}
