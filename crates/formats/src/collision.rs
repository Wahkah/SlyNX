//! Collision data: solid bodies, collision meshes, collision trees, convex volumes and the material tables
//! (classes/solid-object.md section 1, classes/shared-structures.md, collision.md).
//!
//! Everything is copied out into owned, aligned arrays; the engine builds its run-time structures from these.

use alloc::vec::Vec;

use crate::disc::Executable;
use crate::error::{Error, Result};
use crate::options::{OptionList, OptionTable};
use crate::reader::{Reader, Vec3};

/// One edge record of a collision triangle (collision.md 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollisionEdge {
    /// End points (position indices).
    pub ends: [u16; 2],
    /// The triangle on the other side.
    pub other: u16,
}

/// One collision triangle ("surface").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionTriangle {
    /// Corner position indices.
    pub corners: [u16; 3],
    /// Bounding radius.
    pub radius: f32,
    /// Bounding centre.
    pub center: Vec3,
    /// First edge in [`CollisionMesh::edges`].
    pub first_edge: u32,
    /// Number of edges stored under this triangle.
    pub edge_count: u8,
}

/// A collision mesh, in the owning object's local space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollisionMesh {
    /// Bounding radius.
    pub radius: f32,
    /// Shared positions.
    pub positions: Vec<Vec3>,
    /// Triangles.
    pub triangles: Vec<CollisionTriangle>,
    /// Every edge record, grouped by owning triangle.
    pub edges: Vec<CollisionEdge>,
}

impl CollisionMesh {
    /// Reads a collision mesh.
    pub fn read(r: &mut Reader<'_>) -> Result<CollisionMesh> {
        let radius = r.f32()?;
        let np = r.u16()? as usize;
        let mut positions = Vec::with_capacity(np);
        for _ in 0..np {
            positions.push(r.vec3()?);
        }
        let nt = r.u16()? as usize;
        let edge_total = r.u16()? as usize;
        let mut triangles = Vec::with_capacity(nt);
        let mut edges = Vec::with_capacity(edge_total);
        for _ in 0..nt {
            let at = r.pos();
            let corners = [r.u16()?, r.u16()?, r.u16()?];
            if corners.iter().any(|&c| c as usize >= np) {
                return Err(Error::Corrupt { offset: at, what: "collision triangle corner out of range" });
            }
            let radius = r.f32()?;
            let center = r.vec3()?;
            let at = r.pos();
            let n = r.i8()?;
            if n < 0 {
                return Err(Error::Corrupt { offset: at, what: "negative edge count" });
            }
            let first_edge = edges.len() as u32;
            for _ in 0..n {
                let at = r.pos();
                let e = CollisionEdge { ends: [r.u16()?, r.u16()?], other: r.u16()? };
                if e.ends.iter().any(|&p| p as usize >= np) || e.other as usize >= nt {
                    return Err(Error::Corrupt { offset: at, what: "collision edge index out of range" });
                }
                edges.push(e);
            }
            triangles.push(CollisionTriangle { corners, radius, center, first_edge, edge_count: n as u8 });
        }
        if edges.len() != edge_total {
            return Err(r.corrupt("collision edge total does not match"));
        }
        Ok(CollisionMesh { radius, positions, triangles, edges })
    }

    /// The edges stored under triangle `t`.
    pub fn triangle_edges(&self, t: usize) -> &[CollisionEdge] {
        let tri = &self.triangles[t];
        let s = tri.first_edge as usize;
        &self.edges[s..s + tri.edge_count as usize]
    }

    /// The three corner positions of triangle `t`.
    pub fn corners(&self, t: usize) -> [Vec3; 3] {
        let c = self.triangles[t].corners;
        [self.positions[c[0] as usize], self.positions[c[1] as usize], self.positions[c[2] as usize]]
    }

    /// Whether the mesh has no triangles.
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }

    /// For each surface, the position indices of the edges that name it: stored under it, and stored under another
    /// surface with it as the other side (specs/sly1/collision.md 1.2: surface 1275's edges are stored under 1275,
    /// 1277 and 1278).
    pub fn edge_ends(&self) -> Vec<Vec<u16>> {
        let mut out = alloc::vec![Vec::new(); self.triangles.len()];
        for (t, tri) in self.triangles.iter().enumerate() {
            let s = tri.first_edge as usize;
            for e in self.edges.get(s..s + tri.edge_count as usize).unwrap_or(&[]) {
                out[t].extend(e.ends);
                if let Some(o) = out.get_mut(e.other as usize) {
                    o.extend(e.ends);
                }
            }
        }
        out
    }

    /// The polygon surface `t` covers: the convex hull, in the plane of its three corners, of the corners and every
    /// end of the edges naming it, as position indices around it (collinear points dropped). Only convex shared
    /// sides are stored and corners may lie inside, so the hull of all of them is the safe outline (collision.md
    /// 1.2). `ends` is [`CollisionMesh::edge_ends`].
    pub fn surface_polygon(&self, t: usize, ends: &[Vec<u16>]) -> Vec<u16> {
        let Some(tri) = self.triangles.get(t) else { return Vec::new() };
        let mut pts: Vec<u16> = tri.corners.to_vec();
        for &v in ends.get(t).map_or(&[][..], |e| &e[..]) {
            if !pts.contains(&v) && (v as usize) < self.positions.len() {
                pts.push(v);
            }
        }
        if pts.len() <= 3 {
            return tri.corners.to_vec();
        }
        let sub = |x: Vec3, y: Vec3| [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
        let cross =
            |x: Vec3, y: Vec3| [x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]];
        let dot = |x: Vec3, y: Vec3| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
        let [a, b, c] = self.corners(t);
        let n = cross(sub(b, a), sub(c, a));
        if dot(n, n) == 0.0 {
            return tri.corners.to_vec();
        }
        let (u, w) = (sub(b, a), cross(n, sub(b, a)));
        let mut q: Vec<(f32, f32, u16)> = pts
            .iter()
            .map(|&v| {
                let d = sub(self.positions[v as usize], a);
                (dot(d, u), dot(d, w), v)
            })
            .collect();
        q.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.total_cmp(&y.1)));
        let turn = |o: (f32, f32, u16), p: (f32, f32, u16), r: (f32, f32, u16)| {
            (p.0 - o.0) * (r.1 - o.1) - (p.1 - o.1) * (r.0 - o.0)
        };
        let mut hull: Vec<(f32, f32, u16)> = Vec::with_capacity(q.len() * 2);
        for pass in 0..2 {
            let start = hull.len();
            let order: Vec<_> = if pass == 0 { q.clone() } else { q.iter().rev().copied().collect() };
            for p in order {
                while hull.len() >= start + 2 && turn(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0 {
                    hull.pop();
                }
                hull.push(p);
            }
            hull.pop();
        }
        hull.into_iter().map(|h| h.2).collect()
    }

    /// The mesh with each surface covered by its polygon ([`CollisionMesh::surface_polygon`]) as a fan: the stored
    /// surfaces first at their indices (the tree names surfaces by index), then the extra fan triangles, without
    /// edges. A fan triangle over the three stored corners is not added again.
    pub fn with_polygons(&self) -> CollisionMesh {
        let ends = self.edge_ends();
        let mut out = self.clone();
        for t in 0..self.triangles.len() {
            let tri = self.triangles[t];
            let ring = self.surface_polygon(t, &ends);
            if ring.len() <= 3 {
                continue;
            }
            for i in 1..ring.len() - 1 {
                let corners = [ring[0], ring[i], ring[i + 1]];
                if corners.iter().all(|v| tri.corners.contains(v)) {
                    continue;
                }
                out.triangles.push(CollisionTriangle { corners, edge_count: 0, first_edge: 0, ..tri });
            }
        }
        out
    }
}

/// "No child" in a collision tree.
pub const NO_CHILD: u16 = 0xFFFF;

/// One collision-tree node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeNode {
    /// The triangle whose plane splits space at this node.
    pub triangle: u16,
    /// Child behind the plane, or [`NO_CHILD`].
    pub negative: u16,
    /// Child on the side the normal points to (open space), or [`NO_CHILD`].
    pub positive: u16,
}

/// A collision tree (collision.md 2).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollisionTree {
    /// Nodes; node 0 is the root.
    pub nodes: Vec<TreeNode>,
    /// The "full node count" field.
    pub full_count: u16,
}

impl CollisionTree {
    /// Reads a tree whose triangle indices refer to a mesh of `triangles` triangles.
    pub fn read(r: &mut Reader<'_>, triangles: usize) -> Result<CollisionTree> {
        let n = r.u16()? as usize;
        let full_count = r.u16()?;
        let mut nodes = Vec::with_capacity(n.min(r.remaining() / 6));
        // On the disc every child comes after its parent and has exactly one parent (checked on all 46 worlds);
        // requiring it rules out cycles and shared subtrees, so a walk of the tree visits each node once.
        let mut has_parent = alloc::vec![false; n];
        for i in 0..n {
            let at = r.pos();
            let node = TreeNode { triangle: r.u16()?, negative: r.u16()?, positive: r.u16()? };
            if node.triangle as usize >= triangles {
                return Err(Error::Corrupt { offset: at, what: "collision tree index out of range" });
            }
            for c in [node.negative, node.positive] {
                if c == NO_CHILD {
                    continue;
                }
                let c = c as usize;
                if c >= n || c <= i {
                    return Err(Error::Corrupt { offset: at, what: "collision tree child not after its parent" });
                }
                if core::mem::replace(&mut has_parent[c], true) {
                    return Err(Error::Corrupt { offset: at, what: "collision tree node with two parents" });
                }
            }
            nodes.push(node);
        }
        Ok(CollisionTree { nodes, full_count })
    }
}

/// "No child" in a [`WideCollisionTree`].
pub const NO_CHILD_WIDE: u32 = u32::MAX;

/// A collision-tree node with 32-bit indices, for trees past 65,535 nodes (three Sly 3 level objects have 75,558
/// to 103,211).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WideTreeNode {
    /// The triangle whose plane splits space at this node.
    pub triangle: u32,
    /// Child behind the plane, or [`NO_CHILD_WIDE`].
    pub negative: u32,
    /// Child on the side the normal points to, or [`NO_CHILD_WIDE`].
    pub positive: u32,
}

/// A collision tree with 32-bit indices: the form the engine queries. A [`CollisionTree`] (16-bit, as stored by
/// Sly 1) converts to it losslessly.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WideCollisionTree {
    /// Nodes; node 0 is the root.
    pub nodes: Vec<WideTreeNode>,
}

impl From<&CollisionTree> for WideCollisionTree {
    fn from(t: &CollisionTree) -> Self {
        let wide = |c: u16| if c == NO_CHILD { NO_CHILD_WIDE } else { c as u32 };
        WideCollisionTree {
            nodes: t
                .nodes
                .iter()
                .map(|n| WideTreeNode {
                    triangle: n.triangle as u32,
                    negative: wide(n.negative),
                    positive: wide(n.positive),
                })
                .collect(),
        }
    }
}

impl WideCollisionTree {
    /// Checks the tree's shape against a mesh of `triangles` triangles: every triangle index in range, every child
    /// after its parent and with one parent (the rule [`CollisionTree::read`] applies), so a walk visits each node
    /// once.
    pub fn validate(&self, triangles: usize) -> Result<()> {
        let n = self.nodes.len();
        let mut has_parent = alloc::vec![false; n];
        for (i, node) in self.nodes.iter().enumerate() {
            if node.triangle as usize >= triangles {
                return Err(Error::Corrupt { offset: i, what: "collision tree index out of range" });
            }
            for c in [node.negative, node.positive] {
                if c == NO_CHILD_WIDE {
                    continue;
                }
                let c = c as usize;
                if c >= n || c <= i {
                    return Err(Error::Corrupt { offset: i, what: "collision tree child not after its parent" });
                }
                if core::mem::replace(&mut has_parent[c], true) {
                    return Err(Error::Corrupt { offset: i, what: "collision tree node with two parents" });
                }
            }
        }
        Ok(())
    }
}

/// A plane `n · x = d`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plane {
    /// Normal.
    pub normal: Vec3,
    /// Distance.
    pub d: f32,
}

/// A convex-volume tree node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeNode {
    /// Plane index.
    pub plane: u16,
    /// Negative child or −1.
    pub negative: i16,
    /// Positive child or −1.
    pub positive: i16,
}

/// A convex volume (collision.md 5), in its owner's local space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConvexVolume {
    /// Planes.
    pub planes: Vec<Plane>,
    /// Nodes; node 0 is the root.
    pub nodes: Vec<VolumeNode>,
}

impl ConvexVolume {
    /// Reads a convex volume.
    pub fn read(r: &mut Reader<'_>) -> Result<ConvexVolume> {
        let np = r.u16()? as usize;
        let mut planes = Vec::with_capacity(np);
        for _ in 0..np {
            let d = r.f32()?;
            let normal = r.vec3()?;
            planes.push(Plane { normal, d });
        }
        let nn = r.u16()? as usize;
        let mut nodes = Vec::with_capacity(nn);
        for _ in 0..nn {
            let at = r.pos();
            let node = VolumeNode { plane: r.u16()?, negative: r.i16()?, positive: r.i16()? };
            let bad_child = |c: i16| c < -1 || (c >= 0 && c as usize >= nn);
            if node.plane as usize >= np || bad_child(node.negative) || bad_child(node.positive) {
                return Err(Error::Corrupt { offset: at, what: "convex volume index out of range" });
            }
            nodes.push(node);
        }
        Ok(ConvexVolume { planes, nodes })
    }

    /// Parses a convex volume at `offset`.
    pub fn parse(data: &[u8], offset: usize) -> Result<ConvexVolume> {
        ConvexVolume::read(&mut Reader::at(data, offset))
    }
}

/// One camera-fade group entry (solid layout row 7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FadeEntry {
    /// 1 starts a group, −1 continues it.
    pub key: i16,
    /// Mesh index into the object's mesh set.
    pub mesh: u16,
}

/// The collision part of a solid body (solid layout rows 1 to 12).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Solid {
    /// File offset of the body.
    pub offset: usize,
    /// Physics flags (bit 0: fixed).
    pub physics_flags: u8,
    /// Collision mesh.
    pub mesh: CollisionMesh,
    /// Collision tree.
    pub tree: CollisionTree,
    /// Mass.
    pub mass: f32,
    /// Moment of inertia (columns).
    pub inertia: [Vec3; 3],
    /// Centre of mass.
    pub center_of_mass: Vec3,
    /// Camera-fade groups.
    pub fade_groups: Vec<FadeEntry>,
    /// Camera-fade node map (one entry per tree node when present).
    pub fade_node_map: Vec<i16>,
    /// Hit-group count (0 on the whole disc; the groups themselves are skipped).
    pub hit_groups: u32,
    /// Camera collision mesh (empty in most solids; the camera then uses [`Self::mesh`]).
    pub camera_mesh: CollisionMesh,
    /// Camera collision tree.
    pub camera_tree: CollisionTree,
    /// File offset of the placed part (row 13).
    pub placed: usize,
}

impl Solid {
    /// Bit 0 of the physics flags.
    pub const FIXED: u8 = 0x01;

    /// Parses the collision part of a solid body at `offset` (a solid-layout record's offset + 6).
    pub fn parse(data: &[u8], offset: usize) -> Result<Solid> {
        let mut r = Reader::at(data, offset);
        let physics_flags = r.u8()?;
        let mesh = CollisionMesh::read(&mut r)?;
        let tree = CollisionTree::read(&mut r, mesh.triangles.len())?;
        let mass = r.f32()?;
        let inertia = r.mat3()?;
        let center_of_mass = r.vec3()?;
        let ng = r.u16()? as usize;
        let mut fade_groups = Vec::with_capacity(ng);
        for _ in 0..ng {
            fade_groups.push(FadeEntry { key: r.i16()?, mesh: r.u16()? });
        }
        let nm = r.u32()? as usize;
        let mut fade_node_map = Vec::with_capacity(nm.min(r.remaining() / 2));
        for _ in 0..nm {
            fade_node_map.push(r.i16()?);
        }
        let hit_groups = r.u32()?;
        r.skip_n(hit_groups as usize, 4)?;
        if hit_groups > 0 {
            r.skip_n(mesh.triangles.len() + 1, 2)?;
        }
        let camera_mesh = CollisionMesh::read(&mut r)?;
        let camera_tree = CollisionTree::read(&mut r, camera_mesh.triangles.len())?;
        Ok(Solid {
            offset,
            physics_flags,
            mesh,
            tree,
            mass,
            inertia,
            center_of_mass,
            fade_groups,
            fade_node_map,
            hit_groups,
            camera_mesh,
            camera_tree,
            placed: r.pos(),
        })
    }

    /// Whether the solid is fixed (never moves).
    pub fn is_fixed(&self) -> bool {
        self.physics_flags & Self::FIXED != 0
    }

    /// The mesh and tree the camera collides with: the camera mesh when it has triangles, else the normal mesh.
    pub fn camera_collision(&self) -> (&CollisionMesh, &CollisionTree) {
        if self.camera_mesh.is_empty() { (&self.mesh, &self.tree) } else { (&self.camera_mesh, &self.camera_tree) }
    }
}

/// The option that sets a solid's material (collision.md 6).
pub const MATERIAL_OPTION: i16 = 128;

/// A solid's material from its option list (option 128; 0 when absent).
pub fn material_of(data: &[u8], options: &OptionList, table: &OptionTable) -> Result<i16> {
    for item in options.iter(data, table) {
        let (id, v) = item?;
        if id == MATERIAL_OPTION && v.len() >= 2 {
            return Ok(i16::from_le_bytes([v[0], v[1]]));
        }
    }
    Ok(0)
}

/// Number of entries in each material table.
pub const MATERIAL_COUNT: usize = 13;
/// Virtual address of the friction table in SCUS_971.98.
pub const FRICTION_TABLE_VA: u32 = 0x0026_4840;
/// Virtual address of the elasticity table in SCUS_971.98.
pub const ELASTICITY_TABLE_VA: u32 = 0x0026_48A8;

/// One material-table entry: a weight and a weighted value.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MaterialEntry {
    /// Weight `w`.
    pub weight: f32,
    /// Weighted value `a`.
    pub value: f32,
}

/// The friction and elasticity tables (collision.md 6), read from the player's executable.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialTables {
    /// Friction entries.
    pub friction: [MaterialEntry; MATERIAL_COUNT],
    /// Elasticity entries.
    pub elasticity: [MaterialEntry; MATERIAL_COUNT],
}

fn read_material_table(exe: &Executable, va: u32) -> Result<[MaterialEntry; MATERIAL_COUNT]> {
    let raw = exe.bytes_at(va, MATERIAL_COUNT * 8)?;
    let mut out = [MaterialEntry::default(); MATERIAL_COUNT];
    for (e, c) in out.iter_mut().zip(raw.as_chunks::<8>().0) {
        e.weight = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        e.value = f32::from_le_bytes([c[4], c[5], c[6], c[7]]);
    }
    Ok(out)
}

impl MaterialTables {
    /// Reads both tables, after checking the executable's SHA-256 (refuses any other executable).
    pub fn from_executable(exe: &Executable) -> Result<MaterialTables> {
        if !exe.is_supported() {
            return Err(Error::UnknownExecutable);
        }
        Ok(MaterialTables {
            friction: read_material_table(exe, FRICTION_TABLE_VA)?,
            elasticity: read_material_table(exe, ELASTICITY_TABLE_VA)?,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::geometry::tests::W;

    /// A unit square in the z = 0 plane (two triangles, normal +z) with one stored edge, a tree over both
    /// triangles, an empty camera mesh and one fade group.
    pub fn sample_solid(w: &mut W) {
        w.u8(0);
        w.f32(1.5).u16(4);
        for p in [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]] {
            w.v3(p);
        }
        w.u16(2).u16(1);
        w.u16(0).u16(1).u16(2).f32(0.8).v3([0.6, 0.3, 0.]).i8(1).u16(0).u16(2).u16(1);
        w.u16(0).u16(2).u16(3).f32(0.8).v3([0.3, 0.6, 0.]).i8(0);
        w.u16(2).u16(2).u16(0).u16(NO_CHILD).u16(1).u16(1).u16(NO_CHILD).u16(NO_CHILD);
        w.f32(10.0).ident().v3([0.5, 0.5, 0.]);
        w.u16(1).i16(1).u16(0);
        w.u32(2).i16(0).i16(-1);
        w.u32(0);
        w.f32(0.0).u16(0).u16(0).u16(0);
        w.u16(0).u16(0);
    }

    #[test]
    fn parses_a_solid() {
        let mut w = W::default();
        sample_solid(&mut w);
        let s = Solid::parse(&w.0, 0).unwrap();
        assert_eq!(s.placed, w.0.len());
        assert_eq!(s.mesh.positions.len(), 4);
        assert_eq!(s.mesh.triangles.len(), 2);
        assert_eq!(s.mesh.triangle_edges(0), &[CollisionEdge { ends: [0, 2], other: 1 }]);
        assert!(s.mesh.triangle_edges(1).is_empty());
        assert_eq!(s.tree.nodes[0], TreeNode { triangle: 0, negative: NO_CHILD, positive: 1 });
        assert_eq!(s.fade_groups, [FadeEntry { key: 1, mesh: 0 }]);
        assert_eq!(s.fade_node_map, [0, -1]);
        assert!(!s.is_fixed());
        assert_eq!(s.camera_collision().0.triangles.len(), 2);
    }

    #[test]
    fn rejects_cyclic_and_shared_tree_links() {
        let tree = |links: &[(u16, u16)]| {
            let mut w = W::default();
            w.u16(links.len() as u16).u16(links.len() as u16);
            for &(neg, pos) in links {
                w.u16(0).u16(neg).u16(pos);
            }
            CollisionTree::read(&mut Reader::new(&w.0), 1)
        };
        assert!(tree(&[(NO_CHILD, 1), (2, NO_CHILD), (NO_CHILD, NO_CHILD)]).is_ok());
        // A child pointing back at its parent (a cycle), at itself, and a node with two parents.
        assert!(tree(&[(NO_CHILD, 1), (0, NO_CHILD)]).is_err());
        assert!(tree(&[(NO_CHILD, 0)]).is_err());
        assert!(tree(&[(2, 1), (2, NO_CHILD), (NO_CHILD, NO_CHILD)]).is_err());
    }

    #[test]
    fn rejects_out_of_range_indices() {
        let mut w = W::default();
        w.f32(1.0).u16(1).v3([0.; 3]).u16(1).u16(0).u16(0).u16(0).u16(3).f32(0.).v3([0.; 3]).i8(0);
        assert!(CollisionMesh::read(&mut Reader::new(&w.0)).is_err());
        let mut w = W::default();
        w.u16(1).v3([0.; 3]).f32(0.).u16(1).u16(0).i16(5).i16(-1);
        assert!(ConvexVolume::parse(&w.0, 0).is_err());
    }
}
