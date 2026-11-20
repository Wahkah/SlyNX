//! Sly 2 and Sly 3 transform-list rigs (specs/sly2/animation.md 4 to 6, transforms.md 2; Sly 3 uses the same lists
//! and rules, specs/sly3/animation.md 1): channels that replace part of a
//! transform's local matrix by the transform's kind, mirror pairs and two-bone chains.
//!
//! Order each frame (animation.md 6): vector channels, then mirror pairs, then chains.

use alloc::vec::Vec;

use formats::anim::{ScalarCurve, VectorCurve};
use formats::geometry::Affine;
use formats_sly2::anim::Clip;
use formats_sly2::transforms::{TransformList, TransformRef};
use formats_sly3::anim::ClipData;

use super::curve::{eval_scalar, eval_vector, sample_vector};
use crate::math::{
    MAT3_IDENTITY, Mat3, Vec3, acos, affine_inverse, axis_angle, cross, dot, length, mat_mul, normalize, scale, sub,
    transpose,
};

/// Transform kinds (transforms.md 2).
pub mod kind {
    /// Not animated.
    pub const STATIC: i16 = 0;
    /// Translation.
    pub const TRANSLATION: i16 = 1;
    /// Rotation from Euler angles.
    pub const EULER: i16 = 6;
    /// Scale (the diagonal of the rotation part).
    pub const SCALE: i16 = 12;
}

/// One transform of a rig.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XformNode {
    /// Parent transform (always earlier), or `None` for the rig's root frame.
    pub parent: Option<usize>,
    /// Kind.
    pub kind: i16,
    /// ID (the `s32` stored with it).
    pub id: i32,
    /// Loaded (bind) local matrix.
    pub bind: Affine,
}

/// A two-bone chain with its load-time measurements (animation.md 6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChainRig {
    /// Upper, lower and end transforms.
    pub upper: usize,
    /// Lower transform.
    pub lower: usize,
    /// End transform (carries the effector).
    pub end: usize,
    /// Effector offset in the end transform's frame.
    pub effector: Vec3,
    /// Handle transform.
    pub handle: usize,
    /// Target offset in the handle's frame.
    pub target: Vec3,
    /// Upper length (upper to lower).
    pub a: f32,
    /// Lower length (lower to effector).
    pub b: f32,
    /// Bind bend angle between the upper and lower directions.
    pub bind_bend: f32,
    /// Bend axis in the upper bone's frame.
    pub axis_upper: Vec3,
}

/// A mirror pair (animation.md 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MirrorRig {
    /// Source transform.
    pub source: usize,
    /// Target transform.
    pub target: usize,
    /// Copy the position too.
    pub position: bool,
    /// Copy the rotation.
    pub rotation: bool,
}

/// A transform-list rig and its current state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XformRig {
    /// Transforms.
    pub nodes: Vec<XformNode>,
    /// Current local matrices.
    pub local: Vec<Affine>,
    /// World matrices from the last update.
    pub world: Vec<Affine>,
    /// Swivel angle per transform (the scalar channels of chain upper bones).
    pub swivel: Vec<f32>,
    /// Chains.
    pub chains: Vec<ChainRig>,
    /// Mirror pairs.
    pub mirrors: Vec<MirrorRig>,
}

/// `x` brought into `[−π, π]` (Sly 2 pose hook: angle blends).
fn rem_pi(x: f32) -> f32 {
    use core::f32::consts::PI;
    crate::math::rem_euclid(x + PI, 2.0 * PI) - PI
}

/// The rotation turning unit vector `a` onto unit vector `b`.
fn rotation_between(a: Vec3, b: Vec3) -> Mat3 {
    let c = cross(a, b);
    let s = length(c);
    let d = dot(a, b).clamp(-1.0, 1.0);
    if s < 1e-7 {
        if d > 0.0 {
            return MAT3_IDENTITY;
        }
        // Opposite: half a turn about any perpendicular axis.
        let p = if a[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        return axis_angle(scale(normalize(cross(a, p)), core::f32::consts::PI));
    }
    axis_angle(scale(c, crate::math::atan2(s, d) / s))
}

impl XformRig {
    /// From a transform list. References to the enclosing object or the owner, and −1, hang from the root frame.
    pub fn from_list(list: &TransformList) -> XformRig {
        let n = list.transforms.len();
        let idx = |r: i16| match TransformRef::decode(r) {
            TransformRef::Own(i) if (i as usize) < n => Some(i as usize),
            _ => None,
        };
        let nodes: Vec<XformNode> = list
            .transforms
            .iter()
            .enumerate()
            .map(|(i, t)| XformNode { parent: idx(t.parent).filter(|&p| p < i), kind: t.kind, id: t.id, bind: t.local })
            .collect();
        let mut rig = XformRig {
            local: nodes.iter().map(|n| n.bind).collect(),
            world: alloc::vec![Affine::IDENTITY; n],
            swivel: alloc::vec![0.0; n],
            nodes,
            chains: Vec::new(),
            mirrors: Vec::new(),
        };
        rig.compute_world(&Affine::IDENTITY);
        for c in &list.chains {
            let (Some(upper), Some(lower), Some(end), Some(handle)) =
                (idx(c.upper), idx(c.lower), idx(c.end), idx(c.handle))
            else {
                continue;
            };
            rig.chains.push(rig.measure(upper, lower, end, c.effector, handle, c.target));
        }
        for m in &list.mirrors {
            if let (Some(source), Some(target)) = (idx(m.source), idx(m.target)) {
                rig.mirrors.push(MirrorRig { source, target, position: m.copy_position, rotation: m.copy_rotation });
            }
        }
        rig
    }

    /// Adds a transform (for rigs built by hand); returns its index.
    pub fn push(&mut self, node: XformNode) -> usize {
        self.local.push(node.bind);
        self.world.push(Affine::IDENTITY);
        self.swivel.push(0.0);
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// Measures a chain in the current (bind) pose and returns its set-up (step 1 of animation.md 6).
    pub fn measure(
        &self,
        upper: usize,
        lower: usize,
        end: usize,
        effector: Vec3,
        handle: usize,
        target: Vec3,
    ) -> ChainRig {
        let pu = self.world[upper].t;
        let pl = self.world[lower].t;
        let pe = self.world[end].transform_point(effector);
        let (u, l) = (sub(pl, pu), sub(pe, pl));
        let (a, b) = (length(u), length(l));
        // Degenerate bones (animation.md 6 step 1): angle 0 and the axis (1, 0, 0) in the upper bone's frame.
        let (bind_bend, axis_upper) = if a < 1e-4 || b < 1e-4 {
            (0.0, [1.0, 0.0, 0.0])
        } else {
            let n = cross(u, l);
            let axis = if dot(n, n) <= 1e-8 { perpendicular_axis(u) } else { normalize(n) };
            (acos(dot(normalize(u), normalize(l))), crate::math::mat_mul_vec(&transpose(&self.world[upper].cols), axis))
        };
        ChainRig { upper, lower, end, effector, handle, target, a, b, bind_bend, axis_upper }
    }

    /// Puts every local matrix and swivel back to the loaded pose.
    pub fn reset(&mut self) {
        for (l, n) in self.local.iter_mut().zip(&self.nodes) {
            *l = n.bind;
        }
        self.swivel.iter_mut().for_each(|s| *s = 0.0);
    }

    /// Writes a vector value into transform `i` by its kind (animation.md 4): kind 1 replaces the translation,
    /// kind 6 the rotation part (Rz · Ry · Rx of Euler angles), kind 12 the diagonal of the rotation part. Other
    /// kinds are left alone.
    pub fn set_vector(&mut self, i: usize, v: Vec3) {
        let Some(n) = self.nodes.get(i) else { return };
        let l = &mut self.local[i];
        match n.kind {
            kind::TRANSLATION => l.t = v,
            kind::EULER => l.cols = super::curve::euler_zyx(v),
            kind::SCALE => {
                l.cols[0][0] = v[0];
                l.cols[1][1] = v[1];
                l.cols[2][2] = v[2];
            }
            _ => {}
        }
    }

    /// Applies channels at time `t`, each `(transform, vector curve, scalar curve)`: a vector by the transform's
    /// kind, a scalar as the swivel of its transform. With `scalar_wins` (Sly 3, animation.md 2) a channel with both
    /// tracks uses only the scalar one.
    pub fn apply_channels<'a>(
        &mut self,
        channels: impl IntoIterator<Item = (i16, Option<&'a VectorCurve>, Option<&'a ScalarCurve>)>,
        t: f32,
        scalar_wins: bool,
    ) {
        for (transform, vector, scalar) in channels {
            let i = transform as usize;
            if !(scalar_wins && scalar.is_some())
                && let Some(v) = vector.and_then(|v| eval_vector(v, t))
            {
                self.set_vector(i, v);
            }
            if let Some(s) = scalar.and_then(|s| eval_scalar(s, t))
                && let Some(w) = self.swivel.get_mut(i)
            {
                *w = s;
            }
        }
    }

    /// Sly 2 pose hook (gait blends, characters.md 1.1): applies several members' channels at once, each
    /// `(channels, t, weight)`, writing `Σ wᵢ · valueᵢ / Σ wᵢ` per transform (vectors by kind, scalars as swivels).
    /// Members are evaluated in order; a transform keeps its current value where no member has a channel. Euler
    /// angles of later members are taken within π of the first member's, so the sum does not cross a wrap.
    pub fn blend_channels<'a, I>(&mut self, members: impl IntoIterator<Item = (I, f32, f32)>, scalar_wins: bool)
    where
        I: IntoIterator<Item = (i16, Option<&'a VectorCurve>, Option<&'a ScalarCurve>)>,
    {
        // Per transform: Σ w·v, Σ w and the first member's value; Σ w·s and Σ w.
        let n = self.nodes.len();
        let mut vectors: Vec<(Vec3, f32, Vec3)> = alloc::vec![([0.0; 3], 0.0, [0.0; 3]); n];
        let mut scalars: Vec<(f32, f32)> = alloc::vec![(0.0, 0.0); n];
        for (channels, t, w) in members {
            if w <= 0.0 {
                continue;
            }
            for (transform, vector, scalar) in channels {
                let i = transform as usize;
                if i >= n {
                    continue;
                }
                if !(scalar_wins && scalar.is_some())
                    && let Some(mut v) = vector.and_then(|v| eval_vector(v, t))
                {
                    let e = &mut vectors[i];
                    if e.1 == 0.0 {
                        e.2 = v;
                    } else if self.nodes[i].kind == kind::EULER {
                        for k in 0..3 {
                            v[k] = e.2[k] + rem_pi(v[k] - e.2[k]);
                        }
                    }
                    e.0 = crate::math::madd(e.0, v, w);
                    e.1 += w;
                }
                if let Some(s) = scalar.and_then(|s| eval_scalar(s, t)) {
                    scalars[i].0 += w * s;
                    scalars[i].1 += w;
                }
            }
        }
        for (i, &(sum, w, _)) in vectors.iter().enumerate() {
            if w > 0.0 {
                self.set_vector(i, scale(sum, 1.0 / w));
            }
        }
        for (i, &(sum, w)) in scalars.iter().enumerate() {
            if w > 0.0 {
                self.swivel[i] = sum / w;
            }
        }
    }

    /// Sly 2 pose hook: the channels of a Sly 2 clip that target `object`, in the form [`Self::blend_channels`]
    /// takes.
    pub fn clip_channels(
        clip: &Clip,
        object: i32,
    ) -> impl Iterator<Item = (i16, Option<&VectorCurve>, Option<&ScalarCurve>)> {
        clip.channels
            .iter()
            .filter(move |c| c.target == object)
            .map(|c| (c.transform, c.vector.as_ref(), c.scalar.as_ref()))
    }

    /// Sly 3 pose hook: the channels of a Sly 3 clip that target `object`, for [`Self::blend_channels`].
    pub fn sly3_clip_channels(
        clip: &ClipData,
        object: i32,
    ) -> impl Iterator<Item = (i16, Option<&VectorCurve>, Option<&ScalarCurve>)> {
        clip.channels
            .iter()
            .filter(move |c| c.target == object)
            .map(|c| (c.transform, c.vector.as_ref(), c.scalar.as_ref()))
    }

    /// Applies a Sly 2 clip at time `t`: the channels whose target is `object` (the character's object ID).
    pub fn apply_clip(&mut self, clip: &Clip, object: i32, t: f32) {
        let chs = clip.channels.iter().filter(|c| c.target == object);
        self.apply_channels(chs.map(|c| (c.transform, c.vector.as_ref(), c.scalar.as_ref())), t, false);
    }

    /// Applies a Sly 3 clip at time `t` (specs/sly3/animation.md: Sly 2's channels, the scalar track winning when a
    /// channel has both).
    pub fn apply_sly3_clip(&mut self, clip: &ClipData, object: i32, t: f32) {
        let chs = clip.channels.iter().filter(|c| c.target == object);
        self.apply_channels(chs.map(|c| (c.transform, c.vector.as_ref(), c.scalar.as_ref())), t, true);
    }

    /// The root-motion velocity of a clip at `t`: the rate of transform 1's translation channel (animation.md 5),
    /// in the character's frame.
    pub fn root_velocity(clip: &Clip, object: i32, t: f32) -> Option<Vec3> {
        let ch = clip.channels.iter().find(|c| c.target == object && c.transform == 1)?;
        Some(sample_vector(ch.vector.as_ref()?, t)?.rate)
    }

    /// The root-motion velocity of a Sly 3 clip at `t` (transform 1, as Sly 2).
    pub fn root_velocity_sly3(clip: &ClipData, object: i32, t: f32) -> Option<Vec3> {
        let ch = clip.channels.iter().find(|c| c.target == object && c.transform == 1)?;
        Some(sample_vector(ch.vector.as_ref()?, t)?.rate)
    }

    /// World matrices from the local ones (`root` for transforms without a parent).
    pub fn compute_world(&mut self, root: &Affine) {
        for i in 0..self.nodes.len() {
            let p = self.nodes[i].parent.map_or(*root, |p| self.world[p]);
            self.world[i] = p.mul(&self.local[i]);
        }
    }

    /// Sets transform `i`'s world matrix by changing its local one.
    fn set_world(&mut self, i: usize, w: Affine, root: &Affine) {
        let p = self.nodes[i].parent.map_or(*root, |p| self.world[p]);
        if let Some(inv) = affine_inverse(&p) {
            self.local[i] = inv.mul(&w);
        }
    }

    /// The full update after the channels: world matrices, mirror pairs, chains.
    pub fn update(&mut self, root: &Affine) {
        self.compute_world(root);
        for k in 0..self.mirrors.len() {
            let m = self.mirrors[k];
            let mut w = self.world[m.target];
            if m.rotation {
                w.cols = self.world[m.source].cols;
            }
            if m.position {
                w.t = self.world[m.source].t;
            }
            self.set_world(m.target, w, root);
            self.compute_world(root);
        }
        for k in 0..self.chains.len() {
            self.solve_chain(k, root);
        }
    }

    /// The goal of chain `k`: the handle's world position plus the target offset in the handle's frame.
    pub fn chain_goal(&self, k: usize) -> Vec3 {
        let c = &self.chains[k];
        self.world[c.handle].transform_point(c.target)
    }

    /// The effector of chain `k` in world space.
    pub fn chain_effector(&self, k: usize) -> Vec3 {
        let c = &self.chains[k];
        self.world[c.end].transform_point(c.effector)
    }

    /// Steps 2 to 4 of animation.md 6 for chain `k`.
    fn solve_chain(&mut self, k: usize, root: &Affine) {
        let c = self.chains[k];
        // 2. Bind rotations for the two bones (their channels, if any, are replaced), and the goal.
        self.local[c.upper].cols = self.nodes[c.upper].bind.cols;
        self.local[c.lower].cols = self.nodes[c.lower].bind.cols;
        self.compute_world(root);
        let goal = self.chain_goal(k);
        let pu = self.world[c.upper].t;
        // 3. Bend the lower bone about the bend axis.
        let d = length(sub(goal, pu)).clamp((c.a - c.b).abs() + 1e-4, c.a + c.b - 1e-4);
        let bend = acos((d * d - c.a * c.a - c.b * c.b) / (2.0 * c.a * c.b));
        let axis = normalize(crate::math::mat_mul_vec(&self.world[c.upper].cols, c.axis_upper));
        let r = axis_angle(scale(axis, bend - c.bind_bend));
        let wl = self.world[c.lower];
        self.set_world(c.lower, Affine { cols: mat_mul(&r, &wl.cols), t: wl.t }, root);
        self.compute_world(root);
        // 4. Aim the upper bone so the effector points at the goal, then spin it by the swivel.
        let eff = self.chain_effector(k);
        let r = rotation_between(normalize(sub(eff, pu)), normalize(sub(goal, pu)));
        let spin = axis_angle(scale(normalize(sub(goal, pu)), self.swivel[c.upper]));
        let wu = self.world[c.upper];
        let cols = mat_mul(&spin, &mat_mul(&r, &wu.cols));
        self.set_world(c.upper, Affine { cols, t: wu.t }, root);
        self.compute_world(root);
    }
}

/// The bend axis of a straight limb (animation.md 6, the routine at 0x0011E390): from u's smallest component pick a
/// coordinate axis e (if |x| ≤ |y|: X when |x| ≤ |z|, else Z; if |x| > |y|: Y when |y| ≤ |z|, else Z), take
/// p = u × e for X or Z and p = e × u for Y, and return normalise(u × p).
pub fn perpendicular_axis(u: Vec3) -> Vec3 {
    let [x, y, z] = [u[0].abs(), u[1].abs(), u[2].abs()];
    let (e, swap) = if x <= y {
        if x <= z { ([1.0, 0.0, 0.0], false) } else { ([0.0, 0.0, 1.0], false) }
    } else if y <= z {
        ([0.0, 1.0, 0.0], true)
    } else {
        ([0.0, 0.0, 1.0], false)
    };
    let p = if swap { cross(e, u) } else { cross(u, e) };
    normalize(cross(u, p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(parent: Option<usize>, kind: i16, t: Vec3) -> XformNode {
        XformNode { parent, kind, id: 0, bind: Affine { cols: MAT3_IDENTITY, t } }
    }

    /// Root, hip, knee (100 below), foot (100 below), handle.
    fn leg() -> XformRig {
        let mut r = XformRig::default();
        r.push(node(None, kind::STATIC, [0.0; 3]));
        r.push(node(Some(0), kind::EULER, [0.0, 0.0, 200.0]));
        r.push(node(Some(1), kind::STATIC, [0.0, 0.0, -100.0]));
        r.push(node(Some(2), kind::STATIC, [0.0, 0.0, -100.0]));
        r.push(node(Some(0), kind::TRANSLATION, [0.0, 0.0, 0.0]));
        r.compute_world(&Affine::IDENTITY);
        // Bent slightly forward at load so the bend plane is defined.
        r.local[2].t = [5.0, 0.0, -100.0];
        r.nodes[2].bind.t = [5.0, 0.0, -100.0];
        r.compute_world(&Affine::IDENTITY);
        let c = r.measure(1, 2, 3, [0.0; 3], 4, [0.0; 3]);
        r.chains.push(c);
        r
    }

    #[test]
    fn blends_weigh_members_and_unwrap_angles() {
        // Sly 2 pose hook: gait blends.
        let mut r = XformRig::default();
        let t = r.push(node(None, kind::TRANSLATION, [0.0; 3]));
        let e = r.push(node(Some(0), kind::EULER, [0.0; 3]));
        let c = |v: Vec3| VectorCurve::Components { mask: 0, base: v, curves: [None, None, None] };
        let (a0, a1) = (c([0.0, 0.0, 100.0]), c([40.0, 0.0, 100.0]));
        // 3.0 and −3.0 rad are 0.28 apart across the wrap: their mean is π, not 0.
        let (b0, b1) = (c([3.0, 0.0, 0.0]), c([-3.0, 0.0, 0.0]));
        let m0 = alloc::vec![(t as i16, Some(&a0), None), (e as i16, Some(&b0), None)];
        let m1 = alloc::vec![(t as i16, Some(&a1), None), (e as i16, Some(&b1), None)];
        r.blend_channels([(m0, 0.0, 0.75), (m1, 0.0, 0.25)], false);
        assert!((r.local[t].t[0] - 10.0).abs() < 1e-5, "{:?}", r.local[t].t);
        let want = super::super::curve::euler_zyx([3.0 * 0.75 + (2.0 * core::f32::consts::PI - 3.0) * 0.25, 0.0, 0.0]);
        for (x, y) in r.local[e].cols.iter().flatten().zip(want.iter().flatten()) {
            assert!((x - y).abs() < 1e-4);
        }
    }

    #[test]
    fn kinds_replace_parts_of_the_local_matrix() {
        let mut r = XformRig::default();
        let t = r.push(node(None, kind::TRANSLATION, [1.0, 2.0, 3.0]));
        let e = r.push(node(None, kind::EULER, [1.0, 2.0, 3.0]));
        let s = r.push(node(None, kind::SCALE, [1.0, 2.0, 3.0]));
        r.set_vector(t, [9.0, 8.0, 7.0]);
        r.set_vector(e, [0.0, 0.0, core::f32::consts::FRAC_PI_2]);
        r.set_vector(s, [2.0, 3.0, 4.0]);
        assert_eq!(r.local[t].t, [9.0, 8.0, 7.0]);
        assert_eq!(r.local[e].t, [1.0, 2.0, 3.0]);
        assert!((r.local[e].cols[0][1] - 1.0).abs() < 1e-6);
        assert_eq!((r.local[s].cols[0][0], r.local[s].cols[1][1], r.local[s].cols[2][2]), (2.0, 3.0, 4.0));
    }

    #[test]
    fn chain_reaches_its_goal_and_keeps_bone_lengths() {
        let mut r = leg();
        for goal in [[30.0, 0.0, 50.0], [0.0, 60.0, 80.0], [50.0, -40.0, 20.0]] {
            r.reset();
            r.local[4].t = goal;
            r.update(&Affine::IDENTITY);
            let eff = r.chain_effector(0);
            assert!(length(sub(eff, goal)) < 1e-2, "goal {goal:?}: effector {eff:?}");
            let (pu, pl) = (r.world[1].t, r.world[2].t);
            assert!((length(sub(pl, pu)) - r.chains[0].a).abs() < 1e-3);
            assert!((length(sub(eff, pl)) - r.chains[0].b).abs() < 1e-3);
        }
        // Out of reach: the limb points straight at the goal.
        r.reset();
        r.local[4].t = [0.0, 0.0, -500.0];
        r.update(&Affine::IDENTITY);
        let eff = r.chain_effector(0);
        let dir = normalize(sub(eff, r.world[1].t));
        assert!(dot(dir, [0.0, 0.0, -1.0]) > 0.999);
        // The swivel turns the knee about the hip-to-goal axis but keeps the foot on the goal.
        r.reset();
        r.local[4].t = [30.0, 0.0, 50.0];
        r.update(&Affine::IDENTITY);
        let knee0 = r.world[2].t;
        r.reset();
        r.local[4].t = [30.0, 0.0, 50.0];
        r.swivel[1] = 1.0;
        r.update(&Affine::IDENTITY);
        assert!(length(sub(r.world[2].t, knee0)) > 1.0);
        assert!(length(sub(r.chain_effector(0), [30.0, 0.0, 50.0])) < 1e-2);
    }

    #[test]
    fn mirror_copies_world_rotation() {
        let mut r = XformRig::default();
        let a = r.push(node(None, kind::EULER, [0.0; 3]));
        let b = r.push(node(None, kind::STATIC, [5.0, 0.0, 0.0]));
        r.mirrors.push(MirrorRig { source: a, target: b, position: false, rotation: true });
        r.set_vector(a, [0.3, 0.2, 0.1]);
        r.update(&Affine { cols: MAT3_IDENTITY, t: [1.0, 1.0, 1.0] });
        assert!(
            r.world[a].cols.iter().flatten().zip(r.world[b].cols.iter().flatten()).all(|(x, y)| (x - y).abs() < 1e-5)
        );
        assert_eq!(r.world[b].t, [6.0, 1.0, 1.0]);
    }

    #[test]
    fn straight_limb_axis() {
        // animation.md 6 worked values.
        assert_eq!(perpendicular_axis([0.0, 0.0, 1.0]), [-1.0, 0.0, 0.0]);
        assert_eq!(perpendicular_axis([0.0, 1.0, 0.0]), [-1.0, 0.0, 0.0]);
        assert_eq!(perpendicular_axis([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]);
    }
}
