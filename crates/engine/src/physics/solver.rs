//! The physics world and its step (physics.md 3): integration, contacts through the collision world, the pivot
//! pass, the impact and penetration pass, rest detection and recorded impacts.

use alloc::vec::Vec;

use formats::geometry::Affine;

use super::body::{Body, Motion};
use crate::collision::world::{CollisionWorld, QueryFilter};
use crate::collision::{BodyId as CollisionBodyId, Materials};
use crate::math::{
    Vec3, add, axis_angle, cross, dot, length, madd, mat_mul, mat_mul_vec, normalize, orthonormalize, rotation_vector,
    scale, sub, transpose,
};

/// A source of random numbers in `[0, 1)`, injected so a run is deterministic.
pub trait Rng {
    /// The next number in `[0, 1)`.
    fn next_f32(&mut self) -> f32;
}

/// A small deterministic generator (SplitMix64), for callers without their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SplitMix64(pub u64);

impl Rng for SplitMix64 {
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// The solver's thresholds (physics.md 3.2) and the port's own tolerances.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolverSettings {
    /// An impact is solved when a contact of a body approaches faster than this (5 units/s).
    pub impact_speed: f32,
    /// Penetration is corrected when a contact is deeper than this (2 units).
    pub penetration_depth: f32,
    /// The penetration fix pushes each contact out by its depth minus this (1 unit), leaving it this deep.
    pub push_keep: f32,
    /// Range of the random restitution factor k (0.9 to 1.1).
    pub restitution_range: [f32; 2],
    /// Iterations of the pivot pass (the spec leaves the solver's iteration count open).
    pub pivot_iterations: u32,
    /// A contact with a gap up to this counts as touching (units; contacts are real touches only).
    pub touch_tolerance: f32,
    /// The port's guard against tunnelling: probes are swept along each frame's motion, shrunk by this much, and
    /// the motion stops where they hit. Shrinking by more than the 2-unit resting overlap keeps surfaces a body
    /// rests or slides on from blocking it.
    pub sweep_shrink: f32,
    /// Speeds below this count as still (units/s).
    pub rest_speed: f32,
    /// Angular speeds below this count as still (radians/s).
    pub rest_angular_speed: f32,
    /// Still frames before a body rests (2).
    pub rest_frames: u8,
}

impl Default for SolverSettings {
    fn default() -> Self {
        SolverSettings {
            impact_speed: 5.0,
            penetration_depth: 2.0,
            push_keep: 1.0,
            restitution_range: [0.9, 1.1],
            pivot_iterations: 4,
            touch_tolerance: 0.01,
            sweep_shrink: 3.0,
            rest_speed: 0.01,
            rest_angular_speed: 0.001,
            rest_frames: 2,
        }
    }
}

impl SolverSettings {
    /// Sly 2's rest rule (Sly 2 physics.md): a body is still below 5 units/s and 0.05 rad/s, and rests after one
    /// still frame. Sly 2 also moves a resting body back half a frame along its velocity; callers do that when a
    /// body first rests.
    pub fn sly2() -> SolverSettings {
        SolverSettings { rest_speed: 5.0, rest_angular_speed: 0.05, rest_frames: 1, ..SolverSettings::default() }
    }
}

/// Identifies a body of a [`PhysicsWorld`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BodyHandle(pub u32);

/// What a contact is against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Other {
    /// Another physics body.
    Body(BodyHandle),
    /// A collision body with no physics body (fixed scenery, or a solid the object model moves itself).
    Static(CollisionBodyId),
}

/// A contact found this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The body whose probe touched.
    pub body: BodyHandle,
    /// What it touched.
    pub other: Other,
    /// Contact point on the surface.
    pub point: Vec3,
    /// Normal from the surface toward the body.
    pub normal: Vec3,
    /// Penetration depth (negative: a gap within the contact margin).
    pub depth: f32,
    /// Material of the surface touched.
    pub material: i16,
}

/// An impact solved this step (physics.md 3 step 6: solids with a recorded impact react to it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    /// The body.
    pub body: BodyHandle,
    /// What it hit.
    pub other: Other,
    /// Contact point.
    pub point: Vec3,
    /// Contact normal (toward the body).
    pub normal: Vec3,
    /// Approach speed before the impact.
    pub speed: f32,
}

/// Bodies and the solver.
#[derive(Clone, Debug, Default)]
pub struct PhysicsWorld {
    bodies: Vec<Body>,
    enabled: Vec<bool>,
    by_collision: Vec<Option<u32>>,
    /// Solver settings.
    pub settings: SolverSettings,
    contacts: Vec<Contact>,
    impacts: Vec<Impact>,
    touched: Vec<bool>,
}

struct Pair {
    contact: Contact,
    other_body: Option<usize>,
}

impl PhysicsWorld {
    /// An empty world.
    pub fn new() -> PhysicsWorld {
        PhysicsWorld::default()
    }

    /// Adds a body.
    pub fn add(&mut self, body: Body) -> BodyHandle {
        let h = self.bodies.len() as u32;
        if let Some(c) = body.desc.collision {
            let i = c.0 as usize;
            if self.by_collision.len() <= i {
                self.by_collision.resize(i + 1, None);
            }
            self.by_collision[i] = Some(h);
        }
        self.bodies.push(body);
        self.enabled.push(true);
        BodyHandle(h)
    }

    /// Turns a body on or off (off: neither moved nor collided as a physics body).
    pub fn set_enabled(&mut self, h: BodyHandle, on: bool) {
        self.enabled[h.0 as usize] = on;
    }

    /// A body.
    pub fn body(&self, h: BodyHandle) -> &Body {
        &self.bodies[h.0 as usize]
    }

    /// A body, mutably. Changing its velocity or transform through this does not wake it; call `wake`.
    pub fn body_mut(&mut self, h: BodyHandle) -> &mut Body {
        &mut self.bodies[h.0 as usize]
    }

    /// Every body.
    pub fn bodies(&self) -> &[Body] {
        &self.bodies
    }

    /// The contacts of the last step (after the pivot pass, before positions moved).
    pub fn contacts(&self) -> &[Contact] {
        &self.contacts
    }

    /// The impacts of the last step.
    pub fn impacts(&self) -> &[Impact] {
        &self.impacts
    }

    /// Moves a kinematic body to `transform` over `dt`, deriving its velocity and angular velocity, and moves its
    /// collision entry. Bodies resting on it are woken.
    pub fn move_kinematic(&mut self, cw: &mut CollisionWorld, h: BodyHandle, transform: Affine, dt: f32) {
        let b = &mut self.bodies[h.0 as usize];
        if dt > 0.0 {
            let com_old = b.center_of_mass();
            let com_new = transform.transform_point(b.desc.center_of_mass);
            b.velocity = scale(sub(com_new, com_old), 1.0 / dt);
            b.angular_velocity =
                scale(rotation_vector(&mat_mul(&transform.cols, &transpose(&b.transform.cols))), 1.0 / dt);
        }
        b.transform = transform;
        if let Some(c) = b.desc.collision {
            cw.set_transform(c, transform);
        }
        for o in self.bodies.iter_mut() {
            o.wake();
        }
    }

    fn awake_dynamic(&self, i: usize) -> bool {
        let b = &self.bodies[i];
        self.enabled[i] && b.desc.motion == Motion::Dynamic && !b.asleep
    }

    /// One physics step of `dt` seconds (physics.md 3.2), against `cw` (which holds the fixed scenery and the
    /// bodies' own collision entries, moved here as the bodies move). Random restitution factors come from `rng`.
    ///
    /// The order: contacts at the start-of-frame positions (real touches only); integration (3.1); the pivot pass
    /// at those contacts (approach removed, never pulled together, with friction); contacts again at the moved
    /// positions; penetration (deeper than 2: each contact pushed out by its depth − 1); impacts (a body with a
    /// contact approaching faster than 5 gets `approach × (1 + e·k)` at each approaching contact); rest.
    pub fn step(&mut self, cw: &mut CollisionWorld, materials: &Materials, rng: &mut impl Rng, dt: f32) {
        self.impacts.clear();
        let n = self.bodies.len();
        self.touched.clear();
        self.touched.resize(n, false);
        let s = self.settings;
        // 2. Contacts at the start-of-frame positions.
        let start = self.find_contacts(cw);
        // 3. Integrate, then the pivot pass at those contacts (moved with their bodies).
        let mut disp: Vec<Vec3> = alloc::vec![[0.0; 3]; n];
        for (i, d) in disp.iter_mut().enumerate() {
            if self.awake_dynamic(i) {
                *d = self.integrate(cw, i, dt);
            }
        }
        self.sync_collision(cw, |_| true);
        let pivots: Vec<Pair> = start
            .into_iter()
            .map(|mut p| {
                p.contact.point = add(p.contact.point, disp[p.contact.body.0 as usize]);
                p
            })
            .collect();
        for _ in 0..s.pivot_iterations {
            for p in &pivots {
                if self.approach(p) > 0.0 {
                    self.resolve(p, materials, 0.0);
                }
            }
        }
        // 4. Contacts again at the moved positions.
        let pairs = self.find_contacts(cw);
        // 5. Penetration: a body with a contact deeper than 2 has its contacts pushed out to 1 deep. Several
        // contacts of one surface push once: each adds only what the pushes so far have not covered along its normal.
        let mut pushed: Vec<Vec3> = alloc::vec![[0.0; 3]; n];
        let deep: Vec<bool> = (0..n)
            .map(|i| pairs.iter().any(|p| p.contact.body.0 as usize == i && p.contact.depth > s.penetration_depth))
            .collect();
        for p in pairs.iter().filter(|p| deep[p.contact.body.0 as usize]) {
            let a = p.contact.body.0 as usize;
            let need = p.contact.depth - s.push_keep - dot(pushed[a], p.contact.normal);
            if need > 0.0 {
                let m = self.push_apart(p, need);
                pushed[a] = madd(pushed[a], p.contact.normal, m);
            }
        }
        // 6. Impacts, with the approach after integration and the pivot pass.
        let approach: Vec<f32> = pairs.iter().map(|p| self.approach(p)).collect();
        let hit: Vec<bool> = (0..n)
            .map(|i| pairs.iter().zip(&approach).any(|(p, &a)| p.contact.body.0 as usize == i && a > s.impact_speed))
            .collect();
        for (p, &speed) in pairs.iter().zip(&approach) {
            if !hit[p.contact.body.0 as usize] || speed <= 0.0 {
                continue;
            }
            let k = s.restitution_range[0] + (s.restitution_range[1] - s.restitution_range[0]) * rng.next_f32();
            let e = materials.elasticity(self.bodies[p.contact.body.0 as usize].desc.material, p.contact.material);
            self.resolve(p, materials, e * k * speed);
            self.impacts.push(Impact {
                body: p.contact.body,
                other: p.contact.other,
                point: p.contact.point,
                normal: p.contact.normal,
                speed,
            });
        }
        self.sync_collision(cw, |i| length(pushed[i]) > 0.0);
        self.contacts = pairs.iter().map(|p| p.contact).collect();
        // 7. Rest: still for the given number of consecutive frames.
        for i in 0..n {
            if !self.awake_dynamic(i) {
                continue;
            }
            let b = &mut self.bodies[i];
            if length(b.velocity) < s.rest_speed && length(b.angular_velocity) < s.rest_angular_speed {
                b.velocity = [0.0; 3];
                b.angular_velocity = [0.0; 3];
                b.still_frames = b.still_frames.saturating_add(1);
                if b.still_frames >= s.rest_frames {
                    b.asleep = true;
                }
            } else {
                b.still_frames = 0;
            }
            b.force = [0.0; 3];
            b.torque = [0.0; 3];
        }
    }

    /// Moves the collision entries of the awake bodies `which` accepts to their bodies' transforms.
    fn sync_collision(&self, cw: &mut CollisionWorld, which: impl Fn(usize) -> bool) {
        let mut moved = false;
        for i in 0..self.bodies.len() {
            if self.awake_dynamic(i)
                && which(i)
                && let Some(c) = self.bodies[i].desc.collision
            {
                cw.set_transform_deferred(c, self.bodies[i].transform);
                moved = true;
            }
        }
        if moved {
            cw.refit();
        }
    }

    /// Integrates body `i` over `dt` (physics.md 3.1): the frame's acceleration taken once at the start, then
    /// `x + v·dt + ½·a·dt²` and `v + a·dt`; the orientation turns by the average angular velocity. The motion is
    /// stopped at the first surface the shrunk probes hit (the port's guard against tunnelling). Returns the
    /// displacement of the centre of mass.
    fn integrate(&mut self, cw: &CollisionWorld, i: usize, dt: f32) -> Vec3 {
        let s = self.settings;
        let (a, alpha) = accelerations(&self.bodies[i], dt);
        let b = &self.bodies[i];
        let disp = madd(scale(b.velocity, dt), a, 0.5 * dt * dt);
        let disp = b.desc.lock_translation.apply(&b.transform.cols, disp);
        let filter = QueryFilter { ignore: b.desc.collision, ..QueryFilter::default() };
        let mut fraction = 1.0f32;
        if b.desc.interacts && length(disp) > 0.0 {
            for pr in &b.desc.probes {
                let c = b.transform.transform_point(pr.center);
                let r = (pr.radius - s.sweep_shrink).max(pr.radius * 0.5);
                if let Some(h) = cw.sphere_sweep(c, r, disp, &filter)
                    && !self.ignores(i, h.body)
                {
                    fraction = fraction.min(h.fraction);
                }
            }
        }
        let b = &mut self.bodies[i];
        let w0 = b.angular_velocity;
        b.velocity = b.desc.lock_translation.apply(&b.transform.cols, madd(b.velocity, a, dt));
        let w1 = b.desc.lock_rotation.apply(&b.transform.cols, madd(w0, alpha, dt));
        b.angular_velocity = w1;
        let turn = b.desc.lock_rotation.apply(&b.transform.cols, scale(add(w0, w1), 0.5 * dt));
        let disp = scale(disp, fraction);
        let com = add(b.center_of_mass(), disp);
        let cols = orthonormalize(&mat_mul(&axis_angle(turn), &b.transform.cols));
        b.transform = Affine { cols, t: sub(com, mat_mul_vec(&cols, b.desc.center_of_mass)) };
        disp
    }

    /// Whether body `i` must ignore collision body `c` (its own entry, or a body that does not interact).
    fn ignores(&self, i: usize, c: CollisionBodyId) -> bool {
        match self.by_collision.get(c.0 as usize).copied().flatten() {
            Some(o) => o as usize == i || !self.enabled[o as usize] || !self.bodies[o as usize].desc.interacts,
            None => false,
        }
    }

    /// The real contacts at the current positions: probes touching or overlapping a surface.
    fn find_contacts(&self, cw: &CollisionWorld) -> Vec<Pair> {
        let mut out = Vec::new();
        let mut overlaps = Vec::new();
        let margin = self.settings.touch_tolerance;
        for i in 0..self.bodies.len() {
            if !self.awake_dynamic(i) || !self.bodies[i].desc.interacts {
                continue;
            }
            let b = &self.bodies[i];
            let filter = QueryFilter { ignore: b.desc.collision, ..QueryFilter::default() };
            for pr in &b.desc.probes {
                let c = b.transform.transform_point(pr.center);
                overlaps.clear();
                cw.sphere_overlaps(c, pr.radius + margin, &filter, &mut overlaps);
                for o in &overlaps {
                    if self.ignores(i, o.body) {
                        continue;
                    }
                    let other_body = self.by_collision.get(o.body.0 as usize).copied().flatten().map(|h| h as usize);
                    let other = match other_body {
                        Some(h) => Other::Body(BodyHandle(h as u32)),
                        None => Other::Static(o.body),
                    };
                    out.push(Pair {
                        contact: Contact {
                            body: BodyHandle(i as u32),
                            other,
                            point: o.point,
                            normal: o.normal,
                            depth: o.depth - margin,
                            material: cw.body(o.body).material,
                        },
                        other_body,
                    });
                }
            }
        }
        out
    }

    /// Relative velocity of the body against the other at the contact point.
    fn relative_velocity(&self, p: &Pair) -> Vec3 {
        let a = self.bodies[p.contact.body.0 as usize].velocity_at(p.contact.point);
        match p.other_body {
            Some(o) => sub(a, self.bodies[o].velocity_at(p.contact.point)),
            None => a,
        }
    }

    /// Approach speed along the normal (positive when approaching).
    fn approach(&self, p: &Pair) -> f32 {
        -dot(self.relative_velocity(p), p.contact.normal)
    }

    /// The inverse effective mass of the pair along direction `d` at the contact point.
    fn inverse_mass_along(&self, p: &Pair, d: Vec3) -> f32 {
        let term = |b: &Body| {
            let im = b.inv_mass();
            if im == 0.0 {
                return 0.0;
            }
            let r = sub(p.contact.point, b.center_of_mass());
            let rn = cross(r, d);
            im + dot(cross(mat_mul_vec(&b.inv_inertia_world(), rn), r), d)
        };
        term(&self.bodies[p.contact.body.0 as usize]) + p.other_body.map_or(0.0, |o| term(&self.bodies[o]))
    }

    fn apply(&mut self, p: &Pair, impulse: Vec3) {
        // Solver impulses do not reset the rest count of the body they keep still; a resting other body that is hit
        // wakes up.
        let a = p.contact.body.0 as usize;
        self.bodies[a].add_impulse(impulse, p.contact.point);
        self.touched[a] = true;
        if let Some(o) = p.other_body {
            let b = &mut self.bodies[o];
            b.add_impulse(scale(impulse, -1.0), p.contact.point);
            if b.asleep && length(impulse) > 0.0 {
                b.wake();
            }
            self.touched[o] = true;
        }
    }

    /// One contact: raises the normal velocity to `target` when it is below it (an impact's separation speed, or
    /// minus the gap over the step for a pivot), with friction limited by μ times the normal impulse.
    fn resolve(&mut self, p: &Pair, materials: &Materials, target: f32) {
        let n = p.contact.normal;
        let rel = self.relative_velocity(p);
        let vn = dot(rel, n);
        if vn >= target {
            return;
        }
        let kn = self.inverse_mass_along(p, n);
        if kn <= 0.0 {
            return;
        }
        let jn = (target - vn) / kn;
        self.apply(p, scale(n, jn));
        // Friction against the remaining sliding velocity.
        let rel = self.relative_velocity(p);
        let vt = madd(rel, n, -dot(rel, n));
        let speed = length(vt);
        if speed > 0.0 {
            let t = scale(vt, 1.0 / speed);
            let kt = self.inverse_mass_along(p, t);
            if kt > 0.0 {
                let mu = materials.friction(self.bodies[p.contact.body.0 as usize].desc.material, p.contact.material);
                let jt = (speed / kt).min(mu * jn);
                self.apply(p, scale(t, -jt));
            }
        }
    }

    /// Moves the body (and a dynamic other, by inverse mass) apart along the normal by `distance`; returns how far
    /// the body itself moved.
    fn push_apart(&mut self, p: &Pair, distance: f32) -> f32 {
        let a = p.contact.body.0 as usize;
        let ima = self.bodies[a].inv_mass();
        let imb = p.other_body.map_or(0.0, |o| self.bodies[o].inv_mass());
        let total = ima + imb;
        if total <= 0.0 {
            return 0.0;
        }
        let n = normalize(p.contact.normal);
        let ba = &mut self.bodies[a];
        ba.transform.t = madd(ba.transform.t, n, distance * ima / total);
        if let Some(o) = p.other_body {
            let bo = &mut self.bodies[o];
            bo.transform.t = madd(bo.transform.t, n, -distance * imb / total);
            bo.wake();
        }
        distance * ima / total
    }
}

/// The frame's linear and angular acceleration of a body, from its state at the frame start (physics.md 2 and
/// 3.1): gravity, forces, the terminal fall drag, and in water buoyancy (with its upright turn) and viscosity
/// toward the current.
fn accelerations(b: &Body, dt: f32) -> (Vec3, Vec3) {
    let im = b.inv_mass();
    let g = b.desc.gravity;
    let f = b.submerged.clamp(0.0, 1.0);
    let buoyancy = b.desc.buoyancy * b.water_factors[0];
    let viscosity = b.desc.viscosity * b.water_factors[1];
    let cap = if dt > 0.0 { 1.0 / dt } else { f32::INFINITY };
    let mut a = add(g, scale(b.force, im));
    let mut alpha = mat_mul_vec(&b.inv_inertia_world(), b.torque);
    if let Some(t) = b.desc.terminal {
        a[2] += t.drag(b.desc.gravity_ratio, b.velocity[2]);
    }
    if f > 0.0 {
        let n = normalize(b.water_normal);
        if length(g) > 0.0 {
            // Buoyancy: −buoyancy × f × gravity, and a turn upright of 3 × buoyancy × f × θ.
            a = madd(a, g, -buoyancy * f);
            let up = b.transform.cols[2];
            let axis = cross(up, n);
            let s = length(axis);
            if s > 1e-6 {
                let theta = crate::math::atan2(s, dot(up, n));
                alpha = madd(alpha, axis, 3.0 * buoyancy * f * theta / s);
            }
        }
        // Viscosity: the velocity relative to the current, split along the surface normal and across it, each part
        // pulled toward the current at its own rate (capped at 1/dt).
        let dv = sub(b.current, b.velocity);
        let dn = dot(dv, n);
        let normal_part = scale(n, dn);
        let tangential = sub(dv, normal_part);
        let sn = if f < 1.0 && dn <= 0.0 { 0.5 + 0.5 * f } else { 1.0 };
        a = madd(a, normal_part, (3.0 * viscosity * sn).min(cap));
        a = madd(a, tangential, (3.0 * viscosity * f).min(cap));
        let w = b.angular_velocity;
        let wn = scale(n, dot(w, n));
        let wt = sub(w, wn);
        alpha = madd(alpha, wn, -(0.75 * viscosity * f).min(cap));
        alpha = madd(alpha, wt, -(0.75 * viscosity).min(cap));
    }
    (a, alpha)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision::TriMesh;
    use crate::collision::world::Shape;
    use crate::physics::body::{BodyDesc, Probe, TerminalFall, pickup_desc, player_desc};
    use alloc::vec;
    use formats::collision::{
        CollisionMesh, CollisionTree, CollisionTriangle, MATERIAL_COUNT, MaterialEntry, MaterialTables,
    };

    const DT: f32 = 1.0 / 60.0;

    fn materials() -> Materials {
        let e = [MaterialEntry { weight: 1.0, value: 0.5 }; MATERIAL_COUNT];
        Materials::new(MaterialTables { friction: e, elasticity: e })
    }

    /// A floor at z = 0 spanning ±10,000.
    fn floor() -> CollisionWorld {
        let s = 10_000.0;
        let positions = vec![[-s, -s, 0.0], [s, -s, 0.0], [s, s, 0.0], [-s, s, 0.0]];
        let tri = |c| CollisionTriangle { corners: c, radius: s, center: [0.0; 3], first_edge: 0, edge_count: 0 };
        let mesh =
            CollisionMesh { radius: s, positions, triangles: vec![tri([0, 1, 2]), tri([0, 2, 3])], edges: vec![] };
        let mut cw = CollisionWorld::new();
        let sh = cw.add_shape(Shape { main: TriMesh::new(&mesh, &CollisionTree::default()), camera: None });
        cw.add_body(sh, Affine::IDENTITY, 0, 0, true);
        cw.rebuild();
        cw
    }

    fn at(z: f32) -> Affine {
        Affine { cols: crate::math::MAT3_IDENTITY, t: [0.0, 0.0, z] }
    }

    fn ball(desc: BodyDesc, z: f32) -> Body {
        Body::new(BodyDesc { probes: vec![Probe { center: [0.0; 3], radius: 10.0 }], ..desc }, at(z))
    }

    #[test]
    fn crate_falls_750_units_in_one_second() {
        // physics.md 6: world gravity, from rest, 1 s: 750 units down at 1500 units/s.
        let mut cw = CollisionWorld::new();
        cw.rebuild();
        let mut pw = PhysicsWorld::new();
        let h = pw.add(Body::new(BodyDesc::default(), at(0.0)));
        let mut rng = SplitMix64(1);
        for _ in 0..60 {
            pw.step(&mut cw, &materials(), &mut rng, DT);
        }
        let b = pw.body(h);
        assert!((b.transform.t[2] + 750.0).abs() < 1e-2, "{}", b.transform.t[2]);
        assert!((b.velocity[2] + 1500.0).abs() < 1e-2);
    }

    #[test]
    fn player_fall_drag_settles_at_1310() {
        // physics.md 6: z = −1137.5 t² to t = 0.3759 s (−855.1 units/s, 160.7 down), then toward −1310.1 with time
        // constant 0.2 s: −1065.5 and 281.2 down at 0.5 s; −1290.1 and 891.4 down at 1 s; −1310.0 at 2 s.
        let t = TerminalFall { speed: -750.0, rate: 5.0 };
        assert!((t.settling_speed(1.3, -2275.0) + 1310.1).abs() < 0.05);
        let mut cw = CollisionWorld::new();
        cw.rebuild();
        let mut pw = PhysicsWorld::new();
        let h = pw.add(Body::new(player_desc(t), at(0.0)));
        let mut rng = SplitMix64(1);
        let dt = 1.0 / 2400.0;
        let mut k = 0u32;
        let check = |pw: &PhysicsWorld, time: f32, z: f32, v: f32| {
            let b = pw.body(h);
            assert!((b.transform.t[2] + z).abs() < 0.3, "t {time}: z {}", b.transform.t[2]);
            assert!((b.velocity[2] + v).abs() < 0.3, "t {time}: v {}", b.velocity[2]);
        };
        for (time, z, v) in [(0.3759f32, 160.7f32, 855.1f32), (0.5, 281.2, 1065.5), (1.0, 891.4, 1290.1)] {
            while (k as f32) * dt < time - dt * 0.5 {
                pw.step(&mut cw, &materials(), &mut rng, dt);
                k += 1;
            }
            check(&pw, time, z, v);
        }
        for _ in 0..2400 {
            pw.step(&mut cw, &materials(), &mut rng, dt);
        }
        assert!((pw.body(h).velocity[2] + 1310.0).abs() < 0.3);
        // At 60 frames/s with the acceleration taken at the frame start: after 61 frames z −916.0, v −1294.1.
        let mut pw = PhysicsWorld::new();
        let h = pw.add(Body::new(player_desc(t), at(0.0)));
        for _ in 0..61 {
            pw.step(&mut cw, &materials(), &mut rng, DT);
        }
        let b = pw.body(h);
        assert!(
            (b.transform.t[2] + 916.0).abs() < 0.1 && (b.velocity[2] + 1294.1).abs() < 0.1,
            "{:?} {:?}",
            b.transform.t,
            b.velocity
        );
    }

    #[test]
    fn pickup_tossed_up_reaches_its_apex() {
        // physics.md 6: 500 units/s up with −2250: apex after 0.222 s, 55.6 units up.
        let mut cw = CollisionWorld::new();
        cw.rebuild();
        let mut pw = PhysicsWorld::new();
        let h = pw.add(Body::new(pickup_desc(), at(0.0)));
        pw.body_mut(h).velocity = [0.0, 0.0, 500.0];
        let mut rng = SplitMix64(1);
        let dt = 1.0 / 3600.0;
        let (mut top, mut t_top, mut t) = (0.0f32, 0.0f32, 0.0f32);
        for _ in 0..3600 {
            pw.step(&mut cw, &materials(), &mut rng, dt);
            t += dt;
            if pw.body(h).transform.t[2] > top {
                top = pw.body(h).transform.t[2];
                t_top = t;
            }
        }
        assert!((top - 55.6).abs() < 0.05 && (t_top - 0.222).abs() < 0.001, "{top} at {t_top}");
    }

    fn drop_ball(seed: u64) -> (Vec<f32>, f32) {
        let mut cw = floor();
        let mut pw = PhysicsWorld::new();
        let h = pw.add(ball(BodyDesc::default(), 300.0));
        let mut rng = SplitMix64(seed);
        let mut zs = Vec::new();
        let mut first = 0.0;
        for _ in 0..600 {
            pw.step(&mut cw, &materials(), &mut rng, DT);
            if first == 0.0
                && let Some(i) = pw.impacts().first()
            {
                first = i.speed;
            }
            zs.push(pw.body(h).transform.t[2]);
        }
        (zs, first)
    }

    #[test]
    fn bounces_settles_and_rests() {
        let mut cw = floor();
        let mut pw = PhysicsWorld::new();
        let h = pw.add(ball(BodyDesc::default(), 300.0));
        let mut rng = SplitMix64(7);
        let mut bounce = None;
        let mut frames = 0;
        while !pw.body(h).is_at_rest() && frames < 1200 {
            pw.step(&mut cw, &materials(), &mut rng, DT);
            frames += 1;
            if bounce.is_none()
                && let Some(i) = pw.impacts().first()
            {
                // Elasticity of material 0 against 0 is 0.5; restitution factor in [0.9, 1.1].
                let v = pw.body(h).velocity[2];
                let ratio = v / i.speed;
                assert!((0.45..=0.55).contains(&ratio), "{ratio}");
                bounce = Some(v);
            }
            assert!(pw.body(h).transform.t[2] > 10.0 - 2.5, "sank to {}", pw.body(h).transform.t[2]);
        }
        assert!(bounce.is_some());
        assert!(pw.body(h).is_at_rest(), "still moving after {frames} frames");
        // A resting object is embedded between 0 and 2 units.
        let z = pw.body(h).transform.t[2];
        assert!((8.0 - 0.3..=10.0 + 1e-3).contains(&z), "{z}");
        // Two still frames: one step after coming to a stop is not enough.
        let mut b = ball(BodyDesc::default(), 10.0);
        b.velocity = [0.0; 3];
        let mut pw = PhysicsWorld::new();
        let h = pw.add(b);
        pw.step(&mut cw, &materials(), &mut rng, DT);
        assert!(!pw.body(h).is_at_rest());
        pw.step(&mut cw, &materials(), &mut rng, DT);
        assert!(pw.body(h).is_at_rest());
    }

    #[test]
    fn sly2_fall_bounce_and_rest_rule() {
        use crate::physics::body::SLY2_GRAVITY;
        // Sly 2 physics.md: −1750 from rest for 1 s is 875 units at 1750 units/s; characters (×1.3) 1137.5.
        let mut cw = CollisionWorld::new();
        cw.rebuild();
        let mut pw = PhysicsWorld::new();
        pw.settings = SolverSettings::sly2();
        let a = pw.add(Body::new(BodyDesc { gravity: [0.0, 0.0, SLY2_GRAVITY], ..BodyDesc::default() }, at(0.0)));
        let b = pw.add(Body::new(BodyDesc { gravity: [0.0, 0.0, SLY2_GRAVITY * 1.3], ..BodyDesc::default() }, at(0.0)));
        let mut rng = SplitMix64(3);
        for _ in 0..60 {
            pw.step(&mut cw, &materials(), &mut rng, DT);
        }
        assert!((pw.body(a).transform.t[2] + 875.0).abs() < 1e-2);
        assert!((pw.body(a).velocity[2] + 1750.0).abs() < 1e-2);
        assert!((pw.body(b).transform.t[2] + 1137.5).abs() < 1e-2);
        // Contact friction and elasticity 0.5 each: the rebound is 0.45 to 0.55 of the impact speed (an impulse of
        // 1.45 to 1.55 times it), and the body rests after a single still frame below 5 units/s.
        let mut cw = floor();
        let mut pw = PhysicsWorld::new();
        pw.settings = SolverSettings::sly2();
        let h = pw.add(ball(BodyDesc { gravity: [0.0, 0.0, SLY2_GRAVITY], ..BodyDesc::default() }, 300.0));
        let mut bounced = false;
        let mut frames = 0;
        while !pw.body(h).is_at_rest() && frames < 1200 {
            pw.step(&mut cw, &materials(), &mut rng, DT);
            frames += 1;
            if !bounced && let Some(i) = pw.impacts().first() {
                let ratio = pw.body(h).velocity[2] / i.speed;
                assert!((0.45..=0.55).contains(&ratio), "{ratio}");
                bounced = true;
            }
        }
        assert!(bounced && pw.body(h).is_at_rest());
        let mut b = ball(BodyDesc::default(), 10.0);
        b.velocity = [0.0; 3];
        let mut pw = PhysicsWorld::new();
        pw.settings = SolverSettings::sly2();
        let h = pw.add(b);
        pw.step(&mut cw, &materials(), &mut rng, DT);
        assert!(pw.body(h).is_at_rest());
    }

    #[test]
    fn deep_penetration_is_pushed_out_to_one_unit() {
        let mut cw = floor();
        let mut pw = PhysicsWorld::new();
        // Probe radius 10 with its centre at 7: 3 units deep, pushed by depth − 1 = 2 to 1 unit deep.
        let h = pw.add(ball(BodyDesc { gravity: [0.0; 3], ..BodyDesc::default() }, 7.0));
        pw.step(&mut cw, &materials(), &mut SplitMix64(1), DT);
        assert!((pw.body(h).transform.t[2] - 9.0).abs() < 1e-3, "{}", pw.body(h).transform.t[2]);
        // 1.5 deep: left alone.
        let mut pw = PhysicsWorld::new();
        let h = pw.add(ball(BodyDesc { gravity: [0.0; 3], ..BodyDesc::default() }, 8.5));
        pw.step(&mut cw, &materials(), &mut SplitMix64(1), DT);
        assert!((pw.body(h).transform.t[2] - 8.5).abs() < 1e-3);
    }

    #[test]
    fn landing_frame_is_embedded_and_bounces() {
        // A falling ball meets the floor in the contacts after integration, embedded, and bounces with e·k of its
        // approach; a ball resting on its pivot shows no approach and no impact.
        let mut cw = floor();
        let mut pw = PhysicsWorld::new();
        let h = pw.add(ball(BodyDesc::default(), 40.0));
        pw.body_mut(h).velocity = [0.0, 0.0, -600.0];
        let mut rng = SplitMix64(9);
        let mut landed = false;
        for _ in 0..10 {
            pw.step(&mut cw, &materials(), &mut rng, DT);
            if let Some(i) = pw.impacts().first() {
                let z = pw.body(h).transform.t[2];
                assert!((8.0 - 1e-3..=10.0).contains(&z), "landing frame ends embedded up to 2: {z}");
                assert!(i.speed > 600.0);
                landed = true;
                break;
            }
        }
        assert!(landed);
        let mut pw = PhysicsWorld::new();
        let h = pw.add(ball(BodyDesc::default(), 9.0));
        for _ in 0..3 {
            pw.step(&mut cw, &materials(), &mut rng, DT);
            assert!(pw.impacts().is_empty());
        }
        assert!(pw.body(h).is_at_rest());
    }

    #[test]
    fn deterministic_for_a_seed() {
        let (a, sa) = drop_ball(42);
        let (b, sb) = drop_ball(42);
        assert_eq!(
            a.iter().map(|z| z.to_bits()).collect::<Vec<_>>(),
            b.iter().map(|z| z.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(sa, sb);
        let (c, _) = drop_ball(43);
        assert_ne!(a, c);
    }

    #[test]
    fn friction_stops_sliding() {
        let mut cw = floor();
        let mut pw = PhysicsWorld::new();
        let h = pw.add(ball(
            BodyDesc { inertia: [[1e9, 0.0, 0.0], [0.0, 1e9, 0.0], [0.0, 0.0, 1e9]], ..Default::default() },
            10.0,
        ));
        pw.body_mut(h).velocity = [200.0, 0.0, 0.0];
        let mut rng = SplitMix64(3);
        for _ in 0..120 {
            pw.step(&mut cw, &materials(), &mut rng, DT);
        }
        // μ = 0.5: decelerates at about 0.5 × 1500 = 750 units/s², so it stops within the first half second.
        assert!(pw.body(h).velocity[0].abs() < 1.0, "{:?}", pw.body(h).velocity);
        let x = pw.body(h).transform.t[0];
        assert!((x - 200.0 * 200.0 / (2.0 * 750.0)).abs() < 8.0, "{x}");
    }

    #[test]
    fn buoyancy_floats_and_viscosity_pulls_toward_the_current() {
        let mut cw = CollisionWorld::new();
        cw.rebuild();
        let mut pw = PhysicsWorld::new();
        // physics.md 6: a crate at rest, fully under, buoyancy 0.5, viscosity 1, world gravity: 750 up from buoyancy,
        // net −750; the velocity is damped at 3 per second toward the current.
        let h = pw.add(Body::new(BodyDesc::default(), at(0.0)));
        pw.body_mut(h).submerged = 1.0;
        pw.step(&mut cw, &materials(), &mut SplitMix64(1), DT);
        assert!((pw.body(h).velocity[2] + 750.0 * DT).abs() < 1e-3, "{:?}", pw.body(h).velocity);
        let h2 = pw.add(Body::new(BodyDesc { gravity: [0.0; 3], viscosity: 1.0, ..Default::default() }, at(0.0)));
        pw.body_mut(h2).submerged = 1.0;
        pw.body_mut(h2).velocity = [60.0, 0.0, 0.0];
        pw.body_mut(h2).current = [0.0, 30.0, 0.0];
        pw.step(&mut cw, &materials(), &mut SplitMix64(1), DT);
        // Δv = (−60, 30, 0), tangential, rate 3: a = (−180, 90, 0).
        let v = pw.body(h2).velocity;
        assert!((v[0] - 57.0).abs() < 1e-3 && (v[1] - 1.5).abs() < 1e-3, "{v:?}");
        // Partly under and moving up out of the water: the normal rate is 3 × (0.5 + 0.5 f).
        let h3 = pw.add(Body::new(BodyDesc { gravity: [0.0; 3], viscosity: 1.0, ..Default::default() }, at(0.0)));
        pw.body_mut(h3).submerged = 0.5;
        pw.body_mut(h3).velocity = [0.0, 0.0, 100.0];
        pw.step(&mut cw, &materials(), &mut SplitMix64(1), DT);
        assert!((pw.body(h3).velocity[2] - (100.0 - 225.0 * DT)).abs() < 1e-3, "{:?}", pw.body(h3).velocity);
    }
}
