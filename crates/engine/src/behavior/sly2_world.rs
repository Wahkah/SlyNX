//! Sly 2 effects (specs/sly2/effects.md 1) and the remaining interactables (interactables.md 5 to 7): emitters,
//! explosion groups and parts, barriers, water and hiding barrels.
//!
//! - **Emitters** (class 29, 1.2 to 1.5): the Sly 1 emitter with Sly 2 option numbers (rate 472, origin 483 to 486,
//!   direction 491, speed 496, tilt 502, lifetime 505, acceleration 498, damping 499, alpha 508, sprite shader 526
//!   and size 529), into the world's particle pool. An emitter emits while the camera is within its activation
//!   radius (option 13, default 3,000) unless a script stopped it (option 479).
//! - **Explosions** (classes 31, 32, 33; 1.6, 1.7): a group sets off every part below it; a part fires a burst of
//!   its count (555) with the part option numbers (536 to 588), then its sound (589). Groups and parts go off on a
//!   blast notice (props, bombs, scripts).
//! - **Barriers** (class 86, interactables.md 6): solids that only some objects collide with. Mode 0: the listed
//!   objects and classes (456, 457) collide, others pass; mode 1 the reverse. [`barrier_blocks_characters`] tells the
//!   gang's collision whether the playable characters (class family 51 and 52, and 53 to 55) collide with it.
//! - **Water** (class 89, 7): its collision parts are sensors (not solid for the gang). A character entering it
//!   while falling faster than 100 units/s splashes (sound 19, the splash explosion 874); a water whose contact
//!   value (876) is −1000 or less is lethal for the characters (`World::shared.gang_hits`).
//! - **Hiding barrels** (class 88, 5): a hit makes noise 706, knocks the barrel over (state 3) and raises script
//!   event 11. Getting in and out, and carrying the barrel, belong to the gang (characters.md).
//!
//! For the Sly 3 behaviours, which alias these: the barrier rule ([`barrier_blocks_characters`]), the water rule
//! ([`Water`]) and the barrel's hit reaction ([`Barrel`]) take no Sly 2-only data besides the option numbers above.

use alloc::vec::Vec;

use formats::geometry::Affine;

use super::effects::{MAX_PARTICLES, Particle};
use super::sly2_sensors::{CHARACTERS, GangHit, characters};
use super::{ClassBehavior, Cx};
use crate::math::{normalize, scale};
use crate::object::{Arg, Handle, OptionStore, OptionValue};
use crate::world::{Notice, World};

/// The option numbers of one emitter record set (class 29, or the explosion parts 32 and 33).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ids {
    /// Origin kind, point, box corners.
    pub origin: u16,
    /// See [`Ids::origin`].
    pub point: u16,
    /// See [`Ids::origin`].
    pub box1: u16,
    /// See [`Ids::origin`].
    pub box2: u16,
    /// Fixed direction.
    pub direction: u16,
    /// Speed range, tilt range, lifetime.
    pub speed: u16,
    /// See [`Ids::speed`].
    pub tilt: u16,
    /// See [`Ids::speed`].
    pub life: u16,
    /// Acceleration, damping.
    pub accel: u16,
    /// See [`Ids::accel`].
    pub damping: u16,
    /// Alpha and size curves, sprite shader.
    pub alpha: u16,
    /// See [`Ids::alpha`].
    pub size: u16,
    /// See [`Ids::alpha`].
    pub shader: u16,
}

/// Class 29's options.
pub const EMITTER_IDS: Ids = Ids {
    origin: 483,
    point: 484,
    box1: 485,
    box2: 486,
    direction: 491,
    speed: 496,
    tilt: 502,
    life: 505,
    accel: 498,
    damping: 499,
    alpha: 508,
    size: 529,
    shader: 526,
};

/// The explosion parts' options (classes 32 and 33).
pub const PART_IDS: Ids = Ids {
    origin: 536,
    point: 537,
    box1: 538,
    box2: 539,
    direction: 543,
    speed: 548,
    tilt: 550,
    life: 562,
    accel: 558,
    damping: 559,
    alpha: 564,
    size: 584,
    shader: 581,
};

/// Option IDs.
pub mod opt {
    /// Emission rate range (class 29).
    pub const RATE: u16 = 472;
    /// Stop emitting (class 29 action).
    pub const STOP: u16 = 479;
    /// Activation radius.
    pub const ACTIVATION: u16 = 13;
    /// Explosion parts: count per detonation, sound.
    pub const COUNT: u16 = 555;
    /// See [`COUNT`].
    pub const SOUND: u16 = 589;
    /// Barrier lists and mode.
    pub const BARRIER_OBJECTS: u16 = 456;
    /// See [`BARRIER_OBJECTS`].
    pub const BARRIER_CLASSES: u16 = 457;
    /// See [`BARRIER_OBJECTS`].
    pub const BARRIER_MODE: u16 = 458;
    /// Water: splash explosion, contact value.
    pub const SPLASH: u16 = 874;
    /// See [`SPLASH`].
    pub const CONTACT: u16 = 876;
}

/// Default activation radius of an emitter.
pub const ACTIVATION_RADIUS: f32 = 3000.0;
/// Water: the splash sound, the speed that makes one, and a lethal contact value.
pub const SPLASH: (i32, f32, f32) = (19, 100.0, -1000.0);
/// Noise template of a hit barrel, and its fallen state and event.
pub const BARREL_NOISE: i32 = 706;

fn curve(o: &OptionStore, id: u16, default: [f32; 3]) -> [f32; 3] {
    match o.get(id) {
        Some(OptionValue::Quad(q)) => [q[0], q[1], q[2]],
        Some(OptionValue::Vector(v)) => *v,
        _ => default,
    }
}

/// One particle from an option set with the given numbers, placed by `w`.
pub fn spawn(rng: &mut crate::game::rng::Rng, o: &OptionStore, ids: &Ids, w: &Affine) -> Particle {
    let origin = match o.i32(ids.origin).unwrap_or(0) {
        1 => {
            let (a, b) = (o.vec3(ids.box1).unwrap_or([0.0; 3]), o.vec3(ids.box2).unwrap_or([0.0; 3]));
            [rng.range(a[0], b[0]), rng.range(a[1], b[1]), rng.range(a[2], b[2])]
        }
        _ => o.vec3(ids.point).unwrap_or([0.0; 3]),
    };
    let dir = normalize(w.transform_vector(o.vec3(ids.direction).unwrap_or([1.0, 0.0, 0.0])));
    let [t0, t1] = o.pair(ids.tilt).unwrap_or([0.0, 0.0]);
    let tilt = rng.range(t0, t1.max(t0));
    let dir = if tilt != 0.0 {
        let side =
            normalize(crate::math::cross(dir, if dir[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] }));
        let spin = crate::world::rotation_from_vector(scale(dir, rng.range(0.0, core::f32::consts::TAU)));
        let side = crate::math::mat_mul_vec(&spin, side);
        crate::math::mat_mul_vec(&crate::world::rotation_from_vector(scale(side, tilt)), dir)
    } else {
        dir
    };
    let [s0, s1] = o.pair(ids.speed).unwrap_or([750.0, 750.0]);
    Particle {
        position: w.transform_point(origin),
        velocity: scale(dir, rng.range(s0, s1.max(s0))),
        age: 0.0,
        life: o.f32_or(ids.life, 1.0).max(0.01),
        accel: o.vec3(ids.accel).unwrap_or([0.0, 0.0, -1500.0]),
        damping: o.f32_or(ids.damping, -1.0),
        size: curve(o, ids.size, [1.0, 0.0, 0.0]),
        alpha: curve(o, ids.alpha, [1.0, 0.0, -1.0]),
        color: [255, 255, 255, 128],
        shader: o.i32(ids.shader).map_or(-1, |s| s as i16),
    }
}

/// A Sly 2 continuous emitter (class 29).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Emitter {
    /// Fraction of a particle carried over.
    pub carry: f32,
    /// Current rate.
    pub rate: f32,
    /// Seconds until the rate is drawn again.
    pub redraw: f32,
    /// Stopped by a script (479).
    pub stopped: bool,
}

impl ClassBehavior for Emitter {
    fn ticks(&self) -> bool {
        !self.stopped
    }

    fn action(&mut self, _world: &mut World, _me: Handle, id: u16, _args: &[Arg]) -> bool {
        if id == opt::STOP {
            self.stopped = true;
            return true;
        }
        false
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if self.stopped || cx.world.shared.particles.len() >= MAX_PARTICLES {
            return;
        }
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        let Some(w) = cx.world.world_transform(cx.me) else { return };
        let reach = o.f32_or(opt::ACTIVATION, ACTIVATION_RADIUS);
        if super::dist_sq(w.t, cx.world.shared.camera_eye) > reach * reach {
            return;
        }
        self.redraw -= cx.dt;
        if self.redraw <= 0.0 {
            let [a, b] = o.pair(opt::RATE).unwrap_or([10.0, 10.0]);
            self.rate = cx.rng.range(a, b.max(a));
            self.redraw = 1.0;
        }
        self.carry += self.rate * cx.dt;
        while self.carry >= 1.0 && cx.world.shared.particles.len() < MAX_PARTICLES {
            self.carry -= 1.0;
            let p = spawn(cx.rng, &o, &EMITTER_IDS, &w);
            cx.world.shared.particles.push(p);
        }
    }
}

/// Fires one explosion part's burst and sound.
pub fn fire_part(world: &mut World, part: Handle, rng: &mut crate::game::rng::Rng) {
    let Some(o) = world.options(part).cloned() else { return };
    let w = world.world_transform(part).unwrap_or(Affine::IDENTITY);
    let [a, b] = o.pair(opt::COUNT).unwrap_or([1.0, 1.0]);
    let n = rng.range(a, b.max(a)) as usize;
    for _ in 0..n.min(256) {
        if world.shared.particles.len() >= MAX_PARTICLES {
            break;
        }
        let p = spawn(rng, &o, &PART_IDS, &w);
        world.shared.particles.push(p);
    }
    if let Some(s) = o.i32(opt::SOUND) {
        world.play_sound(s, Some(part));
    }
}

/// An explosion group (31) or part (32, 33): goes off on a blast notice.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Explosion {
    /// Times set off.
    pub fired: u32,
}

impl ClassBehavior for Explosion {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let blasts = cx.world.notices(cx.me).iter().filter(|n| matches!(n, Notice::Blast(..))).count();
        for _ in 0..blasts {
            self.fired += 1;
            if cx.world.class(cx.me) == Some(31) {
                // Every part below the group.
                let mut stack: Vec<Handle> = cx.world.children(cx.me).collect();
                while let Some(h) = stack.pop() {
                    if matches!(cx.world.class(h), Some(32 | 33)) {
                        fire_part(cx.world, h, cx.rng);
                    }
                    stack.extend(cx.world.children(h));
                }
            } else {
                fire_part(cx.world, cx.me, cx.rng);
            }
        }
    }
}

/// Whether the playable characters collide with barrier `h` (class 86).
pub fn barrier_blocks_characters(world: &World, h: Handle) -> bool {
    let o = world.options(h);
    let mode = o.and_then(|o| o.i32(opt::BARRIER_MODE)).unwrap_or(0);
    let classes: Vec<i32> =
        o.map(|o| o.all(opt::BARRIER_CLASSES).filter_map(|v| v.as_i32()).collect()).unwrap_or_default();
    let ids: Vec<i32> = o.map(|o| o.all(opt::BARRIER_OBJECTS).filter_map(|v| v.as_i32()).collect()).unwrap_or_default();
    let characters = super::sly3::character_classes(world); // Sly 3 hook: Sly 3's character classes
    let listed = classes.iter().any(|&c| c == 51 || c == 52 || characters.contains(&(c as i16)))
        || ids.iter().any(|&id| matches!(id, 7..=9));
    if mode == 1 { !listed } else { listed }
}

/// Water (class 89).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Water {
    /// Characters inside last frame.
    pub inside: Vec<Handle>,
    /// Splashes made.
    pub splashes: u32,
}

impl ClassBehavior for Water {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let Some(shape) = water_box(cx.world, cx.me) else { return };
        let now: Vec<(Handle, [f32; 3])> =
            characters(cx.world).into_iter().filter(|&(_, c)| super::sly2_props::in_volume(&shape, c)).collect();
        let contact = cx.world.options(cx.me).and_then(|o| o.f32(opt::CONTACT)).unwrap_or(0.0);
        for &(h, c) in &now {
            if !self.inside.contains(&h) {
                let falling = cx.world.player == Some(h) && cx.world.shared.player.velocity[2] < -SPLASH.1;
                if falling {
                    self.splashes += 1;
                    cx.world.play_sound(SPLASH.0, Some(cx.me));
                    if let Some(e) = cx.world.options(cx.me).and_then(|o| o.i32(opt::SPLASH))
                        && let Some(e) = cx.world.find_sly2_id(e, Some(cx.me))
                    {
                        cx.world.notify(e, Notice::Blast(cx.me, [0.0; 3]));
                    }
                }
            }
            if contact <= SPLASH.2 {
                cx.world.shared.gang_hits.push(GangHit { target: h, amount: -contact, kind: 0x2000, source: c });
            }
        }
        self.inside = now.into_iter().map(|(h, _)| h).collect();
    }
}

/// The box of a solid's collision points (water's sensor parts), in the solid's frame.
pub fn water_box(world: &World, h: Handle) -> Option<(Affine, [f32; 3], [f32; 3])> {
    if world.sly2.is_none() {
        return super::sly3::interactables::solid_box(world, h); // Sly 3 hook: Sly 3 water (class 101)
    }
    let r = world.sly2_record(h)?;
    let formats_sly2::stream::Body::Solid(s) = &r.body else { return None };
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for part in &s.collision {
        for p in &part.mesh.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    if lo[0] > hi[0] {
        return None;
    }
    Some((world.world_transform(h)?, lo, hi))
}

/// A hiding barrel (class 88).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Barrel {
    /// Knocked over (state 3).
    pub fallen: bool,
}

impl ClassBehavior for Barrel {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let hit = cx.world.notices(cx.me).iter().find_map(|n| match *n {
            Notice::Attack { by, .. } | Notice::Hit(by) => Some(by),
            _ => None,
        });
        if let Some(by) = hit
            && !self.fallen
        {
            self.fallen = true;
            let at = cx.my_position();
            super::sly2::make_noise(cx.world, BARREL_NOISE, at, Some(by), None, cx.time);
            cx.raise(11, Vec::new());
        }
    }
}

/// The sound manager (class 152): script action 146 queues a music stream by its number (options-by-class.md 2.1;
/// audio.md 3). The request goes to `World::shared.music_requests` for the game's sound; the script gets no handle
/// back yet (the stream is queued after the frame).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SoundManager {
    /// Streams queued.
    pub queued: u32,
}

/// Option 146: queue a music stream.
pub const QUEUE_MUSIC: u16 = 146;

impl ClassBehavior for SoundManager {
    fn action(&mut self, world: &mut World, _me: Handle, id: u16, args: &[Arg]) -> bool {
        if id != QUEUE_MUSIC {
            return false;
        }
        let n = args.iter().find_map(|a| match *a {
            Arg::Int(n) => Some(n as i16),
            Arg::Short(_, n) => Some(n),
            Arg::Float(f) => Some(f as i16),
            _ => None,
        });
        if let Some(n) = n {
            world.shared.music_requests.push(n);
            self.queued += 1;
        }
        true
    }
}

/// Thrown-object options (interactables.md 8).
pub mod thrown {
    /// Impact radius against objects (default 200) and against the playable characters (default 200).
    pub const RADIUS: u16 = 383;
    /// See [`RADIUS`].
    pub const PLAYER_RADIUS: u16 = 384;
    /// Flight kind: 0 ends on contact, 1 sticks to what it hits.
    pub const KIND: u16 = 385;
    /// No hits at the end.
    pub const DUD: u16 = 387;
    /// Contact value of the hits (default −10).
    pub const CONTACT: u16 = 395;
    /// Noise at impact (default 1).
    pub const NOISE: u16 = 396;
    /// Defaults: radius, push away from the impact (interactables.md 8), contact value, the impact noise template.
    pub const DEFAULTS: (f32, f32, f32, i32) = (200.0, 500.0, -10.0, 709);
    /// Contact sphere of a thrown object in flight. *Placeholder*: the object's own collision is not used yet.
    pub const FLIGHT_RADIUS: f32 = 20.0;
}

/// A thrown object (class 79; also darts, bullets and missiles, 81 and 82): flies under the physics bridge once it is
/// in the world, and at the end of its flight (its first contact) hits what is near, pushes it, makes noise 709, raises
/// script event 5 and leaves the world. With flight kind 1 it sticks instead (it stops where it hit).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Thrown {
    /// Flying (handed to the physics).
    pub flying: bool,
    /// The flight ended.
    pub ended: bool,
}

impl ClassBehavior for Thrown {
    fn ticks(&self) -> bool {
        !self.ended
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if self.ended || !cx.world.in_world(cx.me) {
            return;
        }
        if !self.flying {
            // Launched by whatever added it to the world: it starts with the velocity it was given (none here:
            // launch velocities belong to the launchers' scripts) and falls.
            self.flying = true;
            cx.world.physics.push(crate::world::PhysicsCommand::Dynamic {
                object: cx.me,
                velocity: [0.0; 3],
                angular: [0.0; 3],
                radius: Some(thrown::FLIGHT_RADIUS),
            });
            return;
        }
        let contact = cx.world.notices(cx.me).iter().any(|n| matches!(n, Notice::Touched(_) | Notice::Impact(..)));
        if !contact {
            return;
        }
        self.ended = true;
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        let at = cx.my_position();
        if o.i32(thrown::KIND) == Some(1) {
            cx.world.physics.push(crate::world::PhysicsCommand::Release(cx.me));
            return;
        }
        let (r_default, push, contact_default, noise) = thrown::DEFAULTS;
        if !o.bool_or(thrown::DUD, false) {
            let value = o.f32_or(thrown::CONTACT, contact_default);
            let pr = o.f32_or(thrown::PLAYER_RADIUS, r_default);
            for (h, c) in characters(cx.world) {
                if pr > 0.0 && super::dist_sq(c, at) <= pr * pr {
                    cx.world.shared.gang_hits.push(GangHit { target: h, amount: -value, kind: 0x100, source: at });
                }
            }
            let r = o.f32_or(thrown::RADIUS, r_default);
            let mut near = Vec::new();
            if !cx.collision.overlap_sphere(cx.world, at, r, &mut near) {
                near = cx
                    .world
                    .handles()
                    .filter(|&h| cx.world.position(h).is_some_and(|p| super::dist_sq(p, at) <= r * r))
                    .collect();
            }
            for h in near {
                if h == cx.me || cx.world.class(h).is_some_and(|c| CHARACTERS.contains(&c)) {
                    continue;
                }
                let dir = cx.world.position(h).map_or([0.0, 0.0, 1.0], |p| normalize(crate::math::sub(p, at)));
                cx.world.notify(h, Notice::Blast(cx.me, scale(dir, push)));
            }
        }
        if o.bool_or(thrown::NOISE, true) {
            super::sly2::make_noise(cx.world, noise, at, Some(cx.me), None, cx.time);
        }
        cx.world.physics.push(crate::world::PhysicsCommand::Release(cx.me));
        cx.raise(5, Vec::new());
        cx.world.set_in_world(cx.me, false);
    }
}
