//! Collision meshes and trees (classes/solid-object.md 2).
//!
//! A solid's collision parts are kept whole: the mesh as a [`formats::collision::CollisionMesh`] and the tree as a
//! [`formats::collision::CollisionTree`], so `engine::collision` builds Sly 2 collision unchanged
//! (`engine::collision::TriMesh::new`). Two things differ from Sly 1:
//!
//! - **No bounding radius** is stored with a Sly 2 mesh (2.1); [`CollisionPart::mesh`] gets the largest distance of
//!   its positions from the origin of its space.
//! - **The tree** (2.2) stores its children as `u32` node numbers, `0xFFFF_FFFF` for none, and may store a leaf as a
//!   node of its own (surface `0xFFFF`, no children). The conversion drops leaf nodes: a missing child or a leaf
//!   becomes `NO_CHILD`, and the other nodes keep their order. A tree that does not convert (a child out of range, before its parent, shared, or a surface
//!   out of range) is left empty and counted in [`CollisionPart::raw_tree`].
//!
//! Collision meshes of zones, volumes, warps and tracks are read the same way but only stepped over.

use alloc::vec::Vec;

use formats::collision::{CollisionEdge, CollisionTree, CollisionTriangle, NO_CHILD, TreeNode};

/// A collision mesh in `formats::collision` form.
pub type Mesh = formats::collision::CollisionMesh;
use formats::error::{Error, Result};
use formats::reader::Reader;

/// The polygon a stored surface covers (classes/solid-object.md 2.1): the convex hull, in the plane of its three
/// corners, of the corners and every end of the edges that name it, as vertex indices in order around it. The edges
/// naming a surface are those stored under it and those stored under a neighbouring surface with it as the other
/// side; together they name only the convex sides it shares with others (concave sides and open borders are not
/// stored, and 2,584 Sly 2 surfaces have no edges at all), and one to three corners can lie inside the polygon, so
/// the hull is taken over all of them. The layout is Sly 1's, so this is
/// [`formats::collision::CollisionMesh::surface_polygon`]. Test vector: the cellar's solid 136751, surface 0
/// (corners 13, 9, 7), is a seven-sided polygon.
pub fn surface_polygon(mesh: &Mesh, t: usize) -> Vec<u16> {
    mesh.surface_polygon(t, &mesh.edge_ends())
}

/// The surfaces of a collision mesh as triangles that cover them: each stored surface's polygon
/// ([`surface_polygon`]) as a fan. The result keeps every stored surface first, at its index (the tree names surfaces
/// by index), and appends the extra fan triangles after them with no edges; a fan triangle over the three stored
/// corners is not added again. This is [`formats::collision::CollisionMesh::with_polygons`] (same layout as Sly 1).
pub fn with_polygons(mesh: &Mesh) -> Mesh {
    mesh.with_polygons()
}

/// One collision part of a solid body (2).
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionPart {
    /// Row 1: the transform reference the part hangs on (−1 none; classes/solid-object.md 2). Formerly read as a
    /// shape ID; see [`CollisionPart::transform`].
    pub shape: i16,
    /// Row 2: the part's tag, by which clips (events 15 and 16) and scripts switch parts on and off; see
    /// [`CollisionPart::tag`].
    pub mask: u32,
    /// Row 3: two flags.
    pub flags: [i8; 2],
    /// Row 4: the part's radius; see [`CollisionPart::radius`].
    pub size: f32,
    /// The mesh, in the solid's space.
    pub mesh: Mesh,
    /// The tree, converted (empty when it did not convert).
    pub tree: CollisionTree,
    /// The stored tree: (node count, the second and third count fields).
    pub raw_tree: (u32, u32, u32),
    /// Row 7: the contact kind (`u32`), the contact value (`s16`), a second `s16` and a flag (`s8`); see
    /// [`CollisionPart::contact_kind`] and the methods after it.
    pub tail: (u32, i16, i16, i8),
}

/// The field meanings of classes/solid-object.md 2 ("Field meanings found 2026-11-18", from the reference).
impl CollisionPart {
    /// The transform reference the part hangs on (row 1; −1 none).
    pub fn transform(&self) -> i16 {
        self.shape
    }

    /// The part's tag (row 2): clips and scripts switch parts on and off by it.
    pub fn tag(&self) -> u32 {
        self.mask
    }

    /// The part's radius (row 4).
    pub fn radius(&self) -> f32 {
        self.size
    }

    /// The contact kind (row 7): a hit-kind bit set sent with contacts (guards.md 6.3).
    pub fn contact_kind(&self) -> u32 {
        self.tail.0
    }

    /// The contact value (row 7): the hit amount, −10 by default.
    pub fn contact_value(&self) -> i16 {
        self.tail.1
    }

    /// The second `s16` of row 7 (equal to the mesh's position count on the parts surveyed).
    pub fn contact_second(&self) -> i16 {
        self.tail.2
    }

    /// The flag of row 7.
    pub fn contact_flag(&self) -> i8 {
        self.tail.3
    }
}

/// Reads a collision mesh (2.1): positions, surfaces with their edges.
pub fn read_mesh(r: &mut Reader<'_>) -> Result<Mesh> {
    let np = r.u16()? as usize;
    let mut positions = Vec::with_capacity(np.min(r.remaining() / 12));
    for _ in 0..np {
        positions.push(r.vec3()?);
    }
    let nt = r.u16()? as usize;
    let edge_total = r.u16()? as usize;
    let mut triangles = Vec::with_capacity(nt.min(r.remaining() / 23));
    let mut edges = Vec::with_capacity(edge_total.min(r.remaining() / 6));
    for _ in 0..nt {
        let at = r.pos();
        let corners = [r.u16()?, r.u16()?, r.u16()?];
        if corners.iter().any(|&c| c as usize >= np) {
            return Err(Error::Corrupt { offset: at, what: "collision surface corner out of range" });
        }
        let radius = r.f32()?;
        let center = r.vec3()?;
        let n = r.u8()?;
        let first_edge = edges.len() as u32;
        for _ in 0..n {
            let at = r.pos();
            let e = CollisionEdge { ends: [r.u16()?, r.u16()?], other: r.u16()? };
            if e.ends.iter().any(|&p| p as usize >= np) || e.other as usize >= nt {
                return Err(Error::Corrupt { offset: at, what: "collision edge index out of range" });
            }
            edges.push(e);
        }
        triangles.push(CollisionTriangle { corners, radius, center, first_edge, edge_count: n });
    }
    if edges.len() != edge_total {
        return Err(r.corrupt("collision edge total does not match"));
    }
    let radius = positions.iter().map(|p| p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).fold(0.0f32, f32::max);
    Ok(Mesh { radius: sqrt(radius), positions, triangles, edges })
}

/// "No child" in a stored tree.
pub const NO_CHILD_RAW: u32 = 0xFFFF_FFFF;

/// One stored tree node: surface, negative child, positive child (`None` for a leaf).
type RawNode = (u16, Option<(u32, u32)>);

/// Reads a collision tree (2.2): the stored nodes and the three count fields.
pub fn read_raw_tree(r: &mut Reader<'_>) -> Result<(Vec<RawNode>, (u32, u32, u32))> {
    let n = r.u32()?;
    let copy = r.u32()?;
    let count = r.u32()?;
    let mut nodes = Vec::with_capacity((n as usize).min(r.remaining() / 2));
    for _ in 0..n {
        let s = r.u16()?;
        nodes.push(if s == 0xFFFF { (s, None) } else { (s, Some((r.u32()?, r.u32()?))) });
    }
    Ok((nodes, (n, copy, count)))
}

/// Converts stored tree nodes to a [`CollisionTree`] over a mesh of `surfaces` surfaces (module documentation);
/// `None` when it does not convert.
pub fn convert_tree(nodes: &[RawNode], surfaces: usize) -> Option<CollisionTree> {
    let mut index = alloc::vec![NO_CHILD; nodes.len()];
    let mut k = 0u32;
    for (i, n) in nodes.iter().enumerate() {
        if n.1.is_some() {
            index[i] = u16::try_from(k).ok().filter(|&v| v != NO_CHILD)?;
            k += 1;
        }
    }
    let mut out = Vec::with_capacity(k as usize);
    let mut has_parent = alloc::vec![false; nodes.len()];
    for (i, &(s, children)) in nodes.iter().enumerate() {
        let Some((neg, pos)) = children else { continue };
        if s as usize >= surfaces {
            return None;
        }
        let mut child = |c: u32| -> Option<u16> {
            if c == NO_CHILD_RAW {
                return Some(NO_CHILD);
            }
            let c = c as usize;
            if c >= nodes.len() || c <= i || core::mem::replace(&mut has_parent[c], true) {
                return None;
            }
            Some(index[c])
        };
        let (negative, positive) = (child(neg)?, child(pos)?);
        out.push(TreeNode { triangle: s, negative, positive });
    }
    let full_count = u16::try_from(out.len()).ok()?;
    Some(CollisionTree { nodes: out, full_count })
}

/// Counts of a collision mesh and its tree (what zones, volumes, warps and tracks keep).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CollisionMesh {
    pub positions: u16,
    pub surfaces: u16,
    pub tree_nodes: u32,
}

/// Reads a collision mesh and its tree, keeping their counts (zones, volumes, warps, tracks).
pub fn read_mesh_and_tree(r: &mut Reader<'_>) -> Result<CollisionMesh> {
    let m = read_mesh(r)?;
    let (_, raw) = read_raw_tree(r)?;
    Ok(CollisionMesh { positions: m.positions.len() as u16, surfaces: m.triangles.len() as u16, tree_nodes: raw.0 })
}

/// Reads one collision part of a solid body (2).
pub fn read_part(r: &mut Reader<'_>) -> Result<CollisionPart> {
    let shape = r.i16()?;
    let mask = r.u32()?;
    let flags = [r.i8()?, r.i8()?];
    let size = r.f32()?;
    let mesh = read_mesh(r)?;
    let (nodes, raw_tree) = read_raw_tree(r)?;
    let tree = convert_tree(&nodes, mesh.triangles.len()).unwrap_or_default();
    let tail = (r.u32()?, r.i16()?, r.i16()?, r.i8()?);
    Ok(CollisionPart { shape, mask, flags, size, mesh, tree, raw_tree, tail })
}

/// `no_std` square root (Newton's method) for the bounding radius.
fn sqrt(v: f32) -> f32 {
    if v <= 0.0 || !v.is_finite() {
        return 0.0;
    }
    let mut x = f32::from_bits((v.to_bits() >> 1) + 0x1FC0_0000);
    for _ in 0..5 {
        x = 0.5 * (x + v / x);
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_meanings() {
        let p = CollisionPart {
            shape: 4,
            mask: 0x20,
            flags: [0, 1],
            size: 75.0,
            mesh: Mesh::default(),
            tree: CollisionTree::default(),
            raw_tree: (0, 0, 0),
            tail: (8, -10, 12, 1),
        };
        assert_eq!((p.transform(), p.tag(), p.radius()), (4, 0x20, 75.0));
        assert_eq!((p.contact_kind(), p.contact_value(), p.contact_second(), p.contact_flag()), (8, -10, 12, 1));
    }

    #[test]
    fn trees_drop_leaf_nodes() {
        // 0: surface 0, children 1 (leaf) and 2; 2: surface 1, children 3 (leaf) and none.
        let nodes = [(0, Some((1, 2))), (0xFFFF, None), (1, Some((3, NO_CHILD_RAW))), (0xFFFF, None)];
        let t = convert_tree(&nodes, 2).unwrap();
        assert_eq!(
            t.nodes,
            [
                TreeNode { triangle: 0, negative: NO_CHILD, positive: 1 },
                TreeNode { triangle: 1, negative: NO_CHILD, positive: NO_CHILD }
            ]
        );
        // A child before its parent, a shared child, a surface out of range.
        assert!(convert_tree(&[(0, Some((0, 1))), (0xFFFF, None)], 1).is_none());
        assert!(convert_tree(&[(0, Some((1, 1))), (0xFFFF, None)], 1).is_none());
        assert!(convert_tree(&[(5, Some((1, 2))), (0xFFFF, None), (0xFFFF, None)], 1).is_none());
        // An empty tree and a lone leaf.
        assert_eq!(convert_tree(&[], 0).unwrap().nodes, []);
        assert_eq!(convert_tree(&[(0xFFFF, None)], 0).unwrap().nodes, []);
    }

    #[test]
    fn meshes_get_a_bounding_radius() {
        let mut d = Vec::new();
        d.extend_from_slice(&3u16.to_le_bytes());
        for p in [[0.0f32, 0.0, 0.0], [3.0, 4.0, 0.0], [0.0, 0.0, 2.0]] {
            for v in p {
                d.extend_from_slice(&v.to_le_bytes());
            }
        }
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&1u16.to_le_bytes());
        for c in [0u16, 1, 2] {
            d.extend_from_slice(&c.to_le_bytes());
        }
        for v in [2.5f32, 1.0, 1.0, 1.0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        d.push(1);
        for v in [0u16, 1, 0] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        let m = read_mesh(&mut Reader::new(&d)).unwrap();
        assert!((m.radius - 5.0).abs() < 1e-5);
        assert_eq!(m.triangles[0].edge_count, 1);
        assert_eq!(m.edges[0], CollisionEdge { ends: [0, 1], other: 0 });
    }
}
