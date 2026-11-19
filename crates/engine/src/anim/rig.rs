//! A transform hierarchy that animations write into: local transforms, world transforms, twists and pose weights.
//!
//! The rig is independent of the game's object model. A character can own one, built from its subtree of the
//! object stream with [`Rig::from_stream`], or a caller can build one node by node. Nodes are stored parents first.

use alloc::vec::Vec;

use formats::anim::Event;
use formats::geometry::Affine;
use formats::stream::{Extension, ObjectStream, PoseWeightSet};

use super::playback::{AnimTarget, rotation_matrix};
use crate::math::{MAT3_IDENTITY, Mat3, Vec3, mat_mul, scale_columns};

/// A local transform: rotation, then scale along the local axes, then position (the scale channel of
/// animation.md 4 is applied along the object's local axes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LocalTransform {
    /// Position in the parent's space.
    pub position: Vec3,
    /// Rotation (columns), pre- and post-rotation included.
    pub rotation: Mat3,
    /// Scale along the local axes.
    pub scale: Vec3,
}

impl LocalTransform {
    /// The identity.
    pub const IDENTITY: LocalTransform =
        LocalTransform { position: [0.0; 3], rotation: MAT3_IDENTITY, scale: [1.0; 3] };

    /// From a stored transform (scale 1).
    pub fn from_affine(a: &Affine) -> LocalTransform {
        LocalTransform { position: a.t, rotation: a.cols, scale: [1.0; 3] }
    }

    /// As an affine transform `[R · diag(s), p]`.
    pub fn to_affine(&self) -> Affine {
        Affine { cols: scale_columns(&self.rotation, self.scale), t: self.position }
    }
}

/// An ancestor that replaces the parent for part of the world transform (extension bits 2 and 3, transforms.md 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BasisOverride {
    /// The ancestor node.
    pub node: u32,
    /// Take the rotation from it (bit 3).
    pub rotation: bool,
    /// Take the position from it (bit 2).
    pub position: bool,
}

/// One node of a rig.
#[derive(Clone, Debug, PartialEq)]
pub struct RigNode {
    /// Object id.
    pub oid: i16,
    /// The object-stream record the node came from, if any.
    pub record: Option<u32>,
    /// Parent node (always a lower index), or `None` for a node placed under the rig's base transform.
    pub parent: Option<u32>,
    /// Rest (stored) local transform.
    pub rest: LocalTransform,
    /// Pre-rotation (extension bit 0).
    pub pre: Option<Mat3>,
    /// Post-rotation (extension bit 1).
    pub post: Option<Mat3>,
    /// Ancestor override.
    pub basis: Option<BasisOverride>,
    /// Rest twist angle (extension bit 5), 0 without an IK chain.
    pub rest_twist: f32,
    /// First entry of this node's pose weights in [`Rig::pose_weights`].
    pub pose_offset: usize,
    /// Number of poses of the node's mesh set.
    pub pose_count: usize,
    /// The pose-weight sets the object carries (placed layout row 14); they drive this node's own mesh set.
    pub pose_sets: Vec<PoseWeightSet>,
    /// The selected set (index into `pose_sets`), whose weights the node's weights ease toward.
    pub pose_goal: Option<usize>,
}

impl RigNode {
    /// A node with a rest transform and nothing else.
    pub fn new(oid: i16, parent: Option<u32>, rest: LocalTransform) -> RigNode {
        RigNode {
            oid,
            record: None,
            parent,
            rest,
            pre: None,
            post: None,
            basis: None,
            rest_twist: 0.0,
            pose_offset: 0,
            pose_count: 0,
            pose_sets: Vec::new(),
            pose_goal: None,
        }
    }
}

/// The hierarchy and its current state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rig {
    /// Nodes, parents first.
    pub nodes: Vec<RigNode>,
    /// Current local transforms.
    pub local: Vec<LocalTransform>,
    /// World transforms from the last [`Rig::update_world`].
    pub world: Vec<Affine>,
    /// Current twist angles.
    pub twist: Vec<f32>,
    /// Current pose weights of every node, at each node's `pose_offset`.
    pub pose_weights: Vec<f32>,
    rest_pose_weights: Vec<f32>,
}

/// The twist angle stored in an extension's IK chain (bit 5), if any.
fn ik_twist(ext: &Extension, data: &[u8]) -> Option<f32> {
    if ext.flags & 0x20 == 0 {
        return None;
    }
    let mut at = ext.offset + 4;
    at += if ext.flags & 0x01 != 0 { 36 } else { 0 };
    at += if ext.flags & 0x02 != 0 { 36 } else { 0 };
    at += if ext.flags & 0x0C != 0 { 2 } else { 0 };
    at += if ext.flags & 0x10 != 0 { 2 } else { 0 };
    // Elbow OID, two vectors, then the twist.
    formats::reader::le_f32(data, at + 2 + 24).ok()
}

impl Rig {
    /// An empty rig.
    pub fn new() -> Rig {
        Rig::default()
    }

    /// Adds a node with `pose_weights` initial pose weights; returns its index. The parent must already exist.
    pub fn add_node(&mut self, mut node: RigNode, pose_weights: &[f32]) -> u32 {
        let idx = self.nodes.len() as u32;
        debug_assert!(node.parent.is_none_or(|p| p < idx));
        if node.parent.is_some_and(|p| p >= idx) {
            node.parent = None;
        }
        node.pose_offset = self.pose_weights.len();
        node.pose_count = pose_weights.len();
        self.pose_weights.extend_from_slice(pose_weights);
        self.rest_pose_weights.extend_from_slice(pose_weights);
        self.local.push(node.rest);
        self.twist.push(node.rest_twist);
        self.world.push(Affine::IDENTITY);
        self.nodes.push(node);
        idx
    }

    /// Builds a rig from the records `root .. subtree_end` of an object stream (one node per record, in stream
    /// order, so node `i` is record `root + i`). Records without a stored transform get the identity. A node's
    /// parent is its record's transform parent when that is inside the range; the rest hang under the base
    /// transform passed to [`Rig::update_world`] (the world transform of `root`'s parent). Pre- and post-rotations,
    /// ancestor overrides inside the range, IK twists and initial pose weights are taken from the records.
    pub fn from_stream(stream: &ObjectStream, data: &[u8], root: u32) -> Rig {
        let mut rig = Rig::new();
        let end = stream.records[root as usize].subtree_end;
        for i in root..end {
            let r = &stream.records[i as usize];
            let parent = r.transform_parent.filter(|&p| p >= root && p < i).map(|p| p - root);
            let mut node = RigNode::new(r.oid, parent, LocalTransform::from_affine(&r.local_transform()));
            node.record = Some(i);
            if let Some(ext) = r.extension {
                node.pre = ext.pre_rotation(data);
                node.post = ext.post_rotation(data);
                node.rest_twist = ik_twist(&ext, data).unwrap_or(0.0);
                if let Some(oid) = ext.basis
                    && let Some(b) = stream.find_ancestor(i, oid).filter(|&b| b >= root)
                {
                    node.basis = Some(BasisOverride {
                        node: b - root,
                        rotation: ext.flags & Extension::ROTATION_FROM_BASIS != 0,
                        position: ext.flags & Extension::POSITION_FROM_BASIS != 0,
                    });
                }
            }
            node.pose_sets = r.pose_weight_sets(data).unwrap_or_default();
            let weights = r.mesh_set.as_ref().map_or(&[][..], |s| &s.pose_weights[..]);
            rig.add_node(node, weights);
        }
        rig
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the rig has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// The node made from object-stream record `record`.
    pub fn node_of_record(&self, record: u32) -> Option<u32> {
        let first = self.nodes.first()?.record?;
        let i = record.checked_sub(first)?;
        (self.nodes.get(i as usize)?.record == Some(record)).then_some(i)
    }

    /// The first node with OID `oid`.
    pub fn find_oid(&self, oid: i16) -> Option<u32> {
        self.nodes.iter().position(|n| n.oid == oid).map(|i| i as u32)
    }

    /// Puts every local transform, twist and pose weight back to its rest value.
    pub fn reset_to_rest(&mut self) {
        for (l, n) in self.local.iter_mut().zip(&self.nodes) {
            *l = n.rest;
        }
        for (t, n) in self.twist.iter_mut().zip(&self.nodes) {
            *t = n.rest_twist;
        }
        self.pose_weights.copy_from_slice(&self.rest_pose_weights);
    }

    /// Selects the pose-weight set named `name` on `node` (event 45 or option 116, animation.md 8.3); −1, or a name
    /// the node does not carry, releases the weights. Returns whether a set was selected.
    pub fn select_pose_set(&mut self, node: u32, name: i16) -> bool {
        let Some(n) = self.nodes.get_mut(node as usize) else { return false };
        n.pose_goal = if name == -1 { None } else { n.pose_sets.iter().position(|s| s.name == name) };
        n.pose_goal.is_some()
    }

    /// Moves the weights of every node with a selected set toward that set's values, by at most `rate × dt` per
    /// weight. The spec says the weights ease toward the set without giving the law; the linear rate is the port's
    /// choice (the caller passes it).
    pub fn update_pose_sets(&mut self, dt: f32, rate: f32) {
        let step = rate * dt;
        for n in &self.nodes {
            let Some(g) = n.pose_goal else { continue };
            let goal = &n.pose_sets[g].weights;
            let w = &mut self.pose_weights[n.pose_offset..n.pose_offset + n.pose_count];
            for (w, &t) in w.iter_mut().zip(goal) {
                *w += (t - *w).clamp(-step, step);
            }
        }
    }

    /// The current pose weights of `node`.
    pub fn node_pose_weights(&self, node: u32) -> &[f32] {
        let n = &self.nodes[node as usize];
        &self.pose_weights[n.pose_offset..n.pose_offset + n.pose_count]
    }

    /// Recomputes every world transform: `W = W_parent · L` (transforms.md 2), with `base` as the parent of nodes
    /// without one, and the ancestor overrides applied.
    pub fn update_world(&mut self, base: &Affine) {
        for i in 0..self.nodes.len() {
            let n = &self.nodes[i];
            let l = self.local[i].to_affine();
            let parent = n.parent.map_or(*base, |p| self.world[p as usize]);
            let mut w = parent.mul(&l);
            if let Some(b) = n.basis {
                let bw = self.world[b.node as usize];
                if b.rotation {
                    w.cols = mat_mul(&bw.cols, &l.cols);
                }
                if b.position {
                    w.t = bw.transform_point(l.t);
                }
            }
            self.world[i] = w;
        }
    }
}

/// The (target OID, set name) of a pose-combination event (type 45: two `s16`).
pub fn pose_set_event(ev: &Event) -> Option<(i16, i16)> {
    (ev.kind == 45).then(|| Some((ev.i16_at(0)?, ev.i16_at(2)?))).flatten()
}

impl AnimTarget for Rig {
    fn set_position(&mut self, node: u32, position: Vec3) {
        if let Some(l) = self.local.get_mut(node as usize) {
            l.position = position;
        }
    }

    fn set_rotation(&mut self, node: u32, euler: Vec3) {
        if let Some(n) = self.nodes.get(node as usize) {
            self.local[node as usize].rotation = rotation_matrix(euler, n.pre.as_ref(), n.post.as_ref());
        }
    }

    fn set_scale(&mut self, node: u32, scale: Vec3) {
        if let Some(l) = self.local.get_mut(node as usize) {
            l.scale = scale;
        }
    }

    fn set_twist(&mut self, node: u32, twist: f32) {
        if let Some(t) = self.twist.get_mut(node as usize) {
            *t = twist;
        }
    }

    fn set_pose_weight(&mut self, node: u32, pose: usize, weight: f32) {
        if let Some(n) = self.nodes.get(node as usize)
            && pose < n.pose_count
        {
            self.pose_weights[n.pose_offset + pose] = weight;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::sin_cos;

    #[test]
    fn world_transforms_compose_with_scale_and_basis() {
        let mut rig = Rig::new();
        let (s, c) = sin_cos(core::f32::consts::FRAC_PI_2);
        let rz = [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]];
        let root = rig.add_node(
            RigNode::new(1, None, LocalTransform { position: [10.0, 0.0, 0.0], rotation: rz, scale: [1.0; 3] }),
            &[],
        );
        let child = rig.add_node(RigNode::new(2, Some(root), LocalTransform::IDENTITY), &[0.5, 0.25]);
        let mut n = RigNode::new(3, Some(child), LocalTransform::IDENTITY);
        n.basis = Some(BasisOverride { node: root, rotation: true, position: false });
        n.rest.position = [1.0, 0.0, 0.0];
        rig.add_node(n, &[]);
        rig.set_position(child, [0.0, 2.0, 0.0]);
        rig.set_scale(child, [2.0, 2.0, 2.0]);
        rig.update_world(&Affine::IDENTITY);
        let w = rig.world[child as usize];
        assert!((w.t[0] - 8.0).abs() < 1e-5 && w.t[1].abs() < 1e-5);
        assert!((w.cols[0][1] - 2.0).abs() < 1e-5);
        // The basis node takes the root's rotation (no scale) but the child's position chain.
        let g = rig.world[2];
        assert!((g.cols[0][1] - 1.0).abs() < 1e-5);
        assert!((g.t[0] - 8.0).abs() < 1e-5 && (g.t[1] - 2.0).abs() < 1e-5);
        rig.set_pose_weight(child, 1, 0.9);
        rig.set_pose_weight(child, 5, 0.9);
        assert_eq!(rig.node_pose_weights(child), &[0.5, 0.9]);
        rig.reset_to_rest();
        assert_eq!(rig.node_pose_weights(child), &[0.5, 0.25]);
        assert_eq!(rig.local[child as usize], LocalTransform::IDENTITY);
        // Pose counts beyond the old 16-bit range keep their size.
        let many = alloc::vec![0.5f32; 70_000];
        let big = rig.add_node(RigNode::new(4, None, LocalTransform::IDENTITY), &many);
        assert_eq!(rig.node_pose_weights(big).len(), 70_000);
        rig.set_pose_weight(big, 69_999, 1.0);
        assert_eq!(rig.node_pose_weights(big)[69_999], 1.0);
        // Pose-weight sets: select by name, ease toward it, release with -1.
        rig.nodes[child as usize].pose_sets = alloc::vec![PoseWeightSet { name: 77, weights: alloc::vec![1.0, 0.0] }];
        assert!(rig.select_pose_set(child, 77));
        rig.update_pose_sets(0.25, 1.0);
        assert_eq!(rig.node_pose_weights(child), &[0.75, 0.0]);
        rig.update_pose_sets(1.0, 1.0);
        assert_eq!(rig.node_pose_weights(child), &[1.0, 0.0]);
        assert!(!rig.select_pose_set(child, -1));
        rig.update_pose_sets(1.0, 1.0);
        assert_eq!(rig.node_pose_weights(child), &[1.0, 0.0]);
        let ev = Event { kind: 45, frame: None, offset: 0, payload: alloc::vec![5, 0, 77, 0] };
        assert_eq!(pose_set_event(&ev), Some((5, 77)));
    }

    #[test]
    fn rotation_uses_pre_and_post() {
        let mut rig = Rig::new();
        let swap = [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
        let mut n = RigNode::new(1, None, LocalTransform::IDENTITY);
        n.pre = Some(swap);
        n.post = Some(swap);
        rig.add_node(n, &[]);
        rig.set_rotation(0, [0.0; 3]);
        assert_eq!(rig.local[0].rotation, mat_mul(&swap, &swap));
    }
}
