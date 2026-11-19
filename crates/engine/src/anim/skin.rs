//! Skinning and poses (animation.md section 8).
//!
//! - [`Skin`] holds a mesh set's bones (as rig nodes) and their bind matrices `K_b = W_b⁻¹ · W_mesh`, captured
//!   once the world has loaded; each frame it produces `S_b = W_mesh⁻¹ · W_b · K_b` as [`BoneMatrix`] rows ready
//!   for a GPU bone buffer.
//! - [`SkinnedSubmesh`] holds a submesh's per-vertex influences and pose deltas in the order of the renderer's
//!   vertices (one per strip entry, as `formats::geometry::Submesh::append_gpu` writes them), for GPU attributes or
//!   a CPU fallback. Poses are applied before skinning; weights are used as stored (not renormalised).

use alloc::vec::Vec;

use formats::geometry::{Affine, MeshSet, Submesh};
use formats::stream::ObjectStream;

use super::rig::Rig;
use crate::math::{Vec3, affine_inverse, madd, mat_mul_vec, normalize, sub};

/// One bone matrix for a GPU buffer: the 3 x 4 affine matrix as three rows (`[m00 m01 m02 tx]`, ...), 48 bytes.
/// A vertex maps to `(row0 · p1, row1 · p1, row2 · p1)` with `p1 = (x, y, z, 1)`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneMatrix {
    /// Rows of the 3 x 4 matrix.
    pub rows: [[f32; 4]; 3],
}

impl BoneMatrix {
    /// The identity.
    pub const IDENTITY: BoneMatrix =
        BoneMatrix { rows: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]] };

    /// From an affine transform (columns + translation).
    pub fn from_affine(a: &Affine) -> BoneMatrix {
        let c = &a.cols;
        BoneMatrix {
            rows: [
                [c[0][0], c[1][0], c[2][0], a.t[0]],
                [c[0][1], c[1][1], c[2][1], a.t[1]],
                [c[0][2], c[1][2], c[2][2], a.t[2]],
            ],
        }
    }

    /// Back to an affine transform.
    pub fn to_affine(&self) -> Affine {
        let r = &self.rows;
        Affine {
            cols: [[r[0][0], r[1][0], r[2][0]], [r[0][1], r[1][1], r[2][1]], [r[0][2], r[1][2], r[2][2]]],
            t: [r[0][3], r[1][3], r[2][3]],
        }
    }
}

/// A mesh set's bones and bind matrices.
#[derive(Clone, Debug, PartialEq)]
pub struct Skin {
    /// The rig node that owns the mesh set.
    pub mesh_node: u32,
    /// The rig node of each bone of the set's bone list (`None`: not found; its matrix stays the identity).
    pub bones: Vec<Option<u32>>,
    /// Bind matrix `K_b` per bone.
    pub bind: Vec<Affine>,
}

impl Skin {
    /// A skin with the given bones and identity bind matrices.
    pub fn new(mesh_node: u32, bones: Vec<Option<u32>>) -> Skin {
        let bind = alloc::vec![Affine::IDENTITY; bones.len()];
        Skin { mesh_node, bones, bind }
    }

    /// Resolves the bones of the mesh set of `mesh_record`: each bone OID names the object with that OID nearest
    /// to the mesh set's owner (animation.md 8.1, object-stream.md 7.4), which must be a node of `rig`.
    pub fn from_stream(rig: &Rig, stream: &ObjectStream, mesh_record: u32, set: &MeshSet) -> Option<Skin> {
        let mesh_node = rig.node_of_record(mesh_record)?;
        let bones = set
            .bones
            .iter()
            .map(|&oid| stream.find_nearest(mesh_record, oid, false).and_then(|r| rig.node_of_record(r)))
            .collect();
        Some(Skin::new(mesh_node, bones))
    }

    /// Records the bind matrices `K_b = W_b⁻¹ · W_mesh` from the current world transforms (call once the world
    /// has loaded, before the first frame).
    pub fn capture_bind(&mut self, world: &[Affine]) {
        let mesh = world[self.mesh_node as usize];
        for (k, b) in self.bind.iter_mut().zip(&self.bones) {
            *k = match b.and_then(|b| affine_inverse(&world[b as usize])) {
                Some(inv) => inv.mul(&mesh),
                None => Affine::IDENTITY,
            };
        }
    }

    /// The skinning matrices `S_b = W_mesh⁻¹ · W_b · K_b` for the current world transforms. `out` is resized to the
    /// bone count. Unresolved bones, and every bone while the mesh transform is singular, give the identity.
    pub fn matrices(&self, world: &[Affine], out: &mut Vec<Affine>) {
        out.clear();
        let inv_mesh = affine_inverse(&world[self.mesh_node as usize]);
        for (k, b) in self.bind.iter().zip(&self.bones) {
            out.push(match (inv_mesh, b) {
                (Some(im), Some(b)) => im.mul(&world[*b as usize]).mul(k),
                _ => Affine::IDENTITY,
            });
        }
    }

    /// Like [`Self::matrices`], written as GPU rows into `out` (resized to the bone count).
    pub fn bone_matrices(&self, world: &[Affine], out: &mut Vec<BoneMatrix>) {
        let mut tmp = Vec::with_capacity(self.bones.len());
        self.matrices(world, &mut tmp);
        out.clear();
        out.extend(tmp.iter().map(BoneMatrix::from_affine));
    }
}

/// The weighted matrix `M = Σ wᵢ · S_{bᵢ}` of one vertex, skipping zero weights; `None` when every weight is 0 (the
/// vertex stays in place). Bone indices out of range are skipped.
pub fn blend_matrix(matrices: &[Affine], bones: [u8; 4], weights: [f32; 4]) -> Option<Affine> {
    let mut m = Affine { cols: [[0.0; 3]; 3], t: [0.0; 3] };
    let mut any = false;
    for (&b, &w) in bones.iter().zip(&weights) {
        if w == 0.0 {
            continue;
        }
        let Some(s) = matrices.get(b as usize) else { continue };
        any = true;
        for c in 0..3 {
            m.cols[c] = madd(m.cols[c], s.cols[c], w);
        }
        m.t = madd(m.t, s.t, w);
    }
    any.then_some(m)
}

/// Skins one vertex: `position' = M · position`, `normal' = normalise(M₃ₓ₃ · normal)` (animation.md 8.2).
pub fn skin_vertex(
    matrices: &[Affine],
    bones: [u8; 4],
    weights: [f32; 4],
    position: Vec3,
    normal: Vec3,
) -> (Vec3, Vec3) {
    match blend_matrix(matrices, bones, weights) {
        Some(m) => (m.transform_point(position), normalize(mat_mul_vec(&m.cols, normal))),
        None => (position, normal),
    }
}

/// Per-vertex skinning attributes for a GPU vertex stream, aligned with the renderer's vertices.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SkinVertex {
    /// Indices into the mesh set's bone list (the instance's bone matrices).
    pub bones: [u8; 4],
    /// Weights, as stored (not renormalised; all zero means "not skinned").
    pub weights: [f32; 4],
}

/// One pose's change to one vertex.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoseDelta {
    /// Vertex (strip entry).
    pub vertex: usize,
    /// Position change (pose position minus base).
    pub position: Vec3,
    /// Normal change (pose normal minus base).
    pub normal: Vec3,
}

/// A submesh's skinning and pose data, one vertex per strip entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkinnedSubmesh {
    /// Base positions.
    pub positions: Vec<Vec3>,
    /// Base normals (zero where the entry has none).
    pub normals: Vec<Vec3>,
    /// Influences per vertex.
    pub influences: Vec<SkinVertex>,
    /// For each pose of the mesh set, the vertices it changes.
    pub poses: Vec<Vec<PoseDelta>>,
}

impl SkinnedSubmesh {
    /// Reads a submesh of a mesh set with `pose_count` poses.
    pub fn from_submesh(data: &[u8], sm: &Submesh, pose_count: usize) -> SkinnedSubmesh {
        let entries = sm.entries(data);
        let mut out = SkinnedSubmesh {
            positions: Vec::with_capacity(entries.len()),
            normals: Vec::with_capacity(entries.len()),
            influences: Vec::with_capacity(entries.len()),
            poses: alloc::vec![Vec::new(); if sm.pose.is_some() { pose_count } else { 0 }],
        };
        for e in &entries {
            let p = sm.position(data, e[0] as usize);
            let n = if e[1] == 0xFF { [0.0; 3] } else { sm.normal(data, e[1] as usize) };
            let (bones, weights) = sm.influences(data, e[0] as usize);
            out.positions.push(p);
            out.normals.push(n);
            out.influences.push(SkinVertex { bones, weights });
        }
        for (pose, deltas) in out.poses.iter_mut().enumerate() {
            for k in 0..entries.len() {
                let dp = sm.pose_position(data, pose, k).map(|p| sub(p, out.positions[k]));
                let dn = sm.pose_normal(data, pose, k).map(|n| sub(n, out.normals[k]));
                if dp.is_some() || dn.is_some() {
                    deltas.push(PoseDelta {
                        vertex: k,
                        position: dp.unwrap_or([0.0; 3]),
                        normal: dn.unwrap_or([0.0; 3]),
                    });
                }
            }
        }
        out
    }

    /// Number of vertices.
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// Whether the submesh has no vertices.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Whether any vertex has a non-zero weight.
    pub fn is_skinned(&self) -> bool {
        self.influences.iter().any(|v| v.weights.iter().any(|&w| w != 0.0))
    }

    /// Applies the poses (`position + Σ w_p · Δpos_p`, same for normals; animation.md 8.3) into `positions` and
    /// `normals` (resized). Weights beyond the pose count are ignored; missing weights count as 0.
    pub fn morph(&self, pose_weights: &[f32], positions: &mut Vec<Vec3>, normals: &mut Vec<Vec3>) {
        positions.clear();
        positions.extend_from_slice(&self.positions);
        normals.clear();
        normals.extend_from_slice(&self.normals);
        for (deltas, &w) in self.poses.iter().zip(pose_weights) {
            if w == 0.0 {
                continue;
            }
            for d in deltas {
                let k = d.vertex;
                positions[k] = madd(positions[k], d.position, w);
                normals[k] = madd(normals[k], d.normal, w);
            }
        }
    }

    /// The CPU path: poses first, then skinning with `matrices` (from [`Skin::matrices`]). Results are in the mesh
    /// owner's local space.
    pub fn deform(
        &self,
        matrices: &[Affine],
        pose_weights: &[f32],
        positions: &mut Vec<Vec3>,
        normals: &mut Vec<Vec3>,
    ) {
        self.morph(pose_weights, positions, normals);
        for ((p, n), inf) in positions.iter_mut().zip(normals.iter_mut()).zip(&self.influences) {
            let (sp, sn) = skin_vertex(matrices, inf.bones, inf.weights, *p, *n);
            *p = sp;
            *n = sn;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::rig::{LocalTransform, RigNode};
    use crate::math::sin_cos;
    use alloc::vec;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn bind_pose_is_identity_and_bones_follow() {
        let mut rig = Rig::new();
        let mesh = rig.add_node(
            RigNode::new(1, None, LocalTransform { position: [5.0, 0.0, 0.0], ..LocalTransform::IDENTITY }),
            &[],
        );
        let bone = rig.add_node(
            RigNode::new(2, Some(mesh), LocalTransform { position: [0.0, 0.0, 10.0], ..LocalTransform::IDENTITY }),
            &[],
        );
        rig.update_world(&Affine::IDENTITY);
        let mut skin = Skin::new(mesh, vec![Some(bone), None]);
        skin.capture_bind(&rig.world);
        let mut m = Vec::new();
        skin.matrices(&rig.world, &mut m);
        for s in &m {
            assert!(close(s.t, [0.0; 3]) && close(s.cols[0], [1.0, 0.0, 0.0]));
        }
        // Rotate the bone 90 degrees about x: a vertex at the bone's origin + (0, 1, 0) goes to + (0, 0, 1).
        let (s, c) = sin_cos(core::f32::consts::FRAC_PI_2);
        rig.local[bone as usize].rotation = [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]];
        // Moving the whole character must not change the skinning matrices.
        rig.update_world(&Affine { cols: crate::math::MAT3_IDENTITY, t: [100.0, -3.0, 7.0] });
        skin.matrices(&rig.world, &mut m);
        let (p, n) = skin_vertex(&m, [0, 1, 0, 0], [1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 10.0], [0.0, 1.0, 0.0]);
        assert!(close(p, [0.0, 0.0, 11.0]), "{p:?}");
        assert!(close(n, [0.0, 0.0, 1.0]));
        // Half weights are not renormalised; all-zero weights leave the vertex in place.
        let (p, _) = skin_vertex(&m, [1, 1, 0, 0], [0.25, 0.25, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        assert!(close(p, [1.0, 0.0, 0.0]));
        let (p, _) = skin_vertex(&m, [0; 4], [0.0; 4], [2.0, 3.0, 4.0], [0.0, 0.0, 1.0]);
        assert_eq!(p, [2.0, 3.0, 4.0]);
        let rows = BoneMatrix::from_affine(&m[0]);
        assert_eq!(rows.to_affine(), m[0]);
        assert_eq!(core::mem::size_of::<BoneMatrix>(), 48);
    }

    #[test]
    fn poses_apply_before_skinning() {
        let sk = SkinnedSubmesh {
            positions: vec![[0.0; 3], [1.0, 0.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 2],
            influences: vec![SkinVertex { bones: [0; 4], weights: [1.0, 0.0, 0.0, 0.0] }; 2],
            poses: vec![vec![PoseDelta { vertex: 1, position: [0.0, 2.0, 0.0], normal: [0.0; 3] }]],
        };
        let shift = Affine { cols: crate::math::MAT3_IDENTITY, t: [0.0, 0.0, 5.0] };
        let (mut p, mut n) = (Vec::new(), Vec::new());
        sk.deform(&[shift], &[0.5], &mut p, &mut n);
        assert_eq!(p, [[0.0, 0.0, 5.0], [1.0, 1.0, 5.0]]);
        assert!(sk.is_skinned());
    }
}
