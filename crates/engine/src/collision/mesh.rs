//! The run-time form of one collision mesh: triangles with planes, a triangle hierarchy, the stored edges, ledges
//! and the collision tree (collision.md 1 and 2).

use alloc::vec::Vec;
use core::cell::Cell;

use formats::collision::{CollisionEdge, CollisionMesh, CollisionTree, NO_CHILD_WIDE, WideCollisionTree};

use super::bvh::Bvh;
use super::geom::{
    Aabb, Contact, Triangle, capsule_sweep_triangle, inverse_dir, ray_triangle, sphere_sweep_triangle,
    sphere_triangle_overlap,
};
use crate::math::{Vec3, add, cross, dot, max3, min3, normalize, scale, sub};

/// A surface counts as floor-like for ledges above this normal z (collision.md 1.1, 7).
pub const FLOOR_Z: f32 = 0.7;
/// A surface counts as wall-like for ledges below this normal z.
pub const WALL_Z: f32 = 0.55;
/// A contact counts as ground above this normal z (collision.md 7).
pub const GROUND_Z: f32 = 0.7;

/// A ray hit on a mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshHit {
    /// Distance along the ray, in units of the ray direction.
    pub t: f32,
    /// Triangle index.
    pub triangle: u32,
    /// Whether the front (open) side was hit.
    pub front: bool,
}

/// A sweep contact on a mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshContact {
    /// The contact.
    pub contact: Contact,
    /// Triangle index.
    pub triangle: u32,
}

/// A grabbable ledge: a stored edge between a floor-like and a wall-like surface (collision.md 1.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ledge {
    /// End points.
    pub ends: [Vec3; 2],
    /// Horizontal unit vector perpendicular to the edge, on the wall's side.
    pub outward: Vec3,
    /// The floor-like triangle.
    pub floor: u32,
    /// The wall-like triangle.
    pub wall: u32,
}

/// One collision mesh, ready for queries, in its owner's local space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TriMesh {
    /// Triangles, in stored order.
    pub triangles: Vec<Triangle>,
    /// Hierarchy over the triangles.
    pub bvh: Bvh,
    /// Bounds of every triangle.
    pub bounds: Aabb,
    /// The stored edges.
    pub edges: Vec<CollisionEdge>,
    /// Per triangle: (first edge, edge count) into `edges`.
    pub edge_ranges: Vec<(u32, u8)>,
    /// The stored positions.
    pub positions: Vec<Vec3>,
    /// The collision tree, with 32-bit indices (Sly 3 has trees past 65,535 nodes).
    pub tree: WideCollisionTree,
}

impl TriMesh {
    /// Builds the run-time mesh.
    pub fn new(mesh: &CollisionMesh, tree: &CollisionTree) -> TriMesh {
        TriMesh::new_wide(mesh, WideCollisionTree::from(tree))
    }

    /// Builds the run-time mesh with a tree of 32-bit indices.
    pub fn new_wide(mesh: &CollisionMesh, tree: WideCollisionTree) -> TriMesh {
        let triangles: Vec<Triangle> = (0..mesh.triangles.len())
            .map(|t| {
                let [a, b, c] = mesh.corners(t);
                Triangle::new(a, b, c)
            })
            .collect();
        let boxes: Vec<Aabb> = triangles.iter().map(Triangle::aabb).collect();
        let bounds = boxes.iter().fold(Aabb::EMPTY, |b, x| b.union(*x));
        TriMesh {
            bvh: Bvh::build(&boxes),
            triangles,
            bounds,
            edges: mesh.edges.clone(),
            edge_ranges: mesh.triangles.iter().map(|t| (t.first_edge, t.edge_count)).collect(),
            positions: mesh.positions.clone(),
            tree,
        }
    }

    /// Whether the mesh has no triangles.
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// The nearest hit of the ray `origin + t · dir`, `t` in `[0, t_max]`.
    pub fn ray_cast(&self, origin: Vec3, dir: Vec3, t_max: f32, two_sided: bool) -> Option<MeshHit> {
        let inv = inverse_dir(dir);
        let limit = Cell::new(t_max);
        let mut best: Option<MeshHit> = None;
        self.bvh.walk(
            |b| b.ray_enter(origin, inv, limit.get()).is_some(),
            |items| {
                for &i in items {
                    if let Some((t, front)) =
                        ray_triangle(origin, dir, &self.triangles[i as usize], limit.get(), two_sided)
                        && best.is_none_or(|b| t < b.t)
                    {
                        best = Some(MeshHit { t, triangle: i, front });
                        limit.set(t);
                    }
                }
                true
            },
        );
        best
    }

    /// Whether anything blocks the ray within `t_max` (stops at the first hit).
    pub fn ray_blocked(&self, origin: Vec3, dir: Vec3, t_max: f32, two_sided: bool) -> bool {
        let inv = inverse_dir(dir);
        let mut hit = false;
        self.bvh.walk(
            |b| b.ray_enter(origin, inv, t_max).is_some(),
            |items| {
                hit = items
                    .iter()
                    .any(|&i| ray_triangle(origin, dir, &self.triangles[i as usize], t_max, two_sided).is_some());
                !hit
            },
        );
        hit
    }

    /// Walks the triangles whose boxes the swept box `start .. start + delta` (grown by `r`) can touch, keeping the
    /// earliest contact `test` reports.
    fn sweep(&self, start: Aabb, delta: Vec3, test: impl Fn(&Triangle) -> Option<Contact>) -> Option<MeshContact> {
        let swept = Aabb { min: min3(start.min, add(start.min, delta)), max: max3(start.max, add(start.max, delta)) };
        // A ray from the start box's centre, against node boxes grown by the start box's half-size, culls along the
        // motion instead of over the whole swept box.
        let center = start.center();
        let half = start.half_extent();
        let inv = inverse_dir(delta);
        let limit = Cell::new(1.0f32);
        let mut best: Option<MeshContact> = None;
        self.bvh.walk(
            |b| {
                b.overlaps(&swept) && {
                    let g = Aabb { min: sub(b.min, half), max: add(b.max, half) };
                    g.ray_enter(center, inv, limit.get()).is_some()
                }
            },
            |items| {
                for &i in items {
                    if let Some(c) = test(&self.triangles[i as usize])
                        && best.is_none_or(|b| c.t < b.contact.t)
                    {
                        best = Some(MeshContact { contact: c, triangle: i });
                        limit.set(c.t);
                    }
                }
                true
            },
        );
        best
    }

    /// The first contact of a sphere moving by `delta`.
    pub fn sphere_sweep(&self, center: Vec3, r: f32, delta: Vec3, two_sided: bool) -> Option<MeshContact> {
        let start = Aabb { min: center, max: center }.grow(r);
        self.sweep(start, delta, |t| sphere_sweep_triangle(center, r, delta, t, two_sided))
    }

    /// The first contact of a capsule (segment `a b`, radius `r`) moving by `delta`.
    pub fn capsule_sweep(&self, a: Vec3, b: Vec3, r: f32, delta: Vec3, two_sided: bool) -> Option<MeshContact> {
        let start = Aabb { min: min3(a, b), max: max3(a, b) }.grow(r);
        self.sweep(start, delta, |t| capsule_sweep_triangle(a, b, r, delta, t, two_sided))
    }

    /// Whether `p` is inside the solid the collision tree describes (collision.md 2): from the root, go to the
    /// positive child when `p` is on the normal's side of the node's plane, else to the negative child; a missing
    /// positive child means open space, a missing negative child means inside. A mesh without a tree has no inside.
    pub fn point_inside(&self, p: Vec3) -> bool {
        let nodes = &self.tree.nodes;
        if nodes.is_empty() {
            return false;
        }
        let mut i = 0usize;
        for _ in 0..nodes.len() {
            let node = nodes[i];
            let tri = &self.triangles[node.triangle as usize];
            let front = tri.is_degenerate() || tri.signed_distance(p) >= 0.0;
            let next = if front { node.positive } else { node.negative };
            if next == NO_CHILD_WIDE {
                return !front;
            }
            i = next as usize;
        }
        false
    }

    /// The tree walk of [`TriMesh::point_inside`], also giving the last node's triangle (the plane that decided).
    pub fn point_inside_at(&self, p: Vec3) -> Option<(bool, u32)> {
        let nodes = &self.tree.nodes;
        if nodes.is_empty() {
            return None;
        }
        let mut i = 0usize;
        for _ in 0..nodes.len() {
            let node = nodes[i];
            let tri = &self.triangles[node.triangle as usize];
            let front = tri.is_degenerate() || tri.signed_distance(p) >= 0.0;
            let next = if front { node.positive } else { node.negative };
            if next == NO_CHILD_WIDE {
                return Some((!front, node.triangle));
            }
            i = next as usize;
        }
        None
    }

    /// The floor below `p` by the collision tree's one-sided rule (collision.md 2; Sly 2 classes/solid-object.md
    /// 2.2): walking down from `p` along `down` (unit, in the mesh's frame) for at most `max_drop`, the first point
    /// the tree calls solid, refined to `0.01`, and the triangle whose plane decided it. `None` when `p` is already
    /// solid or nothing solid is reached. It finds floors whose stored outline does not cover the point.
    pub fn tree_floor(&self, p: Vec3, down: Vec3, max_drop: f32) -> Option<(f32, u32)> {
        const STEP: f32 = 8.0;
        if self.point_inside_at(p)?.0 {
            return None;
        }
        let at = |d: f32| add(p, scale(down, d));
        let (mut empty, mut solid) = (0.0f32, None);
        let mut d = STEP.min(max_drop);
        loop {
            if self.point_inside_at(at(d)).is_some_and(|s| s.0) {
                solid = Some(d);
                break;
            }
            empty = d;
            if d >= max_drop {
                break;
            }
            d = (d + STEP).min(max_drop);
        }
        let mut hi = solid?;
        while hi - empty > 0.01 {
            let mid = 0.5 * (empty + hi);
            if self.point_inside_at(at(mid)).is_some_and(|s| s.0) {
                hi = mid;
            } else {
                empty = mid;
            }
        }
        let tri = self.point_inside_at(at(hi))?.1;
        Some((hi, tri))
    }

    /// The triangle nearest to `p`, with the closest point on it.
    pub fn nearest_triangle(&self, p: Vec3) -> Option<(u32, Vec3)> {
        let best = Cell::new(f32::INFINITY);
        let mut out = None;
        self.bvh.walk(
            |b| {
                let q =
                    [p[0].clamp(b.min[0], b.max[0]), p[1].clamp(b.min[1], b.max[1]), p[2].clamp(b.min[2], b.max[2])];
                crate::math::length_sq(sub(p, q)) < best.get()
            },
            |items| {
                for &i in items {
                    let tri = &self.triangles[i as usize];
                    if tri.is_degenerate() {
                        continue;
                    }
                    let q = super::geom::closest_point_on_triangle(p, tri);
                    let d = crate::math::length_sq(sub(p, q));
                    if d < best.get() {
                        best.set(d);
                        out = Some((i, q));
                    }
                }
                true
            },
        );
        out
    }

    /// Every surface a sphere is in contact with: `(triangle, normal, depth, point)` appended to `out`, the normal
    /// pointing the way out. There is no one-sided test (collision.md 7):
    /// - a centre inside the solid (the tree) is in contact with the nearest surface, whatever the distance, and is
    ///   pushed out along that surface's normal by `r` plus the distance to it;
    /// - otherwise every triangle within `r` counts, from either side, with the normal from the closest point to
    ///   the centre.
    pub fn sphere_overlaps(&self, center: Vec3, r: f32, out: &mut Vec<(u32, Vec3, f32, Vec3)>) {
        if self.point_inside(center) {
            if let Some((i, q)) = self.nearest_triangle(center) {
                let n = self.triangles[i as usize].normal;
                out.push((i, n, r + crate::math::length(sub(center, q)), q));
            }
            return;
        }
        let q = Aabb { min: center, max: center }.grow(r);
        self.bvh.walk(
            |b| b.overlaps(&q),
            |items| {
                for &i in items {
                    if let Some((n, d, p)) = sphere_triangle_overlap(center, r, &self.triangles[i as usize], true) {
                        out.push((i, n, d, p));
                    }
                }
                true
            },
        );
    }

    /// The stored edges of triangle `t`.
    pub fn triangle_edges(&self, t: u32) -> &[CollisionEdge] {
        let (s, n) = self.edge_ranges[t as usize];
        &self.edges[s as usize..s as usize + n as usize]
    }

    /// Every ledge of the mesh (collision.md 1.1; the spec marks the thresholds as not yet confirmed on the game).
    pub fn ledges(&self) -> Vec<Ledge> {
        let mut out = Vec::new();
        for t in 0..self.triangles.len() as u32 {
            for e in self.triangle_edges(t) {
                let (a, b) = (&self.triangles[t as usize], &self.triangles[e.other as usize]);
                let (floor, wall) = if a.normal[2] > FLOOR_Z && b.normal[2] < WALL_Z {
                    (t, e.other as u32)
                } else if b.normal[2] > FLOOR_Z && a.normal[2] < WALL_Z {
                    (e.other as u32, t)
                } else {
                    continue;
                };
                let ends = [self.positions[e.ends[0] as usize], self.positions[e.ends[1] as usize]];
                let dir = sub(ends[1], ends[0]);
                let mut outward = normalize([-dir[1], dir[0], 0.0]);
                if dot(outward, self.triangles[wall as usize].normal) < 0.0 {
                    outward = scale(outward, -1.0);
                }
                out.push(Ledge { ends, outward, floor, wall });
            }
        }
        out
    }

    /// Pushes the segment `a b` through the collision tree (collision.md 2) and calls `visit` with every node it
    /// reaches: a node's positive child is visited when part of the segment is on the side its triangle's normal
    /// points to (within `eps`), its negative child when part is behind.
    pub fn tree_segment(&self, a: Vec3, b: Vec3, eps: f32, mut visit: impl FnMut(u32)) {
        if self.tree.nodes.is_empty() {
            return;
        }
        let mut stack: Vec<u32> = Vec::with_capacity(64);
        stack.push(0);
        // Parsed trees are acyclic with one parent per node (formats checks it); a tree built by hand might not be,
        // so the walk stops after as many visits as there are nodes.
        let mut budget = self.tree.nodes.len();
        while let Some(ni) = stack.pop() {
            if budget == 0 {
                break;
            }
            budget -= 1;
            visit(ni);
            let node = self.tree.nodes[ni as usize];
            let tri = &self.triangles[node.triangle as usize];
            let (da, db) =
                if tri.is_degenerate() { (0.0, 0.0) } else { (tri.signed_distance(a), tri.signed_distance(b)) };
            if node.negative != NO_CHILD_WIDE && da.min(db) < eps {
                stack.push(node.negative);
            }
            if node.positive != NO_CHILD_WIDE && da.max(db) > -eps {
                stack.push(node.positive);
            }
        }
    }

    /// Ray cast through the collision tree instead of the hierarchy (the triangles of the nodes the segment
    /// reaches). Same result as [`Self::ray_cast`]; kept to check the tree and for comparison.
    pub fn tree_ray_cast(&self, origin: Vec3, dir: Vec3, t_max: f32, two_sided: bool) -> Option<MeshHit> {
        let end = add(origin, scale(dir, t_max));
        let mut best: Option<MeshHit> = None;
        self.tree_segment(origin, end, 1e-3, |ni| {
            let i = self.tree.nodes[ni as usize].triangle;
            let limit = best.map_or(t_max, |b| b.t);
            if let Some((t, front)) = ray_triangle(origin, dir, &self.triangles[i as usize], limit, two_sided)
                && best.is_none_or(|b| t < b.t)
            {
                best = Some(MeshHit { t, triangle: i, front });
            }
        });
        best
    }

    /// Normal of triangle `t` (zero for a degenerate one).
    pub fn normal(&self, t: u32) -> Vec3 {
        self.triangles[t as usize].normal
    }

    /// Whether the stored edge `e` of triangle `t` is convex: the other triangle lies behind this one's plane
    /// (collision.md 1). `None` when the far corner is on the plane (a flat edge).
    pub fn edge_is_convex(&self, t: u32, e: &CollisionEdge) -> Option<bool> {
        let tri = &self.triangles[t as usize];
        let other = &self.triangles[e.other as usize];
        // The other triangle's corner farthest from this plane.
        let far =
            other.v.iter().map(|&v| tri.signed_distance(v)).fold(0.0f32, |m, d| if d.abs() > m.abs() { d } else { m });
        let size = crate::math::length(cross(sub(tri.v[1], tri.v[0]), sub(tri.v[2], tri.v[0])));
        if far.abs() <= 1e-4 * size.clamp(1.0, 1e4) { None } else { Some(far < 0.0) }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;
    use formats::collision::{CollisionTriangle, NO_CHILD, TreeNode};

    /// A step: floor z = 0 for x < 5, a wall x = 5 facing -x from z = 0 to 2, and a top z = 2 for x >= 5.
    pub fn step_mesh() -> (CollisionMesh, CollisionTree) {
        let positions = vec![
            [0.0, 0.0, 0.0],
            [5.0, 0.0, 0.0],
            [5.0, 10.0, 0.0],
            [0.0, 10.0, 0.0],
            [5.0, 0.0, 2.0],
            [5.0, 10.0, 2.0],
            [10.0, 0.0, 2.0],
            [10.0, 10.0, 2.0],
        ];
        let tri = |c: [u16; 3], first_edge: u32, edge_count: u8| CollisionTriangle {
            corners: c,
            radius: 10.0,
            center: [0.0; 3],
            first_edge,
            edge_count,
        };
        let triangles = vec![
            tri([0, 1, 2], 0, 0),
            tri([0, 2, 3], 0, 0),
            tri([1, 4, 5], 0, 0),
            tri([1, 5, 2], 0, 0),
            tri([4, 6, 7], 0, 1),
            tri([4, 7, 5], 1, 0),
        ];
        let edges = vec![CollisionEdge { ends: [4, 5], other: 2 }];
        let mesh = CollisionMesh { radius: 20.0, positions, triangles, edges };
        // A solid-volume tree: the wall's plane splits the floor side (x < 5) from the step side (x > 5); below the
        // floor, or below the step's top, is inside.
        let nodes = vec![
            TreeNode { triangle: 2, negative: 2, positive: 1 },
            TreeNode { triangle: 0, negative: NO_CHILD, positive: NO_CHILD },
            TreeNode { triangle: 4, negative: NO_CHILD, positive: NO_CHILD },
        ];
        (mesh, CollisionTree { nodes, full_count: 3 })
    }

    #[test]
    fn queries_on_a_step() {
        let (m, t) = step_mesh();
        let mesh = TriMesh::new(&m, &t);
        assert_eq!(mesh.normal(0), [0.0, 0.0, 1.0]);
        assert_eq!(mesh.normal(2), [-1.0, 0.0, 0.0]);
        let hit = mesh.ray_cast([2.0, 2.0, 5.0], [0.0, 0.0, -1.0], 100.0, false).unwrap();
        assert_eq!((hit.t, hit.triangle), (5.0, 0));
        let hit = mesh.ray_cast([0.0, 3.0, 1.0], [1.0, 0.0, 0.0], 100.0, false).unwrap();
        assert!(hit.triangle == 2 || hit.triangle == 3);
        assert_eq!(hit.t, 5.0);
        assert_eq!(mesh.tree_ray_cast([0.0, 3.0, 1.0], [1.0, 0.0, 0.0], 100.0, false).map(|h| h.t), Some(5.0));
        assert!(mesh.ray_blocked([0.0, 3.0, 1.0], [1.0, 0.0, 0.0], 100.0, false));
        assert!(!mesh.ray_blocked([0.0, 3.0, 1.0], [-1.0, 0.0, 0.0], 100.0, false));
        // A sphere sliding into the wall stops one radius short.
        let c = mesh.sphere_sweep([1.0, 3.0, 1.0], 0.5, [10.0, 0.0, 0.0], false).unwrap();
        assert!((c.contact.t - 0.35).abs() < 1e-5);
        assert_eq!(c.contact.normal, [-1.0, 0.0, 0.0]);
        // Rolling over the top edge of the step from above at z = 2.25 touches the corner edge.
        let c = mesh.sphere_sweep([2.0, 3.0, 2.25], 0.5, [5.0, 0.0, 0.0], false).unwrap();
        assert!(c.contact.normal[0] < 0.0 && c.contact.normal[2] > 0.0);
        let c = mesh.capsule_sweep([1.0, 3.0, 0.75], [1.0, 3.0, 1.75], 0.5, [10.0, 0.0, 0.0], false).unwrap();
        assert!((c.contact.t - 0.35).abs() < 1e-5);
        let mut out = Vec::new();
        mesh.sphere_overlaps([3.0, 1.0, 0.25], 0.5, &mut out);
        assert_eq!(out.len(), 1);
        // Inside the solid (under the floor, inside the step block) and outside.
        assert!(mesh.point_inside([2.0, 5.0, -1.0]));
        assert!(mesh.point_inside([8.0, 5.0, 1.0]));
        assert!(!mesh.point_inside([2.0, 5.0, 1.0]));
        assert!(!mesh.point_inside([8.0, 5.0, 3.0]));
        // A centre inside the block is pushed out through the nearest surface, the top, by r + 1.
        out.clear();
        mesh.sphere_overlaps([8.0, 5.0, 1.0], 0.5, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1, [0.0, 0.0, 1.0]);
        assert!((out[0].2 - 1.5).abs() < 1e-5);
        // Just behind a surface but outside the solid (past the step's top edge, below the top's plane): contact by
        // distance from either side.
        out.clear();
        mesh.sphere_overlaps([4.8, 5.0, 1.9], 0.5, &mut out);
        assert!(!out.is_empty());
        let ledges = mesh.ledges();
        assert_eq!(ledges.len(), 1);
        assert_eq!(ledges[0].outward, [-1.0, 0.0, 0.0]);
        assert_eq!((ledges[0].floor, ledges[0].wall), (4, 2));
        assert_eq!(mesh.edge_is_convex(4, &m.edges[0]), Some(true));
    }

    #[test]
    fn tree_walk_stops_on_a_cyclic_tree() {
        let (m, _) = step_mesh();
        let cyclic = CollisionTree {
            nodes: vec![
                TreeNode { triangle: 0, negative: 1, positive: 1 },
                TreeNode { triangle: 1, negative: 0, positive: 0 },
            ],
            full_count: 2,
        };
        let mesh = TriMesh::new(&m, &cyclic);
        let mut visits = 0;
        mesh.tree_segment([1.0, 1.0, 1.0], [1.0, 1.0, -1.0], 1e-3, |_| visits += 1);
        assert_eq!(visits, 2);
        assert!(mesh.tree_ray_cast([1.0, 1.0, 1.0], [0.0, 0.0, -1.0], 5.0, false).is_some());
    }

    #[test]
    fn wide_trees_past_16_bits() {
        use formats::collision::{WideCollisionTree, WideTreeNode};
        // A chain of 70,000 nodes over one triangle: past what 16-bit indices hold, and it still walks.
        let (mesh, _) = step_mesh();
        let n = 70_000u32;
        let nodes = (0..n)
            .map(|i| WideTreeNode {
                triangle: 0,
                negative: if i + 1 < n { i + 1 } else { NO_CHILD_WIDE },
                positive: NO_CHILD_WIDE,
            })
            .collect();
        let tree = WideCollisionTree { nodes };
        assert!(tree.validate(mesh.triangles.len()).is_ok());
        let m = TriMesh::new_wide(&mesh, tree);
        let mut visits = 0u32;
        let mut last = 0;
        m.tree_segment([1.0, 1.0, -1.0], [1.0, 1.0, -2.0], 1e-3, |i| {
            visits += 1;
            last = i;
        });
        assert_eq!((visits, last), (n, n - 1));
        let bad = WideCollisionTree {
            nodes: alloc::vec![WideTreeNode { triangle: 0, negative: 0, positive: NO_CHILD_WIDE }],
        };
        assert!(bad.validate(mesh.triangles.len()).is_err());
    }
}
