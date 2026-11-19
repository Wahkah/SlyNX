//! Posing a Sly 2 object: channel values applied by transform kind, mirror pairs, two-bone chains, and skin
//! matrices (animation.md 4 to 6, transforms.md 4).
//!
//! Evaluate a clip's curves with `engine::anim::curve` (they are `formats::anim` curves, see [`crate::anim`]), then:
//!
//! ```ignore
//! rig.reset();
//! for ch in clip.channels.iter().filter(|c| c.target == object_id) {
//!     if let Some(v) = ch.vector.as_ref().and_then(|v| eval_vector(v, t)) { rig.set_vector(ch.transform as usize, v); }
//!     if let Some(s) = ch.scalar.as_ref().and_then(|s| eval_scalar(s, t)) { rig.set_swivel(ch.transform as usize, s); }
//! }
//! rig.update(&root, &enclosing);   // channels -> mirror pairs -> chains (animation.md 6)
//! ```
//!
//! Euler angles of kind-6 transforms become `Affine::cols = Rz(z) · Ry(y) · Rx(x)` (columns: the images of the
//! axes), the same convention as `engine::anim::curve::euler_zyx`. Checked on the disc: constant rotation channels
//! reproduce the loaded rest rotations in this form and never only in the transposed one.

use alloc::vec::Vec;

use formats::geometry::Affine;

use crate::transforms::{Chain, Mirror, Transform, TransformList, TransformRef};

type Vec3 = [f32; 3];
type Mat3 = [Vec3; 3];

/// Transform kinds (transforms.md 2, animation.md 4).
pub const KIND_TRANSLATION: i16 = 1;
pub const KIND_ROTATION: i16 = 6;
pub const KIND_SCALE: i16 = 12;

// ---------------------------------------------------------------------------------------------------------------
// `no_std` math

const PI: f32 = core::f32::consts::PI;

/// Square root (Newton's method).
pub fn sqrt(v: f32) -> f32 {
    if v <= 0.0 || !v.is_finite() {
        return 0.0;
    }
    let mut x = f32::from_bits((v.to_bits() >> 1) + 0x1FC0_0000);
    for _ in 0..5 {
        x = 0.5 * (x + v / x);
    }
    x
}

/// `(sin x, cos x)`: reduction to [−π/2, π/2] and a Taylor series (error below 1e-6).
pub fn sin_cos(x: f32) -> (f32, f32) {
    fn sin_small(x: f32) -> f32 {
        let x2 = x * x;
        x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0 * (1.0 - x2 / 110.0)))))
    }
    fn sin_any(x: f32) -> f32 {
        let tau = 2.0 * PI;
        let mut r = x - tau * ((x / tau) as i64 as f32);
        if r > PI {
            r -= tau;
        } else if r < -PI {
            r += tau;
        }
        if r > PI / 2.0 {
            r = PI - r;
        } else if r < -PI / 2.0 {
            r = -PI - r;
        }
        sin_small(r)
    }
    (sin_any(x), sin_any(x + PI / 2.0))
}

/// `atan(x)` for any `x`: two half-angle reductions, then a series.
fn atan(x: f32) -> f32 {
    if !x.is_finite() {
        return if x > 0.0 { PI / 2.0 } else { -PI / 2.0 };
    }
    let (inv, x) = if x.abs() > 1.0 { (true, 1.0 / x) } else { (false, x) };
    // atan(x) = 2 atan(x / (1 + sqrt(1 + x^2))), twice: |y| <= tan(pi/16).
    let y = x / (1.0 + sqrt(1.0 + x * x));
    let y = y / (1.0 + sqrt(1.0 + y * y));
    let y2 = y * y;
    let s = y * (1.0 - y2 * (1.0 / 3.0 - y2 * (1.0 / 5.0 - y2 * (1.0 / 7.0 - y2 / 9.0))));
    let a = 4.0 * s;
    if inv { if x > 0.0 { PI / 2.0 - a } else { -PI / 2.0 - a } } else { a }
}

/// `atan2(y, x)`.
pub fn atan2(y: f32, x: f32) -> f32 {
    if x > 0.0 {
        atan(y / x)
    } else if x < 0.0 {
        if y >= 0.0 { atan(y / x) + PI } else { atan(y / x) - PI }
    } else if y > 0.0 {
        PI / 2.0
    } else if y < 0.0 {
        -PI / 2.0
    } else {
        0.0
    }
}

/// `acos(x)`, `x` clamped to [−1, 1].
pub fn acos(x: f32) -> f32 {
    let x = x.clamp(-1.0, 1.0);
    atan2(sqrt(1.0 - x * x), x)
}

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: Vec3) -> f32 {
    sqrt(dot(a, a))
}
fn normalize(a: Vec3) -> Option<Vec3> {
    let l = len(a);
    (l > 1e-12).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

/// Columns of `Rz(z) · Ry(y) · Rx(x)` (animation.md 4, kind 6).
pub fn euler_zyx(a: Vec3) -> Mat3 {
    let (sx, cx) = sin_cos(a[0]);
    let (sy, cy) = sin_cos(a[1]);
    let (sz, cz) = sin_cos(a[2]);
    [
        [cy * cz, cy * sz, -sy],
        [sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, sx * cy],
        [cx * sy * cz + sx * sz, cx * sy * sz - sx * cz, cx * cy],
    ]
}

/// Rotation by `angle` about the unit `axis` (columns).
pub fn axis_angle(axis: Vec3, angle: f32) -> Mat3 {
    let (s, c) = sin_cos(angle);
    let t = 1.0 - c;
    let [x, y, z] = axis;
    [
        [t * x * x + c, t * x * y + s * z, t * x * z - s * y],
        [t * x * y - s * z, t * y * y + c, t * y * z + s * x],
        [t * x * z + s * y, t * y * z - s * x, t * z * z + c],
    ]
}

/// `m · v` for a column-stored 3 x 3.
fn mat_vec(m: &Mat3, v: Vec3) -> Vec3 {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

fn mat_mul(a: &Mat3, b: &Mat3) -> Mat3 {
    [mat_vec(a, b[0]), mat_vec(a, b[1]), mat_vec(a, b[2])]
}

/// The inverse of an affine transform (`None` when singular).
pub fn inverse(m: &Affine) -> Option<Affine> {
    let [a, b, c] = m.cols;
    let det = dot(a, cross(b, c));
    if det.abs() < 1e-20 {
        return None;
    }
    // Rows of the inverse are the cross products of the columns over the determinant.
    let r0 = cross(b, c).map(|v| v / det);
    let r1 = cross(c, a).map(|v| v / det);
    let r2 = cross(a, b).map(|v| v / det);
    let cols = [[r0[0], r1[0], r2[0]], [r0[1], r1[1], r2[1]], [r0[2], r1[2], r2[2]]];
    let inv = Affine { cols, t: [0.0; 3] };
    let t = inv.transform_vector(m.t);
    Some(Affine { cols, t: [-t[0], -t[1], -t[2]] })
}

// ---------------------------------------------------------------------------------------------------------------
// The rig

/// What a chain records at load (animation.md 6, step 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChainBind {
    /// Upper length (upper to lower) and lower length (lower to effector).
    pub a: f32,
    pub b: f32,
    /// Bend angle between the two bones (0 = straight) and its axis in the upper bone's frame.
    pub bend: f32,
    pub axis: Vec3,
}

/// One object's transforms, posed.
#[derive(Clone, Debug, PartialEq)]
pub struct Rig {
    pub transforms: Vec<Transform>,
    /// Current local matrices (start as loaded).
    pub local: Vec<Affine>,
    /// World matrices after [`Rig::update`].
    pub world: Vec<Affine>,
    /// World matrices as loaded (the bind pose).
    pub bind_world: Vec<Affine>,
    pub chains: Vec<Chain>,
    pub mirrors: Vec<Mirror>,
    pub chain_binds: Vec<Option<ChainBind>>,
    /// Swivel angle per transform (scalar channels; read for chain upper bones).
    pub swivel: Vec<f32>,
}

fn chain_index(r: i16, n: usize) -> Option<usize> {
    match TransformRef::decode(r) {
        TransformRef::Own(i) if (i as usize) < n => Some(i as usize),
        _ => None,
    }
}

impl Rig {
    /// A rig in the loaded pose. `root` and `enclosing` resolve parent references −1 and `0x1000 | n`
    /// (transforms.md 1).
    pub fn new(list: &TransformList, root: &Affine, enclosing: &[Affine]) -> Rig {
        let local: Vec<Affine> = list.transforms.iter().map(|t| t.local).collect();
        let n = local.len();
        let mut rig = Rig {
            transforms: list.transforms.clone(),
            local,
            world: alloc::vec![Affine::IDENTITY; n],
            bind_world: Vec::new(),
            chains: list.chains.clone(),
            mirrors: list.mirrors.clone(),
            chain_binds: Vec::new(),
            swivel: alloc::vec![0.0; n],
        };
        rig.compute_world(root, enclosing);
        rig.bind_world = rig.world.clone();
        rig.chain_binds = rig.chains.iter().map(|c| rig.bind_chain(c)).collect();
        rig
    }

    fn bind_chain(&self, c: &Chain) -> Option<ChainBind> {
        let n = self.world.len();
        let (u, l, e) = (chain_index(c.upper, n)?, chain_index(c.lower, n)?, chain_index(c.end, n)?);
        let pu = self.world[u].t;
        let pl = self.world[l].t;
        let eff = self.world[e].transform_point(c.effector);
        let (uv, lv) = (sub(pl, pu), sub(eff, pl));
        let (a, b) = (len(uv), len(lv));
        // animation.md 6: a bone shorter than 0.0001 gives angle 0 and axis (1, 0, 0) (the executable's constant).
        if a < 1e-4 || b < 1e-4 {
            return Some(ChainBind { a, b, bend: 0.0, axis: [1.0, 0.0, 0.0] });
        }
        let bend = acos(dot(uv, lv) / (a * b));
        // The bend axis u x v, in the upper bone's frame. For parallel bones (|u x v|^2 <= 1e-8) the spec leaves the
        // perpendicular's rule open: this crate takes u x X, or u x Y when u is along X.
        let c = cross(uv, lv);
        let axis_world = if dot(c, c) > 1e-8 {
            normalize(c)?
        } else {
            let x = if (uv[0] / a).abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
            normalize(cross(uv, x))?
        };
        let inv = inverse(&self.world[u])?;
        let axis = normalize(inv.transform_vector(axis_world))?;
        Some(ChainBind { a, b, bend, axis })
    }

    /// Puts every local matrix and swivel back to its loaded value.
    pub fn reset(&mut self) {
        for (l, t) in self.local.iter_mut().zip(&self.transforms) {
            *l = t.local;
        }
        self.swivel.iter_mut().for_each(|s| *s = 0.0);
    }

    /// Applies a vector channel value to transform `i` by its kind (animation.md 4): kind 1 replaces the
    /// translation, kind 6 the rotation part (Euler angles), kind 12 the three diagonal entries; other kinds are not
    /// animated by vector channels.
    pub fn set_vector(&mut self, i: usize, v: Vec3) {
        let Some(t) = self.transforms.get(i) else { return };
        let l = &mut self.local[i];
        match t.kind {
            KIND_TRANSLATION => l.t = v,
            KIND_ROTATION => l.cols = euler_zyx(v),
            KIND_SCALE => {
                l.cols[0][0] = v[0];
                l.cols[1][1] = v[1];
                l.cols[2][2] = v[2];
            }
            _ => {}
        }
    }

    /// Sets the swivel angle of transform `i` (the upper bone of a chain).
    pub fn set_swivel(&mut self, i: usize, angle: f32) {
        if let Some(s) = self.swivel.get_mut(i) {
            *s = angle;
        }
    }

    fn parent_world(&self, i: usize, root: &Affine, enclosing: &[Affine]) -> Affine {
        match TransformRef::decode(self.transforms[i].parent) {
            TransformRef::Own(p) if (p as usize) < i => self.world[p as usize],
            TransformRef::Own(p) => self.world.get(p as usize).copied().unwrap_or(*root),
            TransformRef::Enclosing(p) => enclosing.get(p as usize).copied().unwrap_or(*root),
            _ => *root,
        }
    }

    fn compute_world(&mut self, root: &Affine, enclosing: &[Affine]) {
        for i in 0..self.local.len() {
            self.world[i] = self.parent_world(i, root, enclosing).mul(&self.local[i]);
        }
    }

    /// Recomputes the world matrices of the descendants of `changed` (parents before children in list order).
    fn propagate(&mut self, changed: usize) {
        let n = self.world.len();
        let mut dirty = alloc::vec![false; n];
        dirty[changed] = true;
        for j in changed + 1..n {
            if let TransformRef::Own(p) = TransformRef::decode(self.transforms[j].parent)
                && (p as usize) < j
                && dirty[p as usize]
            {
                self.world[j] = self.world[p as usize].mul(&self.local[j]);
                dirty[j] = true;
            }
        }
    }

    /// Sets transform `i`'s world matrix and its local matrix to match, then updates its descendants.
    fn set_world(&mut self, i: usize, w: Affine, root: &Affine, enclosing: &[Affine]) {
        if let Some(inv) = inverse(&self.parent_world(i, root, enclosing)) {
            self.local[i] = inv.mul(&w);
        }
        self.world[i] = w;
        self.propagate(i);
    }

    /// World matrices from the current locals, then mirror pairs, then chains (animation.md 6).
    pub fn update(&mut self, root: &Affine, enclosing: &[Affine]) {
        self.compute_world(root, enclosing);
        for m in self.mirrors.clone() {
            let n = self.world.len();
            let (Some(s), Some(t)) = (chain_index(m.source, n), chain_index(m.target, n)) else { continue };
            let mut w = self.world[t];
            if m.copy_rotation {
                w.cols = self.world[s].cols;
            }
            if m.copy_position {
                w.t = self.world[s].t;
            }
            self.world[t] = w;
            self.propagate(t);
        }
        for (k, c) in self.chains.clone().iter().enumerate() {
            if let Some(bind) = self.chain_binds[k] {
                self.solve_chain(c, &bind, root, enclosing);
            }
        }
    }

    /// The goal of a chain: the handle's world position plus the target offset in the handle's frame.
    pub fn chain_goal(&self, c: &Chain) -> Option<Vec3> {
        Some(self.world[chain_index(c.handle, self.world.len())?].transform_point(c.target))
    }

    /// The effector of a chain in world space.
    pub fn chain_effector(&self, c: &Chain) -> Option<Vec3> {
        Some(self.world[chain_index(c.end, self.world.len())?].transform_point(c.effector))
    }

    fn solve_chain(&mut self, c: &Chain, bind: &ChainBind, root: &Affine, enclosing: &[Affine]) {
        let n = self.world.len();
        let (Some(u), Some(l)) = (chain_index(c.upper, n), chain_index(c.lower, n)) else { return };
        // 2. Reset the two bones to their loaded rotations; find the goal.
        self.local[u].cols = self.transforms[u].local.cols;
        self.local[l].cols = self.transforms[l].local.cols;
        let wu = self.parent_world(u, root, enclosing).mul(&self.local[u]);
        self.world[u] = wu;
        self.propagate(u);
        let Some(goal) = self.chain_goal(c) else { return };
        // 3. Bend the lower bone about the bend axis to the angle that puts the effector at distance d.
        let pu = self.world[u].t;
        let d = len(sub(goal, pu)).clamp((bind.a - bind.b).abs(), bind.a + bind.b);
        let want = acos((d * d - bind.a * bind.a - bind.b * bind.b) / (2.0 * bind.a * bind.b));
        let axis_world = normalize(self.world[u].transform_vector(bind.axis));
        // A bone shorter than 0.0001 has no bend (animation.md 6).
        if let Some(axis) = axis_world.filter(|_| bind.a >= 1e-4 && bind.b >= 1e-4) {
            let r = axis_angle(axis, want - bind.bend);
            let mut wl = self.world[l];
            wl.cols = [mat_vec(&r, wl.cols[0]), mat_vec(&r, wl.cols[1]), mat_vec(&r, wl.cols[2])];
            self.set_world(l, wl, root, enclosing);
        }
        // 4. Turn the upper bone so the effector points at the goal, then spin it about the upper-to-goal axis by
        //    the swivel angle.
        let Some(eff) = self.chain_effector(c) else { return };
        let (e, g) = (sub(eff, pu), sub(goal, pu));
        let (Some(en), Some(gn)) = (normalize(e), normalize(g)) else { return };
        let turn = match normalize(cross(en, gn)) {
            Some(axis) => axis_angle(axis, acos(dot(en, gn))),
            None => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        };
        let spin = axis_angle(gn, self.swivel[u]);
        let r = mat_mul(&spin, &turn);
        let mut wu = self.world[u];
        wu.cols = [mat_vec(&r, wu.cols[0]), mat_vec(&r, wu.cols[1]), mat_vec(&r, wu.cols[2])];
        self.set_world(u, wu, root, enclosing);
    }

    /// The skin matrix of a bone (transforms.md 4): mesh space to world space, `W_bone · W_bone,bind⁻¹ ·
    /// W_mesh,bind`. With no animation it is the mesh transform's world matrix.
    pub fn skin_matrix(&self, bone: usize, mesh_transform: usize) -> Option<Affine> {
        let inv = inverse(self.bind_world.get(bone)?)?;
        Some(self.world.get(bone)?.mul(&inv).mul(self.bind_world.get(mesh_transform)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math() {
        for i in -40..40 {
            let x = i as f32 * 0.37;
            let (s, c) = sin_cos(x);
            assert!((s - libm_sin(x)).abs() < 1e-5 && (c - libm_sin(x + PI / 2.0)).abs() < 1e-5);
        }
        for i in -20..=20 {
            let x = i as f32 / 20.0;
            assert!((sin_cos(acos(x)).1 - x).abs() < 1e-5, "acos {x}");
        }
        assert!((atan2(1.0, -1.0) - 3.0 * PI / 4.0).abs() < 1e-6);
        assert!((atan2(-1.0, -1.0) + 3.0 * PI / 4.0).abs() < 1e-6);
        assert!((sqrt(2.0) * sqrt(2.0) - 2.0).abs() < 1e-6);
        let m = Affine { cols: euler_zyx([0.3, -0.7, 1.1]), t: [1.0, 2.0, 3.0] };
        let id = m.mul(&inverse(&m).unwrap());
        assert!(
            id.cols.iter().flatten().zip(Affine::IDENTITY.cols.iter().flatten()).all(|(a, b)| (a - b).abs() < 1e-5)
        );
        assert!(id.t.iter().all(|v| v.abs() < 1e-5));
        let r = axis_angle([0.0, 0.0, 1.0], PI / 2.0);
        assert!(len(sub(mat_vec(&r, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0])) < 1e-6);
    }

    fn libm_sin(x: f32) -> f32 {
        // The standard library's sine (tests link std).
        (x as f64).sin() as f32
    }

    fn transform(parent: i16, kind: i16, t: Vec3) -> Transform {
        Transform { parent, kind, id: 0, flags: 0, local: Affine { cols: Affine::IDENTITY.cols, t } }
    }

    #[test]
    fn kinds_mirrors_and_a_chain() {
        // 0 root, 1 translation, 2 upper (rotation), 3 lower (rotation), 4 handle, 5 mirror source, 6 target.
        let list = TransformList {
            transforms: alloc::vec![
                transform(-1, 0, [0.0; 3]),
                transform(0, 1, [0.0, 0.0, 75.0]),
                transform(1, 6, [0.0, 0.0, 0.0]),
                transform(2, 6, [0.0, 0.0, -40.0]),
                transform(1, 1, [0.0, 30.0, -60.0]),
                transform(1, 6, [0.0; 3]),
                transform(1, 6, [5.0; 3]),
            ],
            chains: alloc::vec![Chain {
                upper: 2,
                lower: 3,
                end: 3,
                effector: [0.0, 0.0, -40.0],
                handle: 4,
                target: [0.0; 3]
            }],
            mirrors: alloc::vec![Mirror { source: 5, target: 6, copy_position: false, copy_rotation: true }],
            followers: 0,
            references: [0; 3],
        };
        let mut rig = Rig::new(&list, &Affine::IDENTITY, &[]);
        let bind = rig.chain_binds[0].unwrap();
        assert!((bind.a - 40.0).abs() < 1e-4 && (bind.b - 40.0).abs() < 1e-4 && bind.bend.abs() < 1e-3);
        rig.set_vector(1, [320.0, 0.0, 75.0]);
        rig.set_vector(5, [0.0, 0.0, 1.0]);
        rig.set_vector(0, [9.0; 3]); // kind 0: ignored
        rig.update(&Affine::IDENTITY, &[]);
        assert_eq!(rig.world[1].t, [320.0, 0.0, 75.0]);
        // Mirror: the target's world rotation is the source's; its position is its own.
        assert_eq!(rig.world[6].cols, rig.world[5].cols);
        assert_eq!(rig.world[6].t, [325.0, 5.0, 80.0]);
        // The chain's end reaches the reachable goal.
        let (eff, goal) = (rig.chain_effector(&rig.chains[0]).unwrap(), rig.chain_goal(&rig.chains[0]).unwrap());
        assert!(len(sub(eff, goal)) < 1e-2, "{eff:?} vs {goal:?}");
        // Skin matrices: a bone in its bind pose gives the mesh transform's world matrix. (After reset the chain
        // solver still moves the chain bones toward their goal, so check a bone outside the chain.)
        rig.reset();
        rig.update(&Affine::IDENTITY, &[]);
        let s = rig.skin_matrix(1, 2).unwrap();
        assert!(len(sub(s.t, rig.bind_world[2].t)) < 1e-4);
        assert_eq!(s.cols, rig.bind_world[2].cols);
    }

    #[test]
    fn swivel_is_right_handed_about_the_goal_axis() {
        // animation.md 6, step 4: a positive swivel spins the limb right-handed about the unit vector from the upper
        // bone to the goal (counter-clockwise seen from the goal).
        let axis = [0.0, 0.0, 1.0];
        let r = axis_angle(axis, 0.3);
        let p = mat_vec(&r, [1.0, 0.0, 0.0]);
        assert!(p[1] > 0.0 && (p[0] - 0.955_336_5).abs() < 1e-5);
    }
}
