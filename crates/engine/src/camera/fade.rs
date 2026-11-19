//! Camera-fade groups (collision.md 4): meshes of a solid that fade instead of blocking the view. Each frame the
//! segment from the camera to the focus is pushed through the solid's collision tree; every node it reaches that
//! has a group fades that group's meshes for the next 0.1 s.

use alloc::vec::Vec;

use formats::collision::{FadeEntry, Solid};
use formats::geometry::Affine;
use formats::stream::{Layout, ObjectStream, layout_of};

use crate::collision::TriMesh;
use crate::math::{Vec3, affine_inverse};

/// A solid with camera-fade groups.
#[derive(Clone, Debug, PartialEq)]
pub struct FadeSource {
    /// Its object-stream record.
    pub record: u32,
    /// Its collision mesh and tree.
    pub mesh: TriMesh,
    /// The group list.
    pub groups: Vec<FadeEntry>,
    /// Per tree node: the first group entry, or −1.
    pub node_map: Vec<i16>,
}

impl FadeSource {
    /// From a parsed solid; `None` when it has no fade groups or no node map.
    pub fn new(record: u32, solid: &Solid) -> Option<FadeSource> {
        if solid.fade_groups.is_empty() || solid.fade_node_map.is_empty() {
            return None;
        }
        Some(FadeSource {
            record,
            mesh: TriMesh::new(&solid.mesh, &solid.tree),
            groups: solid.fade_groups.clone(),
            node_map: solid.fade_node_map.clone(),
        })
    }

    /// The meshes (indices into the record's mesh set) the segment `a b` (world space) fades, given the solid's
    /// world transform. Appends to `out` without duplicates.
    pub fn faded_meshes(&self, transform: &Affine, a: Vec3, b: Vec3, out: &mut Vec<u16>) {
        let Some(inv) = affine_inverse(transform) else { return };
        let (la, lb) = (inv.transform_point(a), inv.transform_point(b));
        self.mesh.tree_segment(la, lb, 1e-3, |node| {
            let Some(&first) = self.node_map.get(node as usize) else { return };
            if first < 0 {
                return;
            }
            // The group: its first entry, then the following entries marked −1.
            let mut i = first as usize;
            while let Some(e) = self.groups.get(i) {
                if i != first as usize && e.key != -1 {
                    break;
                }
                if !out.contains(&e.mesh) {
                    out.push(e.mesh);
                }
                i += 1;
            }
        });
    }
}

/// Every in-world solid with camera-fade groups.
pub fn fade_sources(data: &[u8], stream: &ObjectStream) -> formats::Result<Vec<FadeSource>> {
    let mut out = Vec::new();
    for (i, r) in stream.records.iter().enumerate() {
        if !r.in_world || r.template || !matches!(layout_of(r.class), Some(Layout::Solid | Layout::Conveyor)) {
            continue;
        }
        let solid = Solid::parse(data, r.offset + 6)?;
        if let Some(f) = FadeSource::new(i as u32, &solid) {
            out.push(f);
        }
    }
    Ok(out)
}

/// A mesh faded this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FadedMesh {
    /// The record whose mesh set holds it.
    pub record: u32,
    /// Mesh index in that mesh set.
    pub mesh: u16,
}

/// Keeps each faded mesh faded for the fade time after the last frame it blocked the view.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FadeTracker {
    active: Vec<(FadedMesh, f32)>,
}

impl FadeTracker {
    /// Marks `m` as blocking now.
    pub fn touch(&mut self, m: FadedMesh, time: f32) {
        match self.active.iter_mut().find(|(a, _)| *a == m) {
            Some(e) => e.1 = time,
            None => self.active.push((m, time)),
        }
    }

    /// Ages the set by `dt` and returns the meshes still faded.
    pub fn step(&mut self, dt: f32) -> Vec<FadedMesh> {
        for e in &mut self.active {
            e.1 -= dt;
        }
        self.active.retain(|e| e.1 > 0.0);
        self.active.iter().map(|e| e.0).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use formats::collision::{CollisionMesh, CollisionTree, CollisionTriangle, NO_CHILD, TreeNode};

    #[test]
    fn segment_through_a_node_fades_its_group() {
        // One wall at x = 0 (facing −x) and one at x = 100 (facing −x), a tree node per wall.
        let positions = vec![
            [0.0, -50.0, -50.0],
            [0.0, 50.0, -50.0],
            [0.0, 0.0, 50.0],
            [100.0, -50.0, -50.0],
            [100.0, 50.0, -50.0],
            [100.0, 0.0, 50.0],
        ];
        let tri = |c| CollisionTriangle { corners: c, radius: 100.0, center: [0.0; 3], first_edge: 0, edge_count: 0 };
        let mesh =
            CollisionMesh { radius: 200.0, positions, triangles: vec![tri([0, 2, 1]), tri([3, 5, 4])], edges: vec![] };
        let tree = CollisionTree {
            nodes: vec![
                TreeNode { triangle: 0, negative: 1, positive: NO_CHILD },
                TreeNode { triangle: 1, negative: NO_CHILD, positive: NO_CHILD },
            ],
            full_count: 2,
        };
        let solid = Solid {
            mesh,
            tree,
            fade_groups: vec![
                FadeEntry { key: 1, mesh: 3 },
                FadeEntry { key: -1, mesh: 4 },
                FadeEntry { key: 1, mesh: 7 },
            ],
            fade_node_map: vec![-1, 0],
            ..Default::default()
        };
        let f = FadeSource::new(5, &solid).unwrap();
        let mut out = Vec::new();
        // From x = −50 to x = 150 the segment reaches node 0 (no group) and node 1 (group 0: meshes 3 and 4).
        f.faded_meshes(&Affine::IDENTITY, [-50.0, 0.0, 0.0], [150.0, 0.0, 0.0], &mut out);
        assert_eq!(out, [3, 4]);
        // In front of the first wall only: node 1 is not reached.
        out.clear();
        f.faded_meshes(&Affine::IDENTITY, [-50.0, 0.0, 0.0], [-10.0, 0.0, 0.0], &mut out);
        assert!(out.is_empty());
        let mut t = FadeTracker::default();
        t.touch(FadedMesh { record: 5, mesh: 3 }, 0.1);
        assert_eq!(t.step(0.05).len(), 1);
        assert!(t.step(0.06).is_empty());
    }
}
