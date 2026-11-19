//! Effects (effects.md): particle emitters (class 80), explosions (120 to 122), lightning flashes (145), skies (84)
//! and the world darkening. Lights (81) are data for the renderer.
//!
//! Particles are simulated on the CPU into the world's particle pool (`World::shared.particles`): emitters add
//! them continuously, explosions in bursts. Each particle moves under its acceleration and damping
//! (`dv/dt = a + damping × v`) and dies at the end of its lifetime; its size and alpha follow the emitter's
//! quadratic curves over its normalised age. The render scene hands the live particles to the renderer.

use alloc::vec::Vec;

use formats::options::OptionList;
use formats::stream::Context;

use super::{ClassBehavior, Cx};
use crate::math::{add, madd, normalize, scale};
use crate::object::options::decode_list;
use crate::object::{Arg, Handle, OptionStore, OptionValue};
use crate::world::World;
use crate::world::shared::ScreenTint;

/// Emitter and explosion options (effects.md 1, 2).
pub mod opt {
    /// Emission rate range, particles per second.
    pub const RATE: u16 = 351;
    /// Constant population.
    pub const POPULATION: u16 = 352;
    /// Enabled.
    pub const ENABLED: u16 = 355;
    /// Group: emitters sharing a group OID emit together as one.
    pub const GROUP: u16 = 361;
    /// Script calls: pause for a time, pause forever, resume (effects.md 2).
    pub const PAUSE_FOR: u16 = 362;
    /// See [`PAUSE_FOR`].
    pub const PAUSE_FOREVER: u16 = 363;
    /// See [`PAUSE_FOR`].
    pub const RESUME: u16 = 364;
    /// Origin kind: 0 point, 1 box, 2 curve, 3 skeleton, 4 mesh.
    pub const ORIGIN: u16 = 366;
    /// Origin point.
    pub const POINT: u16 = 367;
    /// Box corners.
    pub const BOX1: u16 = 368;
    /// See [`BOX1`].
    pub const BOX2: u16 = 369;
    /// Fixed direction.
    pub const DIRECTION: u16 = 372;
    /// Velocity frame: 0 or 1 relative to the emitter, 2 and 3 world.
    pub const FRAME: u16 = 376;
    /// Speed range.
    pub const SPEED: u16 = 377;
    /// Acceleration.
    pub const ACCEL: u16 = 379;
    /// Damping.
    pub const DAMPING: u16 = 380;
    /// Tilt range.
    pub const TILT: u16 = 384;
    /// Lifetime.
    pub const LIFETIME: u16 = 387;
    /// Alpha curve.
    pub const ALPHA: u16 = 390;
    /// Shader OID.
    pub const SHADER: u16 = 398;
    /// Size curve.
    pub const SIZE: u16 = 399;
    /// Explosion particle count range.
    pub const BURST_COUNT: u16 = 440;
    /// Explosion delay.
    pub const BURST_DELAY: u16 = 444;
    /// Explosion lifetime.
    pub const BURST_LIFE: u16 = 451;
    /// Explosion speed range.
    pub const BURST_SPEED: u16 = 433;
    /// Explosion tilt range.
    pub const BURST_TILT: u16 = 435;
    /// Explosion sound.
    pub const BURST_SOUND: u16 = 477;
    /// Lightning: start, peak, end colours, fade in, fade out, hold, front.
    pub const FLASH_START: u16 = 1133;
    /// See [`FLASH_START`].
    pub const FLASH_PEAK: u16 = 1134;
    /// See [`FLASH_START`].
    pub const FLASH_END: u16 = 1135;
    /// See [`FLASH_START`].
    pub const FLASH_IN: u16 = 1136;
    /// See [`FLASH_START`].
    pub const FLASH_OUT: u16 = 1137;
    /// See [`FLASH_START`].
    pub const FLASH_HOLD: u16 = 1138;
    /// See [`FLASH_START`].
    pub const FLASH_FRONT: u16 = 1139;
    /// Lightning: start (action).
    pub const FLASH_GO: u16 = 1132;
    /// Sky: follow the camera in Z too.
    pub const SKY_Z: u16 = 608;
}

/// Most live particles in a world (a budget for the Switch; emitters skip emission beyond it).
pub const MAX_PARTICLES: usize = 8192;
/// Whether an emitter emits this frame (effects.md 1.2): one with no parent always does; one attached to a parent
/// only while the camera is within its maximum draw distance times max(1, k / field of view); emitters of a group
/// (option 361) emit when any member qualifies.
pub fn emits(world: &World, me: Handle) -> bool {
    let qualifies = |h: Handle| {
        if world.parent(h).is_none() {
            return true;
        }
        let Some(d) = world.max_draw_distance(h) else { return true };
        let s = &world.shared;
        let reach = d * (s.draw_distance_ratio / s.camera_fov.max(1e-3)).max(1.0);
        world.position(h).is_some_and(|p| super::dist_sq(p, s.camera_eye) <= reach * reach)
    };
    match world.options(me).and_then(|o| o.i16(opt::GROUP)) {
        Some(g) if g >= 0 => world
            .slots_of_class(80)
            .iter()
            .filter_map(|&s| world.handle(s))
            .any(|h| (h == me || world.options(h).and_then(|o| o.i16(opt::GROUP)) == Some(g)) && qualifies(h)),
        _ => qualifies(me),
    }
}

/// One particle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    /// Position.
    pub position: [f32; 3],
    /// Velocity.
    pub velocity: [f32; 3],
    /// Age, seconds.
    pub age: f32,
    /// Lifetime, seconds.
    pub life: f32,
    /// Acceleration.
    pub accel: [f32; 3],
    /// Damping per second.
    pub damping: f32,
    /// Size curve (g0, g1, g2) over normalised age.
    pub size: [f32; 3],
    /// Alpha curve.
    pub alpha: [f32; 3],
    /// Colour RGBA 0..255 (alpha on the 0x80 = opaque scale).
    pub color: [u8; 4],
    /// Shader OID.
    pub shader: i16,
}

impl Particle {
    /// Size now (clamped at 0).
    pub fn size_now(&self) -> f32 {
        let u = (self.age / self.life.max(1e-6)).clamp(0.0, 1.0);
        (self.size[0] + self.size[1] * u + self.size[2] * u * u).max(0.0)
    }

    /// Alpha now (clamped at 0).
    pub fn alpha_now(&self) -> f32 {
        let u = (self.age / self.life.max(1e-6)).clamp(0.0, 1.0);
        (self.alpha[0] + self.alpha[1] * u + self.alpha[2] * u * u).max(0.0)
    }
}

/// Advances the world's particles by `dt`.
pub fn step_particles(particles: &mut Vec<Particle>, dt: f32) {
    for p in particles.iter_mut() {
        p.age += dt;
        let a = madd(p.accel, p.velocity, p.damping);
        p.velocity = madd(p.velocity, a, dt);
        p.position = madd(p.position, p.velocity, dt);
    }
    particles.retain(|p| p.age < p.life);
}

fn curve(o: &OptionStore, id: u16, default: [f32; 3]) -> [f32; 3] {
    match o.get(id) {
        Some(OptionValue::Quad(q)) => [q[0], q[1], q[2]],
        Some(OptionValue::Vector(v)) => *v,
        _ => default,
    }
}

/// A random direction tilted from `dir` by an angle drawn from `tilt`.
fn tilted(cx_rng: &mut crate::game::rng::Rng, dir: [f32; 3], tilt: [f32; 2]) -> [f32; 3] {
    let a = cx_rng.range(tilt[0], tilt[1].max(tilt[0]));
    if a == 0.0 {
        return dir;
    }
    let side = normalize(crate::math::cross(dir, if dir[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] }));
    let spin = crate::world::rotation_from_vector(scale(dir, cx_rng.range(0.0, core::f32::consts::TAU)));
    let side = crate::math::mat_mul_vec(&spin, side);
    let r = crate::world::rotation_from_vector(scale(side, a));
    crate::math::mat_mul_vec(&r, dir)
}

/// One new particle from an option set (an emitter's, or an explosion's with the burst options).
fn spawn(rng: &mut crate::game::rng::Rng, o: &OptionStore, w: &formats::geometry::Affine, burst: bool) -> Particle {
    let origin = match o.i32(opt::ORIGIN).unwrap_or(0) {
        1 => {
            let (a, b) = (o.vec3(opt::BOX1).unwrap_or([0.0; 3]), o.vec3(opt::BOX2).unwrap_or([0.0; 3]));
            [rng.range(a[0], b[0]), rng.range(a[1], b[1]), rng.range(a[2], b[2])]
        }
        _ => o.vec3(opt::POINT).unwrap_or([0.0; 3]),
    };
    let position = w.transform_point(origin);
    let dir = normalize(w.transform_vector(o.vec3(opt::DIRECTION).unwrap_or([1.0, 0.0, 0.0])));
    let (speed_id, tilt_id, life_id) = if burst {
        (opt::BURST_SPEED, opt::BURST_TILT, opt::BURST_LIFE)
    } else {
        (opt::SPEED, opt::TILT, opt::LIFETIME)
    };
    let [s0, s1] = o.pair(speed_id).unwrap_or([750.0, 750.0]);
    let dir = tilted(rng, dir, o.pair(tilt_id).unwrap_or([0.0, 0.0]));
    Particle {
        position,
        velocity: scale(dir, rng.range(s0, s1.max(s0))),
        age: 0.0,
        life: o.f32_or(life_id, 1.0).max(0.01),
        accel: o.vec3(opt::ACCEL).unwrap_or([0.0, 0.0, -1500.0]),
        damping: o.f32_or(opt::DAMPING, -1.0),
        size: curve(o, opt::SIZE, [1.0, 0.0, 0.0]),
        alpha: curve(o, opt::ALPHA, [1.0, 0.0, -1.0]),
        color: [255, 255, 255, 128],
        shader: o.i16(opt::SHADER).unwrap_or(-1),
    }
}

/// A particle emitter (class 80).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Emitter {
    /// Fraction of a particle carried to the next frame.
    pub carry: f32,
    /// Current rate.
    pub rate: f32,
    /// Seconds until the rate is drawn again.
    pub redraw: f32,
    /// Paused by a script: seconds left (infinite: until resumed).
    pub paused: Option<f32>,
}

impl ClassBehavior for Emitter {
    fn ticks(&self) -> bool {
        true
    }

    fn action(&mut self, _world: &mut World, _me: Handle, id: u16, args: &[Arg]) -> bool {
        match id {
            opt::PAUSE_FOR => {
                let t = args.iter().find_map(|a| match *a {
                    Arg::Float(f) => Some(f),
                    Arg::Int(n) => Some(n as f32),
                    _ => None,
                });
                self.paused = Some(t.unwrap_or(0.0).max(0.0));
                true
            }
            opt::PAUSE_FOREVER => {
                self.paused = Some(f32::INFINITY);
                true
            }
            opt::RESUME => {
                self.paused = None;
                true
            }
            _ => false,
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if let Some(t) = self.paused.as_mut() {
            *t -= cx.dt;
            if *t > 0.0 {
                return;
            }
            self.paused = None;
        }
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        if !o.bool_or(opt::ENABLED, true) || cx.world.shared.particles.len() >= MAX_PARTICLES {
            return;
        }
        let Some(w) = cx.world.world_transform(cx.me) else { return };
        if !emits(cx.world, cx.me) {
            return;
        }
        self.redraw -= cx.dt;
        if self.redraw <= 0.0 {
            let [a, b] = o.pair(opt::RATE).unwrap_or([10.0, 10.0]);
            self.rate = cx.rng.range(a, b.max(a));
            self.redraw = 1.0;
        }
        let n = if let Some(pop) = o.i32(opt::POPULATION).filter(|&p| p > 0) {
            // Constant population is not tracked per emitter; top up at the rate instead.
            (pop as f32 * cx.dt).min(64.0)
        } else {
            self.rate * cx.dt
        };
        self.carry += n;
        let k = self.carry as usize;
        self.carry -= k as f32;
        for _ in 0..k.min(64) {
            let p = spawn(cx.rng, &o, &w, false);
            cx.world.shared.particles.push(p);
        }
    }
}

/// The options of every member of an explosion (a group's owned members, or the emitter itself).
fn explosion_members(world: &World, e: Handle) -> Vec<OptionStore> {
    let Some(rec) = world.record(e) else { return Vec::new() };
    let r = world.level.record(rec);
    let mut out = Vec::new();
    let to_store = |list: Option<&OptionList>| {
        let mut s = OptionStore::new();
        if let Some(l) = list {
            for (id, v) in decode_list(&world.level.bytes, l, &world.level.table) {
                s.push(id, v);
            }
        }
        s
    };
    if r.class == 120 {
        let end = r.subtree_end;
        for k in rec + 1..end {
            let m = world.level.record(k);
            if m.context == Context::GroupMember && matches!(m.class, 121 | 122) {
                out.push(to_store(m.options.as_ref()));
            }
        }
    } else {
        out.push(world.options(e).cloned().unwrap_or_default());
    }
    out
}

/// Sets off an explosion (a group or an emitter) at a position: a burst of particles per member and its sound.
pub fn set_off(world: &mut World, e: Handle, at: [f32; 3]) {
    let w = formats::geometry::Affine { cols: crate::math::MAT3_IDENTITY, t: at };
    let mut rng = crate::game::rng::Rng::new(world.shared.particles.len() as u64 ^ (e.index() as u64) << 20);
    for o in explosion_members(world, e) {
        let [a, b] = o.pair(opt::BURST_COUNT).unwrap_or([1.0, 1.0]);
        let n = rng.range(a, b.max(a)) as usize;
        for _ in 0..n.min(256) {
            if world.shared.particles.len() >= MAX_PARTICLES {
                break;
            }
            let p = spawn(&mut rng, &o, &w, true);
            world.shared.particles.push(p);
        }
        if let Some(s) = o.i32(opt::BURST_SOUND) {
            world.play_sound(s, Some(e));
        }
    }
    world.raise(e, script::event::TRIGGERED, Vec::new());
}

/// An explosion object (classes 120 to 122): set off by scripts (its action options) or by what owns it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Explosion;

impl ClassBehavior for Explosion {
    fn action(&mut self, world: &mut World, me: Handle, id: u16, _args: &[Arg]) -> bool {
        if matches!(id, 481 | 482 | 489 | 490) {
            let at = world.position(me).unwrap_or([0.0; 3]);
            set_off(world, me, at);
            return true;
        }
        false
    }
}

/// A lightning flash (class 145, effects.md 4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lightning {
    /// Time since it started, while running.
    pub time: Option<f32>,
}

fn color(o: &OptionStore, id: u16) -> [f32; 4] {
    match o.get(id) {
        Some(OptionValue::Color(c)) => {
            [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, (c[3] as f32 / 128.0).min(1.0)]
        }
        _ => [1.0, 1.0, 1.0, 0.0],
    }
}

fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

impl Lightning {
    /// The colour at time `t`, or `None` when done.
    pub fn color_at(o: &OptionStore, t: f32) -> Option<[f32; 4]> {
        let (fi, hold, fo) =
            (o.f32_or(opt::FLASH_IN, 0.0), o.f32_or(opt::FLASH_HOLD, 0.0), o.f32_or(opt::FLASH_OUT, 0.0));
        let (s, p, e) = (color(o, opt::FLASH_START), color(o, opt::FLASH_PEAK), color(o, opt::FLASH_END));
        if t < fi {
            Some(mix(s, p, t / fi.max(1e-6)))
        } else if t < fi + hold {
            Some(p)
        } else if t < fi + hold + fo {
            Some(mix(p, e, (t - fi - hold) / fo.max(1e-6)))
        } else {
            None
        }
    }
}

impl ClassBehavior for Lightning {
    fn ticks(&self) -> bool {
        self.time.is_some()
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let Some(t) = self.time else { return };
        let t = t + cx.dt;
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        match Lightning::color_at(&o, t) {
            Some(c) => {
                cx.world.shared.screen.tints.push(ScreenTint { color: c, front: o.bool_or(opt::FLASH_FRONT, true) });
                self.time = Some(t);
            }
            None => self.time = None,
        }
    }

    fn action(&mut self, world: &mut World, _me: Handle, id: u16, _args: &[Arg]) -> bool {
        if id == opt::FLASH_GO {
            self.time = Some(0.0);
            let _ = world;
            return true;
        }
        false
    }
}

/// A sky (class 84): follows the camera in X and Y (and Z with option 608).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sky;

impl ClassBehavior for Sky {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let eye = cx.world.shared.camera_eye;
        let Some(mut w) = cx.world.world_transform(cx.me) else { return };
        w.t[0] = eye[0];
        w.t[1] = eye[1];
        if cx.world.options(cx.me).is_some_and(|o| o.bool_or(opt::SKY_Z, false)) {
            w.t[2] = eye[2];
        }
        cx.world.set_world_transform(cx.me, w);
        let _ = add;
    }
}
