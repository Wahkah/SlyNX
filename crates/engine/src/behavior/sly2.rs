//! Sly 2 behaviours (specs/sly2), attached to Sly 2 worlds by [`attach`]. First: guards and swarming creatures
//! (guards.md), with the noises they hear (section 4) and the pockets they carry (section 8).
//!
//! **Guards** (classes 58 guard, 57 swarming creature). One job runs at a time (section 2):
//!
//! - **patrol**: stands at its post (routes, option 273, are not followed yet);
//! - **investigate** (4.2): walks to the noise it accepted, looks for 5.5 to 6.5 s (swarmers search after 2 s), then
//!   goes home;
//! - **pursue** (6): while the player is seen, or lost for less than 5 s: closer than the far range (option 293,
//!   default 1,200) it closes in, within the near range (292, default 800) it stops and attacks; it raises an alarm
//!   noise (kind 4, template 710) at most once a second; 5 s without the target ends the pursuit;
//! - **act** (6.1): an attack (option 233's action objects: distance, height and angle ranges, wait after) hurts the
//!   player;
//! - **hit / knocked out** (7): hit points from option 254 (default 40); a hit it did not see coming within 0.2 s
//!   knocks out at once unless option 333 is set; option 232 ignores hits; knocked out, it forgets what it saw and
//!   spills half its pocket's coins (rounded up).
//!
//! **Sight** (3): the eye is 80 above the guard; the view volume (option 315's object) is taken as its box (the
//! guard volume's extents, 3) until the volume meshes are read; option 313 always sees, 314 never; option 312 bit 0
//! sees hidden players, bit 1 sees through solids. **Hearing** (4.1): noises reach by height range and a radius that
//! grows over their first second. **Pockets** (8): `gang::Pocket`, filled from the guard row of the executable's
//! table (`World::shared.pockets`) when the guard starts; a pickpocket takes the next entry (`Pocket::take`).
//!
//! Placeholders (marked): the walk and chase speeds (the animations' root motion moves Sly 2 guards, not read yet),
//! the damage of an attack, and the hit strength of the gang's attacks.

use alloc::boxed::Box;
use alloc::vec::Vec;

use formats::geometry::Affine;

use super::{Behavior, ClassBehavior, Cx};
use crate::gang::{Entry, Pocket};
use crate::math::{length, sub};
use crate::object::Handle;
use crate::world::shared::damage;
use crate::world::{Notice, World};

/// Option IDs (Sly 2 stream options, guards.md).
pub mod opt {
    /// Hit points.
    pub const HIT_POINTS: u16 = 254;
    /// Ignores hit damage.
    pub const IGNORE_HITS: u16 = 232;
    /// Attacks (action objects, *add*).
    pub const ACTIONS: u16 = 233;
    /// Near and far pursuit ranges.
    pub const NEAR: u16 = 292;
    /// See [`NEAR`].
    pub const FAR: u16 = 293;
    /// Detector flags: bit 0 sees hidden players, bit 1 sees through solids, bit 2 only level geometry blocks.
    pub const DETECT_FLAGS: u16 = 312;
    /// Always sees.
    pub const ALWAYS_SEE: u16 = 313;
    /// Never sees.
    pub const NEVER_SEE: u16 = 314;
    /// View volume.
    pub const VIEW: u16 = 315;
    /// Patrol route: a command list (class 113).
    pub const ROUTE: u16 = 273;
    /// Lowest noise kind it takes.
    pub const MIN_NOISE: u16 = 323;
    /// Home object.
    pub const HOME: u16 = 327;
    /// Not knocked out by surprise.
    pub const NO_SURPRISE: u16 = 333;
    /// Action objects: distance range, height range, wait after, angle (degrees), enabled.
    pub const ACT_DISTANCE: u16 = 353;
    /// See [`ACT_DISTANCE`].
    pub const ACT_HEIGHT: u16 = 354;
    /// See [`ACT_DISTANCE`].
    pub const ACT_WAIT: u16 = 355;
    /// See [`ACT_DISTANCE`].
    pub const ACT_ANGLE: u16 = 356;
    /// See [`ACT_DISTANCE`].
    pub const ACT_ENABLED: u16 = 363;
    /// Noise templates (class 117): perception, radius, height range, lifetime, kind.
    pub const NOISE_SEEN: u16 = 1090;
    /// See [`NOISE_SEEN`].
    pub const NOISE_RADIUS: u16 = 1091;
    /// See [`NOISE_SEEN`].
    pub const NOISE_HEIGHT: u16 = 1092;
    /// See [`NOISE_SEEN`].
    pub const NOISE_LIFETIME: u16 = 1093;
    /// See [`NOISE_SEEN`].
    pub const NOISE_KIND: u16 = 1094;
    /// Noise templates' method: make a noise (position, owner) and give it back.
    pub const NEW_NOISE: u16 = 1095;
}

/// Classes.
pub mod class {
    /// Swarming creature or pop-up enemy.
    pub const SWARMER: i16 = 57;
    /// Guard.
    pub const GUARD: i16 = 58;
}

/// Eye height above the guard (3).
pub const EYE_HEIGHT: f32 = 80.0;
/// The guard view volume's box in its own frame (3), and how far ahead of the guard it sits.
pub const VIEW_BOX: ([f32; 3], [f32; 3]) = ([-116.0, -661.0, -341.0], [934.0, 661.0, 315.0]);
/// See [`VIEW_BOX`].
pub const VIEW_AHEAD: f32 = 166.0;
/// A target record not refreshed this long ends the pursuit (6).
pub const LOST_AFTER: f32 = 5.0;
/// Alarms repeat at most this often (4.3).
pub const ALARM_REPEAT: f32 = 1.0;
/// A hit counts as a surprise when the attacker was not seen this recently (7).
pub const SURPRISE_WINDOW: f32 = 0.2;
/// Swarmers start searching this long into an investigation (4.2).
pub const SWARMER_SEARCH: f32 = 2.0;
/// Default pursuit ranges (6).
pub const DEFAULT_RANGES: (f32, f32) = (800.0, 1200.0);
/// Default attack ranges (6.1): distance, height, wait after, angle in degrees.
pub const DEFAULT_ATTACK: ([f32; 2], [f32; 2], [f32; 2], f32) = ([0.0, 300.0], [-5000.0, 5000.0], [0.2, 0.5], 10.0);
/// Walking and chasing speeds. *Placeholder*: Sly 2 guards move by their animations' root motion.
pub const WALK_SPEED: f32 = 250.0;
/// See [`WALK_SPEED`].
pub const CHASE_SPEED: f32 = 550.0;
/// Turning speed, radians per second. *Placeholder*, as [`WALK_SPEED`].
pub const TURN_RATE: f32 = 4.0;
/// A guard's strike (guards.md 6.3): every guard's striking part (tag 13056) sends knock-back (`0x200`) with value
/// −10, i.e. 10 health units.
pub const GUARD_STRIKE: (f32, u32) = (10.0, 0x200);
/// A swarmer's strike: most species bite for −3 with knock-back (guards.md 6.3; goose −5 and some parts −10 with other
/// kinds; the per-part values are in the collision parts, read once `formats-sly2` gives their contact fields).
pub const SWARMER_STRIKE: (f32, u32) = (3.0, 0x200);
/// Hit points one attack of the gang takes. *Placeholder*: characters.md does not give them.
pub const GANG_HIT: f32 = 20.0;

/// A noise (4): a short-lived point of interest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Noise {
    /// Where.
    pub position: [f32; 3],
    /// Who made it.
    pub owner: Option<Handle>,
    /// For alarms (kind 4): what the alarm is about.
    pub target: Option<Handle>,
    /// Kind.
    pub kind: i32,
    /// Full radius.
    pub radius: f32,
    /// Height range relative to the noise.
    pub height: [f32; 2],
    /// Lifetime in seconds (negative: permanent).
    pub lifetime: f32,
    /// Seen through the view volume rather than heard.
    pub seen: bool,
    /// Game time it was made at.
    pub born: f32,
}

impl Noise {
    /// Whether the noise reaches `p` at game time `now` (4.1): inside the height range, and within the radius, which
    /// grows over the first min(lifetime, 1) s from 15 %.
    pub fn reaches(&self, p: [f32; 3], now: f32) -> bool {
        let dz = p[2] - self.position[2];
        if dz < self.height[0] || dz > self.height[1] {
            return false;
        }
        let grow = if self.lifetime < 0.0 { 1.0 } else { self.lifetime.min(1.0) };
        let u = if grow > 0.0 { ((now - self.born) / grow).clamp(0.0, 1.0) } else { 1.0 };
        let r = self.radius * (0.15 + 1.1 * u - 0.25 * u * u);
        let d = sub(p, self.position);
        d[0] * d[0] + d[1] * d[1] <= r * r
    }

    /// Whether it is still alive at `now`.
    pub fn alive(&self, now: f32) -> bool {
        self.lifetime < 0.0 || now - self.born <= self.lifetime
    }
}

/// The fixed noise templates by object ID (4.1): radius, lifetime, seen, kind.
pub fn fixed_template(id: i32) -> Option<(f32, f32, bool, i32)> {
    Some(match id {
        706 => (1500.0, 0.25, false, 0),
        707 => (1500.0, -1.0, true, 0),
        708 => (750.0, 0.25, false, 0),
        709 => (2500.0, 0.35, false, 1),
        710 => (4000.0, 0.5, false, 4),
        711 => (1500.0, -1.0, false, 3),
        712 => (1000.0, 0.25, false, 0),
        _ => return None,
    })
}

/// Makes a noise from template `template` (a class-117 object's ID, or one of the fixed templates) at `position`.
pub fn make_noise(
    world: &mut World,
    template: i32,
    position: [f32; 3],
    owner: Option<Handle>,
    target: Option<Handle>,
    now: f32,
) {
    let (mut radius, mut lifetime, mut seen, mut kind) = fixed_template(template).unwrap_or((1500.0, 0.25, false, 0));
    let mut height = [-200.0, 500.0];
    if let Some(o) = world.find_sly2_id(template, None).and_then(|h| world.options(h)) {
        radius = o.f32(opt::NOISE_RADIUS).unwrap_or(radius);
        lifetime = o.f32(opt::NOISE_LIFETIME).unwrap_or(lifetime);
        seen = o.i32(opt::NOISE_SEEN).map_or(seen, |v| v == 1);
        kind = o.i32(opt::NOISE_KIND).unwrap_or(kind);
        height = o.pair(opt::NOISE_HEIGHT).unwrap_or(height);
    }
    let noises = &mut world.shared.noises;
    noises.retain(|n| n.alive(now));
    if noises.len() >= 64 {
        noises.remove(0);
    }
    noises.push(Noise { position, owner, target, kind, radius, height, lifetime, seen, born: now });
}

/// What a guard is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Job {
    /// At its post.
    Patrol,
    /// Going to a noise, then looking.
    Investigate {
        /// Where.
        at: [f32; 3],
        /// Seconds looking left (counted once there).
        look: f32,
        /// Seconds since it started.
        time: f32,
    },
    /// Chasing the player.
    Pursue,
    /// Going back home after giving up.
    Return,
    /// Knocked out.
    KnockedOut,
}

/// An attack (an action object, 6.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attack {
    /// Horizontal distance range.
    pub distance: [f32; 2],
    /// Height range.
    pub height: [f32; 2],
    /// Wait after, seconds.
    pub wait: [f32; 2],
    /// Half the angle within which the target must be, radians.
    pub half_angle: f32,
}

/// A Sly 2 guard or swarming creature.
#[derive(Clone, Debug, PartialEq)]
pub struct Sly2Guard {
    /// A swarming creature (class 57).
    pub swarmer: bool,
    /// Hit points left.
    pub hit_points: f32,
    /// The job.
    pub job: Job,
    /// The detector's state for the player: 0 unseen, 1 seen, 2 lost.
    pub sight: u8,
    /// Game time the player was last seen.
    pub last_seen: f32,
    /// Game time of the last alarm raised.
    pub last_alarm: f32,
    /// Seconds until it may attack again.
    pub attack_wait: f32,
    /// The pocket.
    pub pocket: Pocket,
    /// Its post (where it stood when the world started).
    pub post: [f32; 3],
    /// Heading, radians.
    pub yaw: f32,
    /// Attacks.
    pub attacks: Vec<Attack>,
    /// Hits that reached it (for tests and the HUD).
    pub hits_taken: u32,
    /// The command list it is running (class 113), if any.
    pub commands: Option<super::sly2_commands::CommandRun>,
    /// Its patrol route (option 273: a command list), run again whenever it is free and has no other list.
    pub route: Option<Handle>,
    /// Its view volume (option 315), resolved at load.
    pub view: Option<Handle>,
    /// Seconds of the current run time left (negative: pausing for up to `route_pause`).
    pub route_clock: f32,
    /// Length of the current pause time.
    pub route_pause: f32,
}

impl Sly2Guard {
    /// A guard for object `h`.
    pub fn new(world: &World, h: Handle) -> Sly2Guard {
        let o = world.options(h);
        let attacks: Vec<Attack> = o
            .map(|o| o.all(opt::ACTIONS).filter_map(|v| v.as_i32()).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| world.find_sly2_id(id, Some(h)))
            .filter_map(|a| world.options(a).map(attack_of))
            .flatten()
            .collect();
        let w = world.world_transform(h).unwrap_or(Affine::IDENTITY);
        Sly2Guard {
            swarmer: world.class(h) == Some(class::SWARMER),
            hit_points: o.and_then(|o| o.f32(opt::HIT_POINTS)).unwrap_or(40.0),
            job: Job::Patrol,
            sight: 0,
            last_seen: f32::NEG_INFINITY,
            last_alarm: f32::NEG_INFINITY,
            attack_wait: 0.0,
            pocket: Pocket::default(),
            post: w.t,
            yaw: crate::math::atan2(w.cols[0][1], w.cols[0][0]),
            attacks: if attacks.is_empty() { alloc::vec![default_attack()] } else { attacks },
            hits_taken: 0,
            commands: None,
            route: None,
            route_clock: 0.0,
            route_pause: 0.0,
            view: o.and_then(|o| o.i32(opt::VIEW)).and_then(|id| world.find_sly2_id(id, Some(h))),
        }
    }

    /// Knocked out.
    pub fn knocked_out(&self) -> bool {
        self.job == Job::KnockedOut
    }

    /// Whether the guard's detector sees `p` (3), the player's position.
    fn sees(&self, cx: &Cx<'_>, p: [f32; 3], who: Option<Handle>) -> bool {
        let o = cx.world.options(cx.me);
        if o.is_some_and(|o| o.bool_or(opt::NEVER_SEE, false)) {
            return false;
        }
        if o.is_some_and(|o| o.bool_or(opt::ALWAYS_SEE, false)) {
            return true;
        }
        let flags = o.and_then(|o| o.i32(opt::DETECT_FLAGS)).unwrap_or(0);
        if flags & 1 == 0 && cx.world.shared.player.hiding != crate::world::Hiding::None {
            return false;
        }
        let Some(me) = cx.world.position(cx.me) else { return false };
        let eye = [me[0], me[1], me[2] + EYE_HEIGHT];
        // The view volume's box, in the volume object's frame when it has one, else ahead of the guard.
        let view = self.view;
        // The view volume's own shape (the box of its collision points) when it can be read.
        if let Some(shape) = view.and_then(|v| super::sly2_props::volume_box(cx.world, v)) {
            if !super::sly2_props::in_volume(&shape, p) {
                return false;
            }
            if flags & 2 != 0 {
                return true;
            }
            let mut ignore = alloc::vec![cx.me];
            ignore.extend(who);
            return cx.collision.line_clear(cx.world, eye, p, &ignore).unwrap_or(true);
        }
        let frame = view.and_then(|v| cx.world.world_transform(v)).unwrap_or_else(|| {
            let (s, c) = (crate::math::sin(self.yaw), crate::math::cos(self.yaw));
            Affine {
                cols: [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]],
                t: [me[0] + c * VIEW_AHEAD, me[1] + s * VIEW_AHEAD, eye[2]],
            }
        });
        let local = to_local(&frame, p);
        let (lo, hi) = VIEW_BOX;
        if (0..3).any(|k| local[k] < lo[k] || local[k] > hi[k]) {
            return false;
        }
        if flags & 2 != 0 {
            return true;
        }
        let mut ignore = alloc::vec![cx.me];
        ignore.extend(who);
        cx.collision.line_clear(cx.world, eye, p, &ignore).unwrap_or(true)
    }

    fn face(&mut self, to: [f32; 3], from: [f32; 3], dt: f32) -> f32 {
        let want = crate::math::atan2(to[1] - from[1], to[0] - from[0]);
        let mut d = want - self.yaw;
        while d > core::f32::consts::PI {
            d -= 2.0 * core::f32::consts::PI;
        }
        while d < -core::f32::consts::PI {
            d += 2.0 * core::f32::consts::PI;
        }
        let step = TURN_RATE * dt;
        self.yaw += d.clamp(-step, step);
        d.abs()
    }

    /// Moves toward `to` at `speed`, stopping `stop` short; returns the horizontal distance left.
    fn walk(&mut self, cx: &mut Cx<'_>, to: [f32; 3], speed: f32, stop: f32) -> f32 {
        let Some(p) = cx.world.position(cx.me) else { return 0.0 };
        self.face(to, p, cx.dt);
        let d = [to[0] - p[0], to[1] - p[1]];
        let dist = libm_hypot(d[0], d[1]);
        let mut q = p;
        if dist > stop {
            let s = (speed * cx.dt).min(dist - stop) / dist.max(1e-6);
            q = [p[0] + d[0] * s, p[1] + d[1] * s, p[2]];
        }
        self.place(cx.world, cx.me, q);
        (dist - stop).max(0.0)
    }

    fn place(&self, world: &mut World, me: Handle, p: [f32; 3]) {
        let (s, c) = (crate::math::sin(self.yaw), crate::math::cos(self.yaw));
        world.set_world_transform(me, Affine { cols: [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]], t: p });
    }

    fn knock_out(&mut self, cx: &mut Cx<'_>) {
        self.job = Job::KnockedOut;
        self.hit_points = 0.0;
        self.sight = 0;
        // Spills half (rounded up) of the coins still in its pocket.
        let coins: u32 = self.pocket.entries.iter().map(|e| if let Entry::Coins(n) = e { *n } else { 0 }).sum();
        let spill = coins.div_ceil(2);
        if spill > 0 {
            let mut left = spill;
            for e in &mut self.pocket.entries {
                if let Entry::Coins(n) = e {
                    let take = (*n).min(left);
                    *n -= take;
                    left -= take;
                }
            }
            self.pocket.entries.retain(|e| *e != Entry::Coins(0));
            let at = cx.world.position(cx.me).unwrap_or([0.0; 3]);
            cx.world.shared.coin_drops.push((at, spill));
        }
        cx.world.shared.guards_knocked_out += 1;
    }

    fn take_notices(&mut self, cx: &mut Cx<'_>) {
        let notices: Vec<Notice> = cx.world.notices(cx.me).to_vec();
        let ignore_hits = cx.world.options(cx.me).is_some_and(|o| o.bool_or(opt::IGNORE_HITS, false));
        let no_surprise = cx.world.options(cx.me).is_some_and(|o| o.bool_or(opt::NO_SURPRISE, false));
        for n in notices {
            match n {
                Notice::Attack { .. } | Notice::Hit(_) | Notice::Blast(..) if !self.knocked_out() => {
                    self.hits_taken += 1;
                    if ignore_hits {
                        continue;
                    }
                    let surprise = cx.time - self.last_seen > SURPRISE_WINDOW && self.sight != 1;
                    if surprise && !no_surprise {
                        self.knock_out(cx);
                    } else {
                        self.hit_points = (self.hit_points - GANG_HIT).max(0.0);
                        if self.hit_points <= 0.0 {
                            self.knock_out(cx);
                        } else {
                            self.job = Job::Pursue;
                            self.last_seen = cx.time;
                        }
                    }
                }
                Notice::Pickpocket(_) => {
                    if let Some(e) = self.pocket.take() {
                        cx.world.shared.picked.push(e);
                    }
                }
                _ => {}
            }
        }
    }

    /// The noise this guard takes now, if any (4.2): kind at least its minimum, not its own, alarms only about the
    /// player; seen over heard, then higher kind, newer, nearer.
    fn pick_noise(&self, cx: &Cx<'_>, me: [f32; 3]) -> Option<[f32; 3]> {
        let min = cx.world.options(cx.me).and_then(|o| o.i32(opt::MIN_NOISE)).unwrap_or(-1);
        let player = cx.world.player;
        cx.world
            .shared
            .noises
            .iter()
            .filter(|n| n.alive(cx.time) && n.kind >= min && n.owner != Some(cx.me))
            .filter(|n| n.kind != 4 || n.target.is_some() && n.target == player)
            .filter(|n| n.reaches(me, cx.time))
            .max_by(|a, b| {
                (a.seen, a.kind, a.born)
                    .partial_cmp(&(b.seen, b.kind, b.born))
                    .unwrap_or(core::cmp::Ordering::Equal)
                    .then_with(|| dist2(b.position, me).total_cmp(&dist2(a.position, me)))
            })
            .map(|n| n.position)
    }
}

fn libm_hypot(x: f32, y: f32) -> f32 {
    length([x, y, 0.0])
}

fn dist2(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = sub(a, b);
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

fn to_local(frame: &Affine, p: [f32; 3]) -> [f32; 3] {
    let d = sub(p, frame.t);
    let c = &frame.cols;
    let n = |v: [f32; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).max(1e-12);
    [0, 1, 2].map(|k| (d[0] * c[k][0] + d[1] * c[k][1] + d[2] * c[k][2]) / n(c[k]))
}

fn default_attack() -> Attack {
    let (distance, height, wait, angle) = DEFAULT_ATTACK;
    Attack { distance, height, wait, half_angle: angle.to_radians() * 0.5 }
}

fn attack_of(o: &crate::object::OptionStore) -> Option<Attack> {
    if !o.bool_or(opt::ACT_ENABLED, true) {
        return None;
    }
    let d = default_attack();
    Some(Attack {
        distance: o.pair(opt::ACT_DISTANCE).unwrap_or(d.distance),
        height: o.pair(opt::ACT_HEIGHT).unwrap_or(d.height),
        wait: o.pair(opt::ACT_WAIT).unwrap_or(d.wait),
        half_angle: o.f32(opt::ACT_ANGLE).map_or(d.half_angle, |a| a.to_radians() * 0.5),
    })
}

impl ClassBehavior for Sly2Guard {
    fn ticks(&self) -> bool {
        true
    }

    /// Option 233 called by a script: adds an attack (an action object, by ID) to the guard (6.1).
    fn action(&mut self, world: &mut World, me: Handle, id: u16, args: &[crate::object::Arg]) -> bool {
        if id != opt::ACTIONS {
            return false;
        }
        for a in args {
            let oid = match *a {
                crate::object::Arg::Int(n) => n,
                crate::object::Arg::Short(_, n) => i32::from(n),
                _ => continue,
            };
            if let Some(at) = world.find_sly2_id(oid, Some(me)).and_then(|o| world.options(o)).and_then(attack_of)
                && self.attacks.len() < 8
            {
                self.attacks.push(at);
            }
        }
        true
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        // Patrol (5): option 273 names the guard's route, a command list (class 113) it runs while free.
        if let Some(list) = cx
            .world
            .options(cx.me)
            .and_then(|o| o.i32(opt::ROUTE))
            .and_then(|id| cx.world.find_sly2_id(id, Some(cx.me)))
            && cx.world.class(list) == Some(113)
        {
            self.route = Some(list);
        }
        let episode = super::sly3::pocket_row(cx.world, cx.world.shared.episode as usize); // Sly 3 hook: row e − 2
        if !self.swarmer
            && let Some(row) = cx.world.shared.pockets.as_ref().and_then(|p| p.row(episode)).copied()
        {
            self.pocket.fill(&row, cx.rng, |_| false);
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        self.take_notices(cx);
        if self.knocked_out() {
            return;
        }
        self.attack_wait = (self.attack_wait - cx.dt).max(0.0);
        let Some(me) = cx.world.position(cx.me) else { return };
        let player = cx.world.player_target();
        let target = player.and_then(|p| cx.world.position(p));
        // On patrol with no list running, it starts its route again.
        if self.commands.is_none()
            && self.job == Job::Patrol
            && let Some(list) = self.route
        {
            self.commands = Some(super::sly2_commands::CommandRun::new(list));
        }
        // A command list (its route, or one it was told to run) drives it while it is free (patrol, return),
        // alternating run and pause times (commands 1 and 2): standing still while a pause lasts.
        if matches!(self.job, Job::Patrol | Job::Return)
            && let Some(mut run) = self.commands.take()
        {
            self.route_clock -= cx.dt;
            if self.route_clock <= -self.route_pause {
                // A new run time, then a pause time, drawn from the list's ranges (no run range: no pauses).
                let draw =
                    |rng: &mut crate::game::rng::Rng, r: Option<[f32; 2]>| r.map(|[a, b]| rng.range(a, b.max(a)));
                self.route_clock = draw(cx.rng, run.run_range).unwrap_or(f32::INFINITY);
                self.route_pause = draw(cx.rng, run.pause_range).unwrap_or(0.0);
            }
            let pausing = self.route_clock < 0.0;
            let mut more = true;
            if !pausing {
                let arrived = run.wait < 0.0;
                run.wait = run.wait.max(0.0);
                let (step, go_on) = super::sly2_commands::step_commands(cx, &mut run, arrived);
                more = go_on;
                match step {
                    super::sly2_commands::Step::Look(p) => {
                        self.face(p, me, cx.dt);
                        self.place(cx.world, cx.me, me);
                    }
                    super::sly2_commands::Step::GoTo(p, speed) => {
                        if self.walk(cx, p, speed, 30.0) <= 0.0 {
                            // Arrived: the next step moves past the command.
                            run.wait = -1.0;
                        }
                    }
                    super::sly2_commands::Step::Idle => {}
                }
            }
            if more {
                self.commands = Some(run);
            }
        }
        // Sight (3).
        if let Some(p) = target {
            if self.sees(cx, p, player) {
                self.sight = 1;
                self.last_seen = cx.time;
                if self.job != Job::Pursue {
                    self.job = Job::Pursue;
                }
            } else if self.sight == 1 {
                self.sight = 2;
            }
        }
        match self.job {
            Job::Pursue => {
                let Some(p) = target.filter(|_| cx.time - self.last_seen <= LOST_AFTER) else {
                    // Lost: back to its post.
                    self.sight = 0;
                    self.job = Job::Return;
                    return;
                };
                if cx.time - self.last_alarm >= ALARM_REPEAT {
                    self.last_alarm = cx.time;
                    make_noise(cx.world, 710, me, Some(cx.me), player, cx.time);
                }
                let o = cx.world.options(cx.me);
                let near = o.and_then(|o| o.f32(opt::NEAR)).unwrap_or(DEFAULT_RANGES.0);
                let far = o.and_then(|o| o.f32(opt::FAR)).unwrap_or(DEFAULT_RANGES.1);
                let d = libm_hypot(p[0] - me[0], p[1] - me[1]);
                let stop = if d > far { near } else { near * 0.5 };
                self.walk(cx, p, CHASE_SPEED, stop.min(self.attacks.iter().map(|a| a.distance[1]).fold(0.0, f32::max)));
                // Attack (6.1) when an action's ranges hold.
                if self.attack_wait <= 0.0 {
                    let me = cx.world.position(cx.me).unwrap_or(me);
                    let d = libm_hypot(p[0] - me[0], p[1] - me[1]);
                    let dz = p[2] - me[2];
                    let off = {
                        let want = crate::math::atan2(p[1] - me[1], p[0] - me[0]);
                        let mut a = want - self.yaw;
                        while a > core::f32::consts::PI {
                            a -= 2.0 * core::f32::consts::PI;
                        }
                        while a < -core::f32::consts::PI {
                            a += 2.0 * core::f32::consts::PI;
                        }
                        a.abs()
                    };
                    let chosen = self.attacks.iter().copied().find(|a| {
                        d >= a.distance[0]
                            && d <= a.distance[1]
                            && dz >= a.height[0]
                            && dz <= a.height[1]
                            && off <= a.half_angle + 0.005 * d
                    });
                    match chosen {
                        Some(a) => {
                            // The strike's contact (guards.md 6.3): kind and value of the guard's striking part.
                            let (amount, kind) = if self.swarmer { SWARMER_STRIKE } else { GUARD_STRIKE };
                            if let Some(t) = player {
                                cx.world.shared.gang_hits.push(super::sly2_sensors::GangHit {
                                    target: t,
                                    amount,
                                    kind,
                                    source: me,
                                });
                            } else {
                                cx.world.hurt_player(cx.me, damage::BLUNT, None);
                            }
                            self.attack_wait = cx.rng.range(a.wait[0], a.wait[1].max(a.wait[0]));
                        }
                        None => self.attack_wait = 0.25,
                    }
                }
            }
            Job::Investigate { at, look, time } => {
                let time = time + cx.dt;
                let left = self.walk(cx, at, WALK_SPEED, 50.0);
                let look = if left <= 0.0 || (self.swarmer && time >= SWARMER_SEARCH) { look - cx.dt } else { look };
                self.job = if look <= 0.0 { Job::Return } else { Job::Investigate { at, look, time } };
            }
            Job::Return => {
                if self.walk(cx, self.post, WALK_SPEED, 10.0) <= 0.0 {
                    self.job = Job::Patrol;
                }
            }
            Job::Patrol => {}
            Job::KnockedOut => {}
        }
        // Hearing (4.2): only while free.
        if matches!(self.job, Job::Patrol | Job::Return)
            && let Some(at) = self.pick_noise(cx, me)
        {
            let look = cx.rng.range(5.5, 6.5);
            self.job = Job::Investigate { at, look, time: 0.0 };
        }
    }
}

/// A noise template (class 117, guards.md 4.1). Its method, option 1095 (scripts.md: script ID 214), makes a noise
/// from the template at a position, owned by an object (or nobody), and gives back the noise object. The noise the
/// guards hear goes to `World::shared.noises`; the Sly 2 script host turns the call's result into a new run-time
/// noise object (`Sly2Level::noise_objects`, destroyed when the noise expires) whose investigation point moves the
/// noise. Called without that host, the result is the template.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NoiseTemplate {
    /// Noises made from it.
    pub made: u32,
}

impl ClassBehavior for NoiseTemplate {
    fn action(&mut self, world: &mut World, me: Handle, id: u16, args: &[crate::object::Arg]) -> bool {
        if id != opt::NEW_NOISE {
            return false;
        }
        let at = args.iter().find_map(|a| match *a {
            crate::object::Arg::Vector(_, v) => Some(v),
            _ => None,
        });
        let owner = args.iter().find_map(|a| match *a {
            crate::object::Arg::Int(id) => world.find_sly2_id(id, Some(me)),
            crate::object::Arg::Short(6, oid) if oid >= 0 => world.find_nearest(oid, Some(me)),
            _ => None,
        });
        let at = at.or_else(|| world.position(me)).unwrap_or([0.0; 3]);
        let template = world.sly2_id(me).unwrap_or(-1);
        let now = world.shared.time;
        make_noise(world, template, at, owner, None, now);
        self.made += 1;
        world.action_result = Some(me);
        true
    }
}

/// The class of noise templates.
pub const NOISE_TEMPLATE_CLASS: i16 = 117;

/// The behaviour of a Sly 2 object, from its class.
pub fn for_class(world: &World, h: Handle) -> Behavior {
    match world.class(h) {
        Some(class::GUARD | class::SWARMER) => Behavior::Sly2Guard(Box::new(Sly2Guard::new(world, h))),
        Some(NOISE_TEMPLATE_CLASS) => Behavior::Sly2Noise(Box::default()),
        Some(85) => Behavior::Sly2Laser(Box::new(super::sly2_sensors::Laser::new(world, h))),
        Some(83) => Behavior::Sly2Pad(Box::default()),
        Some(16) => Behavior::Sly2Detector(Box::new(super::sly2_sensors::Detector::new(world, h))),
        Some(98) => Behavior::Sly2Prop(Box::new(super::sly2_props::Prop::new(world, h))),
        Some(114) => Behavior::Sly2Sequence(Box::default()),
        Some(29) => Behavior::Sly2Emitter(Box::default()),
        Some(31..=33) => Behavior::Sly2Explosion(Box::default()),
        Some(89) => Behavior::Sly2Water(Box::default()),
        Some(88) => Behavior::Sly2Barrel(Box::default()),
        Some(79 | 81 | 82) => Behavior::Sly2Thrown(Box::default()),
        Some(152) => Behavior::Sly2SoundManager(Box::default()),
        Some(113) => Behavior::Sly2Commands(Box::new(super::sly2_commands::CommandList::new(world, h))),
        Some(24 | 25) => Behavior::Sly2Volume(Box::default()),
        Some(c @ 91..=95) => super::sly2_pickups::Sly2Pickup::new(world, h, c)
            .map_or(Behavior::None, |p| Behavior::Sly2Pickup(Box::new(p))),
        _ => Behavior::None,
    }
}

/// Attaches the Sly 2 behaviours to every object of a Sly 2 world (in the world, not removed at start).
pub fn attach(world: &mut World) {
    if world.sly2.is_none() {
        return;
    }
    let handles: Vec<Handle> = world.handles().collect();
    for h in handles {
        let b = for_class(world, h);
        if !matches!(b, Behavior::None) {
            world.set_behavior(h, b);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::progress::Progress;
    use crate::game::systems::{FrameInput, Systems};
    use crate::game::{Game, GameConfig};
    use crate::world::WorldInfo;
    use crate::world::tests::fixture::{Fixture, at};

    #[test]
    fn noises_grow_and_expire() {
        let n = Noise {
            position: [0.0; 3],
            owner: None,
            target: None,
            kind: 0,
            radius: 1000.0,
            height: [-200.0, 500.0],
            lifetime: 0.5,
            seen: false,
            born: 0.0,
        };
        assert!(n.reaches([140.0, 0.0, 0.0], 0.0) && !n.reaches([160.0, 0.0, 0.0], 0.0), "15 % at birth");
        assert!(n.reaches([990.0, 0.0, 0.0], 0.5), "full radius after min(lifetime, 1)");
        assert!(!n.reaches([0.0, 0.0, 600.0], 0.5), "outside the height range");
        assert!(n.alive(0.5) && !n.alive(0.6));
        assert_eq!(fixed_template(710), Some((4000.0, 0.5, false, 4)));
    }

    fn guard_game() -> (Game, Handle, Handle) {
        let mut f = Fixture::new();
        f.top(58, 50, at(0.0, 0.0, 0.0));
        f.top(7, 51, at(2000.0, 0.0, 0.0));
        let world = f.world(WorldInfo::default());
        let mut g =
            Game::new(world, Progress::new_game(), Systems::null(), GameConfig { seed: 1, ..Default::default() });
        let guard = g.world.find_oid(50, None).unwrap();
        let player = g.world.find_oid(51, None).unwrap();
        let b = Behavior::Sly2Guard(Box::new(Sly2Guard::new(&g.world, guard)));
        g.world.set_behavior(guard, b);
        g.world.player = Some(player);
        (g, guard, player)
    }

    fn guard(g: &Game, h: Handle) -> &Sly2Guard {
        match g.world.behavior(h) {
            Some(Behavior::Sly2Guard(x)) => x,
            _ => panic!("not a Sly 2 guard"),
        }
    }

    #[test]
    fn guards_see_chase_attack_and_lose_the_player() {
        let (mut g, gh, ph) = guard_game();
        g.step_fixed(&FrameInput::default());
        assert_eq!(guard(&g, gh).sight, 0, "2,000 ahead is beyond the view volume");
        g.world.set_position(ph, [600.0, 0.0, 0.0]);
        g.step_fixed(&FrameInput::default());
        assert_eq!(guard(&g, gh).job, Job::Pursue);
        assert!(g.world.shared.noises.iter().any(|n| n.kind == 4), "an alarm about the target");
        for _ in 0..120 {
            g.step_fixed(&FrameInput::default());
        }
        assert!(g.world.position(gh).unwrap()[0] > 200.0, "closes in");
        assert!(g.world.shared.gang_hits.iter().any(|h| h.amount == 10.0 && h.kind == 0x200), "strikes the player");
        // Behind the guard and far away: lost, then back to its post after 5 s.
        g.world.set_position(ph, [-5000.0, 0.0, 0.0]);
        for _ in 0..(60 * 6) {
            g.step_fixed(&FrameInput::default());
        }
        assert!(matches!(guard(&g, gh).job, Job::Return | Job::Patrol | Job::Investigate { .. }));
    }

    #[test]
    fn surprise_hits_knock_out_and_spill_coins() {
        let (mut g, gh, ph) = guard_game();
        if let Some(Behavior::Sly2Guard(x)) = g.world.behavior_mut(gh) {
            x.pocket.entries = alloc::vec![Entry::Coins(5), Entry::Coins(4), Entry::Prize(1116)];
        }
        // A pickpocket takes the first entry.
        g.world.notify(gh, Notice::Pickpocket(ph));
        g.step_fixed(&FrameInput::default());
        assert_eq!(g.world.shared.picked, alloc::vec![Entry::Coins(5)]);
        // Unseen from behind: one hit knocks out, and half the coins left (4 → 2) spill.
        g.world.set_position(ph, [-300.0, 0.0, 0.0]);
        g.world.notify(gh, Notice::Attack { by: ph, kind: 1 });
        g.step_fixed(&FrameInput::default());
        assert!(guard(&g, gh).knocked_out());
        assert_eq!(g.world.shared.coin_drops, alloc::vec![([0.0, 0.0, 0.0], 2)]);
    }

    #[test]
    fn guards_run_command_lists() {
        use crate::behavior::sly2_commands::{Command, CommandList, run_commands};
        use crate::object::Arg;
        let (mut g, gh, ph) = guard_game();
        // The player far behind (unseen); its object also holds the list, and is where command 7 walks to.
        g.world.set_position(ph, [-50000.0, 0.0, 0.0]);
        let list = ph;
        let commands = alloc::vec![
            Command { kind: 5, args: alloc::vec![Arg::Float(0.5)] },
            Command { kind: 9, args: alloc::vec![Arg::Int(1)] },
            Command { kind: 7, args: alloc::vec![Arg::Short(6, 51), Arg::Float(400.0), Arg::Int(-1)] },
        ];
        g.world.set_behavior(list, Behavior::Sly2Commands(Box::new(CommandList { commands })));
        assert!(run_commands(&mut g.world, gh, list));
        for _ in 0..20 {
            g.step_fixed(&FrameInput::default());
        }
        assert_eq!(g.world.position(gh).unwrap()[0], 0.0, "waits 0.5 s first");
        for _ in 0..60 {
            g.step_fixed(&FrameInput::default());
        }
        assert!(g.world.position(gh).unwrap()[0] < -100.0, "then walks toward the object");
    }
}
