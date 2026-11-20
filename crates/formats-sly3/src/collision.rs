//! Collision parts of solid bodies (classes/solid-object.md 2). The collision mesh and tree are Sly 2's
//! (../sly2/classes/solid-object.md 2.1 and 2.2) and are read by `formats-sly2`'s readers into the shared
//! `formats::collision` types ([`Mesh`] = `formats::collision::CollisionMesh`, `WideCollisionTree`), so the engine builds
//! Sly 3 collision as it builds Sly 1 and Sly 2 collision. See `formats_sly2::collision` for the two conversions (the
//! computed mesh radius, the tree's leaf nodes dropped). A Sly 3 part carries two more `s8` than Sly 2's.
//!
//! **Tree children.** On the Sly 3 disc a missing child is stored as `0xFFFFFFFF` (as well as by a leaf node of
//! surface `0xFFFF`), so the conversion here ([`convert_tree`]) treats both as `NO_CHILD_WIDE`. Every other child
//! comes after its parent and has one parent, as in Sly 1. Trees convert to the 32-bit
//! `formats::collision::WideCollisionTree`, since three level objects' trees (75,558 to 103,211 stored nodes) have
//! more inner nodes than 16-bit node indices can name.

use alloc::vec::Vec;

use formats::collision::{NO_CHILD_WIDE, WideCollisionTree, WideTreeNode};
use formats::error::{Error, Result};
use formats::reader::Reader;
pub use formats_sly2::collision::{
    CollisionMesh as CollisionCounts, Mesh, read_mesh, read_mesh_and_tree, read_raw_tree,
};

/// One collision part of a solid body.
#[derive(Clone, Debug, PartialEq)]
pub struct CollisionPart {
    /// Shape ID (−1 on level geometry).
    pub shape: i16,
    /// The `s32` after the shape ID (meaning open).
    pub mask: u32,
    /// The two `s8` after it.
    pub flags: [i8; 2],
    /// The `f32` (meaning open).
    pub size: f32,
    /// The `s8` new in Sly 3.
    pub extra: i8,
    /// The mesh, in the solid's space.
    pub mesh: Mesh,
    /// The tree, converted.
    pub tree: WideCollisionTree,
    /// The stored tree: (node count, the second and third count fields).
    pub raw_tree: (u32, u32, u32),
    /// The trailing fields: `u32`, `u16`, `s8` (new in Sly 3), `s16`, `s8`.
    pub tail: (u32, u16, i8, i16, i8),
}

/// The surfaces of a collision mesh as triangles that cover them (spec question: classes/solid-object.md 2.1): the
/// shared `formats::collision::CollisionMesh::with_polygons` (Sly 3's mesh layout is Sly 1's and Sly 2's). A stored
/// surface is a convex polygon in the plane of its three corners: the convex hull of its corners and the ends of
/// every edge naming it, its own and its neighbours'. The v_apt level object's floor (surface 281) is a polygon at
/// z −33 whose corners (173, 177, 172) cover part of it. Some surfaces cannot be rebuilt this way: v_apt's
/// arrival floor (401) names one edge, and even the positions in its plane inside its bounding sphere do not
/// reach the arrival point (1,586, −5,000), so only the collision tree knows its extent. The stored surfaces stay first, at their indices (the tree names surfaces by
/// index); the extra triangles come after them with no edges.
pub fn with_polygons(mesh: &Mesh) -> Mesh {
    mesh.with_polygons()
}

/// No child, as stored.
pub const NO_CHILD_STORED: u32 = u32::MAX;

/// One stored tree node: surface, then the negative and positive children (`None` for a leaf).
pub type RawNode = (u16, Option<(u32, u32)>);

/// Converts stored tree nodes to a [`WideCollisionTree`] over a mesh of `surfaces` surfaces: leaves dropped, a leaf
/// child or a stored `0xFFFFFFFF` becomes `NO_CHILD_WIDE`, inner nodes keep their order. `None` when a surface or
/// child is out of range, a child is not after its parent, or a node has two parents.
pub fn convert_tree(nodes: &[RawNode], surfaces: usize) -> Option<WideCollisionTree> {
    let mut index = alloc::vec![NO_CHILD_WIDE; nodes.len()];
    let mut k = 0u32;
    for (i, n) in nodes.iter().enumerate() {
        if n.1.is_some() {
            index[i] = k;
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
        let mut child = |c: u32| -> Option<u32> {
            if c == NO_CHILD_STORED {
                return Some(NO_CHILD_WIDE);
            }
            let c = c as usize;
            if c >= nodes.len() || c <= i || core::mem::replace(&mut has_parent[c], true) {
                return None;
            }
            Some(index[c])
        };
        let (negative, positive) = (child(neg)?, child(pos)?);
        out.push(WideTreeNode { triangle: s as u32, negative, positive });
    }
    Some(WideCollisionTree { nodes: out })
}

/// Reads one collision part: `s16` shape, `s32`, 2 × `s8`, `f32`, `s8`, mesh and tree, `u32`, `u16`, `s8`, `s16`,
/// `s8`. A tree that does not convert (see [`convert_tree`]) is an error.
pub fn read_part(r: &mut Reader<'_>) -> Result<CollisionPart> {
    let shape = r.i16()?;
    let mask = r.u32()?;
    let flags = [r.i8()?, r.i8()?];
    let size = r.f32()?;
    let extra = r.i8()?;
    let mesh = read_mesh(r)?;
    let at = r.pos();
    let (nodes, raw_tree) = read_raw_tree(r)?;
    let tree = convert_tree(&nodes, mesh.triangles.len())
        .ok_or(Error::Corrupt { offset: at, what: "collision tree that does not convert" })?;
    let tail = (r.u32()?, r.u16()?, r.i8()?, r.i16()?, r.i8()?);
    Ok(CollisionPart { shape, mask, flags, size, extra, mesh, tree, raw_tree, tail })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(tree: &[u8], tree_words: [u32; 3]) -> alloc::vec::Vec<u8> {
        let mut d = alloc::vec![];
        d.extend_from_slice(&(-1i16).to_le_bytes());
        d.extend_from_slice(&[0u8; 11]);
        d.extend_from_slice(&3u16.to_le_bytes());
        for p in [[0.0f32, 0.0, 0.0], [3.0, 0.0, 0.0], [0.0, 4.0, 0.0]] {
            for x in p {
                d.extend_from_slice(&x.to_le_bytes());
            }
        }
        d.extend_from_slice(&1u16.to_le_bytes());
        d.extend_from_slice(&0u16.to_le_bytes());
        for c in [0u16, 1, 2] {
            d.extend_from_slice(&c.to_le_bytes());
        }
        for x in [2.5f32, 1.0, 1.0, 0.0] {
            d.extend_from_slice(&x.to_le_bytes());
        }
        d.push(0);
        for w in tree_words {
            d.extend_from_slice(&w.to_le_bytes());
        }
        d.extend_from_slice(tree);
        d.extend_from_slice(&[0u8; 10]);
        d
    }

    #[test]
    fn a_surface_is_the_hull_of_its_corners_and_edge_ends() {
        use formats::collision::{CollisionEdge, CollisionTriangle};
        // A 10 × 10 square at z 0: corners 0, 1 and 4 (the centre, inside), edges reaching 2 and 3.
        let positions =
            alloc::vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0], [0.0, 10.0, 0.0], [5.0, 5.0, 0.0]];
        let mesh = Mesh {
            radius: 15.0,
            positions,
            triangles: alloc::vec![CollisionTriangle {
                corners: [0, 1, 4],
                radius: 8.0,
                center: [5.0, 5.0, 0.0],
                first_edge: 0,
                edge_count: 2
            }],
            edges: alloc::vec![CollisionEdge { ends: [1, 2], other: 0 }, CollisionEdge { ends: [2, 3], other: 0 }],
        };
        let full = with_polygons(&mesh);
        assert_eq!(full.triangles[0], mesh.triangles[0]);
        // The hull is the square (the centre corner inside it is dropped): two triangles cover it.
        let added = &full.triangles[1..];
        assert_eq!(added.len(), 2);
        assert!(added.iter().all(|t| !t.corners.contains(&4)));
        let area: f32 = added
            .iter()
            .map(|t| {
                let [a, b, c] = t.corners.map(|v| full.positions[v as usize]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0
            })
            .sum();
        assert!((area - 100.0).abs() < 1e-3, "{area}");
    }

    #[test]
    fn stored_minus_one_is_no_child() {
        let t =
            convert_tree(&[(0, Some((1, NO_CHILD_STORED))), (0, Some((NO_CHILD_STORED, 2))), (0, None)], 1).unwrap();
        assert_eq!(
            t.nodes,
            [
                WideTreeNode { triangle: 0, negative: 1, positive: NO_CHILD_WIDE },
                WideTreeNode { triangle: 0, negative: NO_CHILD_WIDE, positive: NO_CHILD_WIDE }
            ]
        );
        assert_eq!(convert_tree(&[(0, Some((0, 1)))], 1), None, "a child before its parent");
    }

    #[test]
    fn part_with_a_tree_of_leaves() {
        // Node 0: surface 0, children leaf 1 and leaf 2.
        let mut t = alloc::vec![];
        t.extend_from_slice(&0u16.to_le_bytes());
        t.extend_from_slice(&1u32.to_le_bytes());
        t.extend_from_slice(&2u32.to_le_bytes());
        t.extend_from_slice(&0xFFFFu16.to_le_bytes());
        t.extend_from_slice(&0xFFFFu16.to_le_bytes());
        let d = part(&t, [3, 3, 3]);
        let mut r = Reader::new(&d);
        let p = read_part(&mut r).unwrap();
        assert_eq!(r.pos(), d.len());
        assert_eq!((p.shape, p.mesh.positions.len(), p.mesh.triangles.len()), (-1, 3, 1));
        assert!((p.mesh.radius - 4.0).abs() < 1e-4);
        assert_eq!(p.tree.nodes, [WideTreeNode { triangle: 0, negative: NO_CHILD_WIDE, positive: NO_CHILD_WIDE }]);
        assert_eq!(p.raw_tree, (3, 3, 3));
    }
}
