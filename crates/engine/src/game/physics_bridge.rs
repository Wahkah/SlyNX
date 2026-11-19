//! Physics and collision for the world: `engine::collision` and `engine::physics` behind the game loop's
//! [`PhysicsSystem`] and [`CollisionQueries`].
//!
//! - **Collision bodies.** At load, every object with the solid or conveyor layout gets a body in a
//!   [`CollisionWorld`] at its world transform, with its material (option 128); clones share the shape of their
//!   source record. A body is enabled while its object is active. Each frame, bodies whose object moved are moved
//!   (the object model, animation and scripts move solids; the hierarchy is refitted once).
//! - **Physics bodies.** Objects become dynamic when a behaviour asks (`World::physics`, [`PhysicsCommand`]): a
//!   released breakable piece, a thrown bomb, a dropped object. The solver moves them, and their transforms are
//!   written back to the world after each step. Everything else is kinematic: moved by the object model, it pushes
//!   dynamic bodies with the velocity of its motion.
//! - **Contacts.** Every contact of the step is delivered as a [`Notice::Touched`] to both objects, every frame it
//!   lasts; impacts as [`Notice::Impact`]. Script events 6 and 7 are raised when a pair starts and stops touching.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;
use alloc::vec::Vec;

use formats::collision::{MATERIAL_COUNT, MaterialEntry, MaterialTables, Solid};
use formats::geometry::Affine;
use formats::stream::{Layout, layout_of};
use script::Value;

use super::systems::{CollisionQueries, PhysicsSystem, StepInfo, SweepHit};
use crate::collision::{BodyId, CollisionWorld, Materials, QueryFilter, Shape, ShapeId};
use crate::object::{Handle, flags};
use crate::physics::body::{Body as PhysBody, BodyDesc, Probe, probes_from_mesh};
use crate::physics::solver::{BodyHandle, Other, PhysicsWorld, SplitMix64};
use crate::world::{Notice, PhysicsCommand, World};

/// Radius of the contact probes put at a dynamic body's collision-mesh corners.
pub const PROBE_RADIUS: f32 = 5.0;

/// Neutral material tables for when the executable's are not available: every entry weight 1, value 0.5
/// (friction and elasticity 0.5 for every pair).
pub fn neutral_materials() -> MaterialTables {
    let e = MaterialEntry { weight: 1.0, value: 0.5 };
    MaterialTables { friction: [e; MATERIAL_COUNT], elasticity: [e; MATERIAL_COUNT] }
}

/// Physics and collision for one world.
pub struct PhysicsBridge {
    /// The collision world.
    pub cw: CollisionWorld,
    /// The solver.
    pub pw: PhysicsWorld,
    materials: Materials,
    rng: SplitMix64,
    /// Collision body of each object slot.
    body_of: BTreeMap<u32, BodyId>,
    /// Object slot of each collision body.
    slot_of_body: Vec<u32>,
    /// Physics body of each dynamic object slot.
    phys_of: BTreeMap<u32, BodyHandle>,
    /// Object slot of each physics body (`u32::MAX` once released).
    slot_of_phys: Vec<u32>,
    touching: BTreeSet<(u32, u32)>,
    loaded: bool,
    excluded: BTreeSet<u32>,
    /// The collision world was given ([`PhysicsBridge::from_collision`]): `load` keeps it.
    prebuilt: bool,
}

impl PhysicsBridge {
    /// A bridge with the executable's material tables (or neutral ones).
    pub fn new(materials: Option<MaterialTables>) -> PhysicsBridge {
        PhysicsBridge {
            cw: CollisionWorld::new(),
            pw: PhysicsWorld::new(),
            materials: Materials::new(materials.unwrap_or_else(neutral_materials)),
            rng: SplitMix64(0x005E_ED0F_51CE),
            body_of: BTreeMap::new(),
            slot_of_body: Vec::new(),
            phys_of: BTreeMap::new(),
            slot_of_phys: Vec::new(),
            touching: BTreeSet::new(),
            loaded: false,
            excluded: BTreeSet::new(),
            prebuilt: false,
        }
    }

    /// A bridge over a collision world built by the caller (Sly 2 and Sly 3 worlds, whose solids the Sly 1 loader
    /// does not read): each body's `user` is its object's slot. Dynamic objects (thrown objects) move through it with
    /// the solver, and contacts are reported as for Sly 1.
    pub fn from_collision(cw: CollisionWorld) -> PhysicsBridge {
        let mut b = PhysicsBridge::new(None);
        for (i, body) in cw.bodies().iter().enumerate() {
            if body.user != u32::MAX {
                b.body_of.entry(body.user).or_insert(BodyId(i as u32));
            }
            b.slot_of_body.push(body.user);
        }
        b.cw = cw;
        b.prebuilt = true;
        b
    }

    /// The collision body of an object.
    pub fn body_of(&self, world: &World, h: Handle) -> Option<BodyId> {
        self.body_of.get(&world.slot(h)?).copied()
    }

    /// The physics body of a dynamic object.
    pub fn phys_of(&self, world: &World, h: Handle) -> Option<BodyHandle> {
        self.phys_of.get(&world.slot(h)?).copied()
    }

    /// Number of dynamic bodies.
    pub fn dynamic_count(&self) -> usize {
        self.phys_of.len()
    }

    fn filter(&self, world: &World, ignore: Option<Handle>) -> QueryFilter {
        QueryFilter { ignore: ignore.and_then(|h| self.body_of(world, h)), ..Default::default() }
    }

    fn handle_of_body(&self, world: &World, b: BodyId) -> Option<Handle> {
        world.handle(*self.slot_of_body.get(b.0 as usize)?)
    }

    fn handle_of_other(&self, world: &World, o: Other) -> Option<Handle> {
        match o {
            Other::Body(p) => world.handle(*self.slot_of_phys.get(p.0 as usize)?),
            Other::Static(b) => self.handle_of_body(world, b),
        }
    }

    /// Makes an object dynamic.
    fn make_dynamic(
        &mut self,
        world: &mut World,
        h: Handle,
        velocity: [f32; 3],
        angular: [f32; 3],
        radius: Option<f32>,
    ) {
        let Some(slot) = world.slot(h) else { return };
        if let Some(&p) = self.phys_of.get(&slot) {
            let b = self.pw.body_mut(p);
            b.velocity = velocity;
            b.angular_velocity = angular;
            b.wake();
            return;
        }
        let o = world.options(h).cloned().unwrap_or_default();
        let collision = self.body_of.get(&slot).copied();
        let mut desc = BodyDesc { collision, ..Default::default() };
        // Sly 1 solids carry their mass; other games' objects use the defaults and the radius given.
        let rec = world.record(h).filter(|_| world.sly1_meanings()).map(|r| world.level.record(r).clone());
        if let Some(r) = rec.filter(|r| matches!(layout_of(r.class), Some(Layout::Solid | Layout::Conveyor)))
            && let Ok(solid) = Solid::parse(&world.level.bytes, r.offset + 6)
        {
            if solid.mass > 0.0 {
                desc.mass = solid.mass;
                desc.inertia = solid.inertia;
            }
            desc.center_of_mass = solid.center_of_mass;
            desc.probes = probes_from_mesh(&solid.mesh, PROBE_RADIUS);
        }
        if desc.probes.is_empty() {
            desc.probes = vec![Probe { center: [0.0; 3], radius: radius.unwrap_or(PROBE_RADIUS) }];
        }
        desc.mass = o.f32(127).unwrap_or(desc.mass).max(1e-3);
        desc.material = o.i16(128).unwrap_or(0);
        if let Some(g) = o.vec3(129) {
            desc.gravity = g;
        }
        if o.bool_or(138, false) {
            desc.gravity = [0.0; 3];
        }
        desc.interacts = !o.bool_or(130, false);
        desc.buoyancy = o.f32_or(134, desc.buoyancy);
        desc.viscosity = o.f32_or(135, desc.viscosity);
        let transform = world.world_transform(h).unwrap_or(Affine::IDENTITY);
        let mut body = PhysBody::new(desc, transform);
        body.velocity = velocity;
        body.angular_velocity = angular;
        let p = self.pw.add(body);
        if self.slot_of_phys.len() <= p.0 as usize {
            self.slot_of_phys.resize(p.0 as usize + 1, u32::MAX);
        }
        self.slot_of_phys[p.0 as usize] = slot;
        self.phys_of.insert(slot, p);
    }

    fn release(&mut self, world: &World, h: Handle) {
        let Some(slot) = world.slot(h) else { return };
        if let Some(p) = self.phys_of.remove(&slot) {
            self.pw.set_enabled(p, false);
            self.slot_of_phys[p.0 as usize] = u32::MAX;
        }
    }

    /// Moves the collision bodies of objects the object model moved, and turns bodies on and off with their objects.
    fn sync_kinematic(&mut self, world: &World, dt: f32) {
        let wt = world.world_transforms();
        let fl = world.flags_column();
        let mut moved = false;
        for (&slot, &b) in &self.body_of {
            let s = slot as usize;
            if s >= fl.len() || world.handle(slot).is_none() {
                self.cw.set_enabled(b, false);
                continue;
            }
            let active = fl[s] & flags::ACTIVE != 0 && fl[s] & flags::UNPLACED == 0 && !self.excluded.contains(&slot);
            if self.cw.body(b).enabled != active {
                self.cw.set_enabled(b, active);
            }
            if self.phys_of.contains_key(&slot) || !active {
                continue;
            }
            if self.cw.body(b).transform != wt[s] {
                self.cw.set_transform_deferred(b, wt[s]);
                moved = true;
            }
        }
        if moved {
            self.cw.refit();
        }
        let _ = dt;
    }
}

impl PhysicsSystem for PhysicsBridge {
    fn load(&mut self, world: &mut World) {
        world.update_transforms();
        if self.prebuilt {
            self.loaded = true;
            return;
        }
        let mut shapes: BTreeMap<u32, Option<(ShapeId, i16, bool)>> = BTreeMap::new();
        let wt = world.world_transforms().to_vec();
        for h in world.handles().collect::<Vec<_>>() {
            let Some(rec) = world.record(h) else { continue };
            let r = world.level.record(rec);
            if !matches!(layout_of(r.class), Some(Layout::Solid | Layout::Conveyor)) {
                continue;
            }
            let entry = *shapes.entry(rec).or_insert_with(|| {
                let solid = Solid::parse(&world.level.bytes, r.offset + 6).ok()?;
                if solid.mesh.is_empty() && solid.camera_mesh.is_empty() {
                    return None;
                }
                let material = world.options(h).and_then(|o| o.i16(128)).unwrap_or(0);
                Some((self.cw.add_shape(Shape::from_solid(&solid)), material, solid.is_fixed()))
            });
            let Some((shape, material, fixed)) = entry else { continue };
            let b = self.cw.add_body(shape, wt[h.index() as usize], material, h.index(), fixed);
            self.body_of.insert(h.index(), b);
            if self.slot_of_body.len() <= b.0 as usize {
                self.slot_of_body.resize(b.0 as usize + 1, u32::MAX);
            }
            self.slot_of_body[b.0 as usize] = h.index();
        }
        self.cw.rebuild();
        self.sync_kinematic(world, 0.0);
        self.loaded = true;
    }

    fn step(&mut self, world: &mut World, step: &StepInfo) {
        for cmd in core::mem::take(&mut world.physics) {
            match cmd {
                PhysicsCommand::Dynamic { object, velocity, angular, radius } => {
                    self.make_dynamic(world, object, velocity, angular, radius)
                }
                PhysicsCommand::Impulse { object, impulse, point } => {
                    if let Some(p) = self.phys_of(world, object) {
                        self.pw.body_mut(p).apply_impulse(impulse, point);
                    }
                }
                PhysicsCommand::Release(h) => self.release(world, h),
            }
        }
        self.sync_kinematic(world, step.game_dt);
        if step.game_dt <= 0.0 {
            return;
        }
        self.pw.step(&mut self.cw, &self.materials, &mut self.rng, step.game_dt);

        // Write the solver's transforms back.
        let moved: Vec<(u32, Affine)> =
            self.phys_of.iter().map(|(&slot, &p)| (slot, self.pw.body(p).transform)).collect();
        for (slot, t) in moved {
            if let Some(h) = world.handle(slot) {
                world.set_world_transform(h, t);
            }
        }

        // Contacts: notices every frame, events 6 and 7 on changes.
        let mut now: BTreeSet<(u32, u32)> = BTreeSet::new();
        for c in self.pw.contacts() {
            let a = self.slot_of_phys.get(c.body.0 as usize).copied().unwrap_or(u32::MAX);
            let Some(ha) = world.handle(a) else { continue };
            let hb = self.handle_of_other(world, c.other);
            if let Some(hb) = hb {
                world.notify(ha, Notice::Touched(hb));
                world.notify(hb, Notice::Touched(ha));
                let key = if a < hb.index() { (a, hb.index()) } else { (hb.index(), a) };
                now.insert(key);
            }
        }
        for i in self.pw.impacts() {
            let a = self.slot_of_phys.get(i.body.0 as usize).copied().unwrap_or(u32::MAX);
            let Some(ha) = world.handle(a) else { continue };
            let hb = self.handle_of_other(world, i.other);
            world.notify(ha, Notice::Impact(hb, i.speed));
            if let Some(hb) = hb {
                world.notify(hb, Notice::Impact(Some(ha), i.speed));
            }
        }
        for &(a, b) in now.difference(&self.touching) {
            if let (Some(ha), Some(hb)) = (world.handle(a), world.handle(b)) {
                world.raise(ha, script::event::TOUCH_BEGIN, vec![Value::Object(Some(hb.to_script()))]);
                world.raise(hb, script::event::TOUCH_BEGIN, vec![Value::Object(Some(ha.to_script()))]);
            }
        }
        for &(a, b) in self.touching.difference(&now) {
            if let (Some(ha), Some(hb)) = (world.handle(a), world.handle(b)) {
                world.raise(ha, script::event::TOUCH_END, vec![Value::Object(Some(hb.to_script()))]);
                world.raise(hb, script::event::TOUCH_END, vec![Value::Object(Some(ha.to_script()))]);
            }
        }
        self.touching = now;
    }

    fn queries(&self) -> &dyn CollisionQueries {
        self
    }

    fn exclude(&mut self, world: &World, root: Handle) {
        let mut stack = vec![root];
        while let Some(h) = stack.pop() {
            self.excluded.insert(h.index());
            stack.extend(world.children(h));
        }
        self.sync_kinematic(world, 0.0);
    }
}

impl CollisionQueries for PhysicsBridge {
    fn point_in_solid(&self, world: &World, solid: Handle, point: [f32; 3]) -> Option<bool> {
        let b = self.body_of(world, solid)?;
        let body = self.cw.body(b);
        let inv = crate::math::affine_inverse(&body.transform)?;
        Some(self.cw.shapes[body.shape.0 as usize].main.point_inside(inv.transform_point(point)))
    }

    fn sphere_touches(&self, world: &World, solid: Handle, centre: [f32; 3], radius: f32) -> Option<bool> {
        let b = self.body_of(world, solid)?;
        let mut out = Vec::new();
        self.cw.sphere_overlaps(centre, radius, &QueryFilter::default(), &mut out);
        Some(out.iter().any(|o| o.body == b))
    }

    fn sweep_sphere(
        &self,
        world: &World,
        from: [f32; 3],
        to: [f32; 3],
        radius: f32,
        ignore: Handle,
    ) -> Option<SweepHit> {
        let delta = crate::math::sub(to, from);
        let h = self.cw.sphere_sweep(from, radius, delta, &self.filter(world, Some(ignore)))?;
        let object = self.handle_of_body(world, h.body)?;
        Some(SweepHit { t: h.fraction, point: h.point, normal: h.normal, object })
    }

    fn overlap_sphere(&self, world: &World, centre: [f32; 3], radius: f32, out: &mut Vec<Handle>) -> bool {
        let mut v = Vec::new();
        self.cw.sphere_overlaps(centre, radius, &QueryFilter::default(), &mut v);
        for o in v {
            if let Some(h) = self.handle_of_body(world, o.body)
                && !out.contains(&h)
            {
                out.push(h);
            }
        }
        true
    }

    fn line_clear(&self, world: &World, from: [f32; 3], to: [f32; 3], ignore: &[Handle]) -> Option<bool> {
        // The collision world ignores one body per query: test again past each ignored hit.
        let dir = crate::math::sub(to, from);
        let len = crate::math::length(dir);
        if len <= 0.0 {
            return Some(true);
        }
        let d = crate::math::scale(dir, 1.0 / len);
        let mut start = from;
        let mut left = len;
        for _ in 0..8 {
            let Some(hit) = self.cw.ray_cast(start, d, left, &QueryFilter::default()) else { return Some(true) };
            let h = self.handle_of_body(world, hit.body);
            if !h.is_some_and(|h| ignore.contains(&h)) {
                return Some(false);
            }
            let step = hit.distance + 0.5;
            start = crate::math::madd(start, d, step);
            left -= step;
            if left <= 0.0 {
                return Some(true);
            }
        }
        Some(false)
    }

    fn ground(&self, world: &World, point: [f32; 3], max_drop: f32) -> Option<([f32; 3], [f32; 3], Option<Handle>)> {
        let g = self.cw.ground_probe(point, max_drop, &QueryFilter::default())?;
        Some((g.point, g.normal, self.handle_of_body(world, g.body)))
    }

    fn collision_world(&self) -> Option<&CollisionWorld> {
        Some(&self.cw)
    }

    fn object_of_body(&self, world: &World, body: BodyId) -> Option<Handle> {
        self.handle_of_body(world, body)
    }
}
