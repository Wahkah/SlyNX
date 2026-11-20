//! Guards (guards.md): the flashlight guard (class 9), the proximity guard (class 10), and the scripted variants
//! (11 to 13, 15, 17 to 19, 74, 75) on the same rules; companions (14) are allies driven by scripts.
//!
//! Every guard belongs to one alert group (`World::shared.groups`): the class-137 object listing it (option 694),
//! or a group of its own. A guard detects the player (sections 4 and 5), reports it to its group, and the group's
//! level (patrol, attack, search, dead) drives the 17-state machine of section 3. States play the animation bound to
//! them (option 656); a state "ends" when its animation does (or at once without one). Guards move by the root
//! motion of their state animations and turn toward their goal; they attack when section 6.2 allows, and a hit from
//! the player stuns or knocks them out (section 7).

use alloc::vec;
use alloc::vec::Vec;

use formats::collision::ConvexVolume;
use formats::geometry::Affine;
use script::Value;

use super::{ClassBehavior, Cx, dist_sq};
use crate::math::{add, dot, length, normalize, scale, sub};
use crate::object::class::id;
use crate::object::{Arg, Handle, OptionValue, flags};
use crate::physics::spring::{SmoothParams, Smoother};
use crate::world::shared::{AlertGroup, AlertLevel, Hiding, INTENSITY_ALARM, INTENSITY_SNEAK, damage};
use crate::world::{AnimCommand, Notice, World};

/// Guard states (guards.md 3); the numbers are data.
pub mod state {
    /// Transient: picks the next state from the group level.
    pub const DECIDE: u8 = 0;
    /// Follows its patrol animation.
    pub const PATROL: u8 = 1;
    /// Stands at its post.
    pub const STAND: u8 = 2;
    /// Waits at a waypoint.
    pub const WAYPOINT: u8 = 3;
    /// Walks back to its route.
    pub const RETURN: u8 = 4;
    /// Walks to its search point.
    pub const GO_SEARCH: u8 = 5;
    /// Searches on the spot.
    pub const SEARCH: u8 = 6;
    /// Noticed the player.
    pub const SPOT: u8 = 7;
    /// Lost the player.
    pub const LOST: u8 = 8;
    /// Gives up the search.
    pub const GIVE_UP: u8 = 9;
    /// Taunts.
    pub const TAUNT: u8 = 10;
    /// Knocked out.
    pub const DYING: u8 = 11;
    /// Chases the player.
    pub const CHASE: u8 = 12;
    /// Waits while chasing.
    pub const WAIT: u8 = 13;
    /// Attacks.
    pub const ATTACK: u8 = 14;
    /// Unused by the generic rules.
    pub const UNUSED: u8 = 15;
    /// Stunned.
    pub const STUNNED: u8 = 16;
    /// Number of states.
    pub const COUNT: usize = 17;
}

/// Guard options (guards.md).
pub mod opt {
    /// State animation binding (state, animation OID), repeated.
    pub const STATE_ANIM: u16 = 656;
    /// The call form of [`STATE_ANIM`].
    pub const CALL_STATE_ANIM: u16 = 657;
    /// Adds an attack effect: (effect object, damage kind) typed arguments (guards.md 6.2).
    pub const ATTACK_EFFECT: u16 = 661;
    /// The guard flies (a byte; guards.md 6.2): its thrown hits throw with no velocity.
    pub const FLYING: u16 = 673;
    /// Attack range (min, max).
    pub const ATTACK_RANGE: u16 = 653;
    /// Attack interval range, seconds.
    pub const ATTACK_INTERVAL: u16 = 654;
    /// Maximum attack angle, degrees.
    pub const ATTACK_ANGLE: u16 = 655;
    /// Stun count.
    pub const STUN_COUNT: u16 = 665;
    /// Coins dropped at death.
    pub const COINS: u16 = 678;
    /// Ignore all damage.
    pub const INVULNERABLE: u16 = 692;
    /// Group members (on class 137).
    pub const GROUP_MEMBERS: u16 = 694;
    /// Near distance (flashlight).
    pub const NEAR_DISTANCE: u16 = 696;
    /// Near height (flashlight).
    pub const NEAR_HEIGHT: u16 = 697;
    /// Sneak area distance.
    pub const SNEAK_DISTANCE: u16 = 698;
    /// Sneak area height.
    pub const SNEAK_HEIGHT: u16 = 699;
    /// Flash volume (OID).
    pub const FLASH: u16 = 701;
    /// Always detect.
    pub const ALWAYS_DETECT: u16 = 704;
    /// Never detect.
    pub const NEVER_DETECT: u16 = 705;
    /// Detection flags: bit 0 sees hidden players, bit 1 no line of sight.
    pub const DETECT_FLAGS: u16 = 706;
    /// Abandon distance (proximity).
    pub const ABANDON_DISTANCE: u16 = 841;
    /// Abandon height (proximity).
    pub const ABANDON_HEIGHT: u16 = 842;
    /// Leash volume (OID).
    pub const LEASH: u16 = 843;
    /// Detection range (min, max).
    pub const DETECT_RANGE: u16 = 845;
    /// View-angle range (min, max), degrees.
    pub const VIEW_ANGLE: u16 = 846;
    /// Detect volume (OID).
    pub const DETECT_VOLUME: u16 = 847;
    /// Turn-rate scale.
    pub const TURN_RATE: u16 = 710;
    /// Give-up volume set by scripts (options-by-class.md 1.1).
    pub const GIVE_UP_VOLUME: u16 = 844;
    /// Detect volume set by scripts.
    pub const SCRIPT_DETECT_VOLUME: u16 = 848;
    /// Detection latched by another object.
    pub const LATCHED: u16 = 849;
    /// Give-up forced by another object.
    pub const FORCE_GIVE_UP: u16 = 850;
}

/// Sight test points in the player's frame (executable 0x274660 and 0x274620; guards.md 4.2).
pub const SIGHT_PATROL: [[f32; 3]; 4] = [[0.0, 0.0, -20.0], [0.0, 0.0, 70.0], [0.0, -30.0, 30.0], [0.0, 30.0, 30.0]];
/// Sight test points while the group attacks.
pub const SIGHT_ATTACK: [[f32; 3]; 4] = [[0.0, 0.0, -80.0], [0.0, 0.0, 100.0], [0.0, -70.0, 40.0], [0.0, 70.0, 40.0]];
/// The flashlight's aim point ahead of the guard.
pub const AIM_AHEAD: f32 = 1000.0;
/// Attack-angle distance scale (quadratic 0, 0.005, 0).
pub const ATTACK_ANGLE_SCALE: f32 = 0.005;
/// Delay ranges, seconds: give up, stop patrol, lost the player.
pub const GIVE_UP_DELAY: (f32, f32) = (0.0, 4.0);
/// See [`GIVE_UP_DELAY`].
pub const STOP_PATROL_DELAY: (f32, f32) = (0.0, 1.5);
/// See [`GIVE_UP_DELAY`].
pub const LOST_DELAY: (f32, f32) = (0.0, 0.3);
/// Search circle spacing.
pub const SEARCH_SPACING: f32 = 700.0;
/// A flashlight guard searching on the spot is done after this long.
pub const SEARCH_DONE_AFTER: f32 = 5.0;
/// Turning (physics.md 5.1) at spin ratio 1: the top speed F (rad/s) and the time T (s) to reach it; with ratio
/// r (option 710) they are 25·r and 0.25/√r.
pub const TURN: (f32, f32) = (25.0, 0.25);

/// One frame of turning (physics.md 5.1) toward a target `angle` away (signed, radians) at signed turning `speed`
/// (rad/s), spin ratio `r`: the wanted speed is F·min(1, a/θ) toward the target, a the angle predicted half a frame
/// ahead and θ = F·T/2; the speed moves toward it by at most F/T per second², a speed above F decays at 2.5 per
/// second. Returns the new speed and the angle turned this frame (never past the target).
pub fn turn_step(angle: f32, speed: f32, r: f32, dt: f32) -> (f32, f32) {
    let r = r.max(1e-3);
    let f = TURN.0 * r;
    let t = TURN.1 / crate::math::sqrt(r);
    let theta = f * t * 0.5;
    let ahead = angle - speed * dt * 0.5;
    let want = if ahead.abs() < 1e-6 { 0.0 } else { ahead.signum() * f * (ahead.abs() / theta).min(1.0) };
    let mut s = speed;
    if s.abs() > f {
        s = s.signum() * (f + (s.abs() - f) * (1.0 - 2.5 * dt).max(0.0));
    }
    let step = f / t * dt;
    s += (want - s).clamp(-step, step);
    let mut turned = s * dt;
    if angle.signum() == turned.signum() && turned.abs() > angle.abs() {
        turned = angle;
    }
    (s, turned)
}
/// Alarm sound when a group of flashlight guards spots the player.
pub const SPOT_SOUND: i32 = 63;
/// Flash volume target scales by level.
pub const FLASH_SCALE: [f32; 4] = [1.0, 1.3, 1.3, 0.1];

/// Which perception rules a guard uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardKind {
    /// Class 9: the light cone and line of sight.
    Flashlight,
    /// Class 10 and the proximity variants: distance, angle and leash.
    Proximity,
    /// Class 14: an ally, never detects.
    Companion,
}

/// A guard's flashlight.
#[derive(Clone, Debug, PartialEq)]
pub struct Flash {
    /// The flash volume object (class 96).
    pub object: Handle,
    /// Its convex volume.
    pub volume: ConvexVolume,
    /// Current scale.
    pub scale: f32,
    smoother: Smoother,
}

/// A guard.
#[derive(Clone, Debug, PartialEq)]
pub struct Guard {
    /// Perception rules.
    pub kind: GuardKind,
    /// Current state.
    pub state: u8,
    /// Time in the current state.
    pub state_time: f32,
    /// Turning speed, rad/s (signed, counter-clockwise positive).
    pub turn_speed: f32,
    /// Its alert group.
    pub group: usize,
    /// Starting transform (leash, posts).
    pub start: Affine,
    /// State animations (segment OIDs).
    pub anims: [Option<i16>; state::COUNT],
    /// Stuns left before a knock-out.
    pub stuns: i32,
    /// Proximity guards stay alerted until the player leaves the leash.
    pub latched: bool,
    /// The flashlight.
    pub flash: Option<Flash>,
    /// Seen the player this frame.
    pub sees: bool,
    stop_patrol_at: Option<f32>,
    lost_at: Option<f32>,
    give_up_at: Option<f32>,
    attack_ready_at: f32,
    search_point: Option<([f32; 3], [f32; 3])>,
    group_serial: u32,
    group_level: AlertLevel,
    anim_done: bool,
    hurt_this_attack: bool,
    sneaking: bool,
    /// Attack effects (option 661, at most 4): effect object OID and damage kind.
    pub effects: Vec<(i16, u8)>,
    /// The effect objects found, with their last position (for their motion at the contact).
    effect_objects: Vec<(Handle, u8, Option<[f32; 3]>)>,
    /// Flying guards (option 673) throw with no velocity (guards.md 6.2).
    pub flying: bool,
}

/// A guard keeps at most this many attack effects (guards.md 6.2).
pub const MAX_EFFECTS: usize = 4;
/// An attack effect's throw: the effect's motion times 1.25 (elasticity 0.25 + 1), at most 1,200, at least 600 up.
pub const THROW_SCALE: f32 = 1.25;
/// See [`THROW_SCALE`].
pub const THROW_MAX: f32 = 1200.0;
/// See [`THROW_SCALE`].
pub const THROW_MIN_UP: f32 = 600.0;

/// An (effect object OID, damage kind) pair from option 661's arguments.
fn attack_effect(args: &[Arg]) -> Option<(i16, u8)> {
    let mut object = None;
    let mut kind = None;
    for a in args {
        match *a {
            Arg::Int(n) if kind.is_none() => kind = Some(n as u8),
            Arg::Float(n) if kind.is_none() => kind = Some(n as u8),
            Arg::Short(tag, n) if (4096..=8191).contains(&tag) && kind.is_none() => kind = Some(n as u8),
            Arg::Short(_, n) if object.is_none() => object = Some(n),
            _ => {}
        }
    }
    Some((object?, kind.unwrap_or(damage::BLUNT)))
}

/// The velocity an attack effect throws the player at, from the effect's motion (guards.md 6.2).
pub fn throw_velocity(motion: [f32; 3]) -> [f32; 3] {
    let mut v = scale(motion, THROW_SCALE);
    let l = length(v);
    if l > THROW_MAX {
        v = scale(v, THROW_MAX / l);
    }
    v[2] = v[2].max(THROW_MIN_UP);
    v
}

/// The `(state, animation OID)` pairs of a state-animation option (typed arguments).
fn state_binding(v: &OptionValue) -> Option<(u8, i16)> {
    let OptionValue::Args(args) = v else { return None };
    binding_of(args)
}

/// A (state, animation OID) pair from an argument list, in either order.
fn binding_of(args: &[Arg]) -> Option<(u8, i16)> {
    let mut state = None;
    let mut anim = None;
    for a in args.iter() {
        match *a {
            Arg::Int(n) if state.is_none() => state = Some(n as u8),
            Arg::Float(n) if state.is_none() => state = Some(n as u8),
            Arg::Short(tag, n) if (4096..=8191).contains(&tag) && state.is_none() => state = Some(n as u8),
            Arg::Short(_, n) => anim = Some(n),
            _ => {}
        }
    }
    Some((state?, anim?))
}

fn heading(w: &Affine) -> [f32; 3] {
    normalize([w.cols[0][0], w.cols[0][1], 0.0])
}

/// Angle between two horizontal unit vectors.
fn angle_between(a: [f32; 3], b: [f32; 3]) -> f32 {
    let c = dot(a, b).clamp(-1.0, 1.0);
    acos(c)
}

/// `acos` from the engine's own `sqrt` and an `atan` series, accurate to about 1e-4 rad.
pub fn acos(c: f32) -> f32 {
    let s = crate::math::sqrt((1.0 - c * c).max(0.0));
    atan2(s, c)
}

/// `atan2` accurate to about 1e-4 rad.
pub fn atan2(y: f32, x: f32) -> f32 {
    let (ax, ay) = (if x < 0.0 { -x } else { x }, if y < 0.0 { -y } else { y });
    if ax == 0.0 && ay == 0.0 {
        return 0.0;
    }
    let (a, swap) = if ay > ax { (ax / ay, true) } else { (ay / ax, false) };
    let s = a * a;
    let mut r = ((-0.046_496_475 * s + 0.159_314_22) * s - 0.327_622_76) * s * a + a;
    if swap {
        r = core::f32::consts::FRAC_PI_2 - r;
    }
    if x < 0.0 {
        r = core::f32::consts::PI - r;
    }
    if y < 0.0 { -r } else { r }
}

impl Guard {
    /// A guard of `class`.
    pub fn new(world: &World, h: Handle) -> Guard {
        let class = world.class(h).unwrap_or(id::GUARD_PROXIMITY);
        let kind = match class {
            id::GUARD_FLASHLIGHT => GuardKind::Flashlight,
            id::COMPANION => GuardKind::Companion,
            _ => GuardKind::Proximity,
        };
        let o = world.options(h).cloned().unwrap_or_default();
        let mut anims = [None; state::COUNT];
        for v in o.all(opt::STATE_ANIM) {
            if let Some((s, a)) = state_binding(v)
                && (s as usize) < state::COUNT
            {
                anims[s as usize] = Some(a);
            }
        }
        Guard {
            kind,
            state: state::DECIDE,
            state_time: 0.0,
            group: usize::MAX,
            start: world.world_transform(h).unwrap_or(Affine::IDENTITY),
            anims,
            stuns: o.i32(opt::STUN_COUNT).unwrap_or(0),
            latched: false,
            flash: None,
            sees: false,
            turn_speed: 0.0,
            stop_patrol_at: None,
            lost_at: None,
            give_up_at: None,
            attack_ready_at: 0.0,
            search_point: None,
            group_serial: 0,
            group_level: AlertLevel::Patrol,
            anim_done: true,
            hurt_this_attack: false,
            sneaking: false,
            effects: Vec::new(),
            effect_objects: Vec::new(),
            flying: o.bool_or(opt::FLYING, false),
        }
    }

    /// Whether the guard is knocked out.
    pub fn is_down(&self) -> bool {
        self.state == state::DYING
    }

    fn enter(&mut self, cx: &mut Cx<'_>, s: u8) {
        if s == self.state {
            return;
        }
        let old = self.state;
        cx.world.anim.push(AnimCommand::Stop { owner: cx.me, tag: old as u32 });
        self.state = s;
        self.state_time = 0.0;
        self.anim_done = true;
        self.hurt_this_attack = false;
        if let Some(oid) = self.anims[s as usize]
            && let Some(seg) = cx.world.find_nearest(oid, Some(cx.me))
        {
            cx.world.anim.push(AnimCommand::Play { segment: seg, owner: cx.me, time: 0.0, speed: 1.0, tag: s as u32 });
            self.anim_done = false;
        }
        cx.raise(script::event::GUARD_STATE, vec![Value::Int(old as i32), Value::Int(s as i32)]);
    }

    /// The player's test position and frame: the player object, or the probe.
    fn target(cx: &Cx<'_>) -> Option<(Affine, [f32; 3])> {
        let base = cx.world.shared.player.base;
        match cx.world.player_target() {
            Some(p) => {
                let w = cx.world.world_transform(p)?;
                let mut t = base.unwrap_or(w.t);
                if base.is_none() && cx.world.shared.player.airborne {
                    t[2] -= 70.0;
                }
                Some((w, t))
            }
            None => {
                let p = cx.world.probe?;
                Some((Affine { cols: crate::math::MAT3_IDENTITY, t: p }, p))
            }
        }
    }

    /// Whether the player hides from a viewer at `from` (guards.md 4.3).
    pub fn hidden_from(cx: &Cx<'_>, player: &Affine, from: [f32; 3]) -> bool {
        let to_viewer = normalize(sub(from, player.t));
        let facing = heading(player);
        match cx.world.shared.player.hiding {
            Hiding::None => false,
            Hiding::Wall { detectable } => !detectable && dot(facing, to_viewer) < 0.0,
            Hiding::Cover { detectable } => !detectable && dot(facing, to_viewer) > 0.0,
            Hiding::Basket => true,
            Hiding::Casual => cx.world.shared.player.rewards & 0x12000 != 0,
        }
    }

    fn detect(&mut self, cx: &mut Cx<'_>, level: AlertLevel) -> bool {
        if self.kind == GuardKind::Companion || matches!(self.state, state::DYING | state::STUNNED) {
            return false;
        }
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        if cx.world.shared.player.controls_locked || o.bool_or(opt::NEVER_DETECT, false) {
            return false;
        }
        if o.bool_or(opt::ALWAYS_DETECT, false) {
            return true;
        }
        let Some((pw, p)) = Self::target(cx) else { return false };
        let me = cx.world.world_transform(cx.me).unwrap_or(self.start);
        match self.kind {
            GuardKind::Flashlight => {
                let d = sub(p, me.t);
                let horiz = length([d[0], d[1], 0.0]);
                let near =
                    horiz <= o.f32_or(opt::NEAR_DISTANCE, 300.0) && d[2].abs() <= o.f32_or(opt::NEAR_HEIGHT, 300.0);
                if !(near || self.in_light(cx, p)) {
                    return false;
                }
                let flags = o.i32(opt::DETECT_FLAGS).unwrap_or(0);
                if flags & 1 == 0 && level != AlertLevel::Attack && Self::hidden_from(cx, &pw, me.t) {
                    return false;
                }
                if flags & 2 != 0 {
                    return true;
                }
                let points = if level == AlertLevel::Attack { SIGHT_ATTACK } else { SIGHT_PATROL };
                let mut ignore = vec![cx.me];
                ignore.extend(cx.world.player_target());
                for b in o.shorts(708).take(4) {
                    ignore.extend(cx.world.find_nearest(b, Some(cx.me)));
                }
                if let Some(f) = &self.flash {
                    ignore.push(f.object);
                }
                points.iter().any(|&q| {
                    let to = pw.transform_point(q);
                    cx.collision.line_clear(cx.world, me.t, to, &ignore).unwrap_or(true)
                })
            }
            GuardKind::Proximity => {
                if !self.in_leash(cx, p) {
                    self.latched = false;
                    return false;
                }
                if self.latched || o.bool_or(opt::LATCHED, false) {
                    return true;
                }
                let inside_volume = o
                    .i16(opt::SCRIPT_DETECT_VOLUME)
                    .or_else(|| o.i16(opt::DETECT_VOLUME))
                    .and_then(|v| cx.world.find_nearest(v, Some(cx.me)))
                    .is_some_and(|v| volume_contains(cx.world, v, p));
                let seen = inside_volume || {
                    let [dmin, dmax] = o.pair(opt::DETECT_RANGE).unwrap_or([1000.0, 1500.0]);
                    let deg = core::f32::consts::PI / 180.0;
                    let [amin, amax] =
                        o.pair(opt::VIEW_ANGLE).map_or([45.0 * deg, 180.0 * deg], |a| [a[0] * deg, a[1] * deg]);
                    let d = sub(p, me.t);
                    let dist = length(d);
                    let allowed = if dist >= dmax {
                        -1.0
                    } else if dist < dmin {
                        amax
                    } else {
                        amax + (amin - amax) * (dist - dmin) / (dmax - dmin).max(1e-6)
                    };
                    allowed >= 0.0 && angle_between(heading(&me), normalize([d[0], d[1], 0.0])) <= allowed
                };
                let seen = seen && !Self::hidden_from(cx, &pw, me.t);
                if seen {
                    self.latched = true;
                }
                seen
            }
            GuardKind::Companion => false,
        }
    }

    fn in_leash(&self, cx: &Cx<'_>, p: [f32; 3]) -> bool {
        if cx.world.shared.player.grabbed {
            return true;
        }
        let o = cx.world.options(cx.me);
        let leash = o.and_then(|o| o.i16(opt::GIVE_UP_VOLUME).or_else(|| o.i16(opt::LEASH)));
        if let Some(v) = leash.and_then(|v| cx.world.find_nearest(v, Some(cx.me))) {
            return volume_contains(cx.world, v, p);
        }
        let d = sub(p, self.start.t);
        let dist = o.map_or(2000.0, |o| o.f32_or(opt::ABANDON_DISTANCE, 2000.0));
        let height = o.map_or(300.0, |o| o.f32_or(opt::ABANDON_HEIGHT, 300.0));
        length(d) <= dist && d[2].abs() <= height
    }

    fn in_light(&self, cx: &Cx<'_>, p: [f32; 3]) -> bool {
        let Some(f) = &self.flash else { return false };
        if f.scale < 0.0001 {
            return false;
        }
        let Some(w) = cx.world.world_transform(f.object) else { return false };
        let Some(inv) = crate::math::affine_inverse(&w) else { return false };
        let local = scale(inv.transform_point(p), 1.0 / f.scale);
        crate::world::volume::contains_local(&f.volume, local)
    }

    /// Scales and aims the flashlight (guards.md 4.1; effects.md 3).
    fn aim_flash(&mut self, cx: &mut Cx<'_>, level: AlertLevel) {
        let me = cx.world.world_transform(cx.me).unwrap_or(self.start);
        let target = match (level, Self::target(cx)) {
            (AlertLevel::Attack, Some((_, p))) => p,
            _ => add(me.t, scale(heading(&me), AIM_AHEAD)),
        };
        let Some(f) = self.flash.as_mut() else { return };
        let goal = FLASH_SCALE[level as usize];
        let params = SmoothParams { fast: 1.0, slow: 0.2, fast_time: 0.1, max_speed_change: None };
        f.scale = f.smoother.step(&params, f.scale, goal, cx.dt);
        let Some(w) = cx.world.world_transform(f.object) else { return };
        let x = normalize(sub(target, w.t));
        if length(x) == 0.0 {
            return;
        }
        let z0 = [0.0, 0.0, 1.0];
        let y = normalize(crate::math::cross(z0, x));
        let y = if length(y) == 0.0 { [0.0, 1.0, 0.0] } else { y };
        let z = crate::math::cross(x, y);
        cx.world.set_world_transform(f.object, Affine { cols: [x, y, z], t: w.t });
    }

    fn group_mut(world: &mut World, g: usize) -> &mut AlertGroup {
        &mut world.shared.groups[g]
    }

    /// Re-evaluates the group's level from its members' reports (guards.md 2).
    fn update_group(cx: &mut Cx<'_>, g: usize) {
        let now = cx.time;
        let all_flash = {
            let grp = &cx.world.shared.groups[g];
            grp.members.iter().all(|&m| cx.world.class_of_slot(m) == id::GUARD_FLASHLIGHT)
        };
        let alive = {
            let grp = &cx.world.shared.groups[g];
            grp.members.iter().any(|&m| {
                cx.world.handle(m).is_some_and(|h| match cx.world.behavior(h) {
                    Some(super::Behavior::Guard(gd)) => !gd.is_down(),
                    _ => false,
                })
            })
        };
        let key = u32::MAX - g as u32;
        let grp = Self::group_mut(cx.world, g);
        let old = grp.level;
        let new = if !alive {
            AlertLevel::Dead
        } else if !grp.seeing.is_empty() {
            AlertLevel::Attack
        } else if old == AlertLevel::Attack {
            AlertLevel::Search
        } else if old == AlertLevel::Search
            && (grp.give_up || grp.members.iter().all(|m| grp.done_searching.contains(m)))
        {
            AlertLevel::Patrol
        } else {
            old
        };
        if new == old {
            return;
        }
        grp.level = new;
        grp.serial += 1;
        grp.give_up = false;
        grp.done_searching.clear();
        let alarms = grp.alarms.clone();
        let _ = now;
        match new {
            AlertLevel::Attack => {
                cx.world.shared.intensity.insert(key, INTENSITY_ALARM);
                if old == AlertLevel::Patrol && all_flash {
                    cx.world.play_sound(SPOT_SOUND, Some(cx.me));
                }
                for a in alarms {
                    if let Some(h) = cx.world.handle(a) {
                        super::sensor::set_alarm(cx.world, h, super::sensor::AlarmState::Triggered);
                    }
                }
            }
            AlertLevel::Patrol => {
                cx.world.shared.intensity.remove(&key);
                for a in alarms {
                    if let Some(h) = cx.world.handle(a) {
                        super::sensor::set_alarm(cx.world, h, super::sensor::AlarmState::Armed);
                    }
                }
            }
            AlertLevel::Search => {
                cx.world.shared.intensity.remove(&key);
            }
            AlertLevel::Dead => {
                cx.world.shared.intensity.remove(&key);
                for a in alarms {
                    if let Some(h) = cx.world.handle(a) {
                        super::sensor::set_alarm(cx.world, h, super::sensor::AlarmState::Disabled);
                    }
                }
            }
        }
    }

    /// Reacts to a change of the group's level: timers and search points (guards.md 2, 5.3).
    fn react(&mut self, cx: &mut Cx<'_>) {
        let (serial, level, last_seen, members) = {
            let g = &cx.world.shared.groups[self.group];
            (g.serial, g.level, g.last_seen, g.members.clone())
        };
        if serial == self.group_serial {
            return;
        }
        let old = self.group_level;
        self.group_serial = serial;
        self.group_level = level;
        let now = cx.time;
        if level == AlertLevel::Attack {
            self.stop_patrol_at =
                Some(if self.sees { now } else { now + cx.rng.range(STOP_PATROL_DELAY.0, STOP_PATROL_DELAY.1) });
        }
        if old == AlertLevel::Attack {
            self.lost_at = Some(now + cx.rng.range(LOST_DELAY.0, LOST_DELAY.1));
        }
        if level == AlertLevel::Search {
            if let Some((p, v)) = last_seen {
                let n = members.len().max(1) as f32;
                let k = members.iter().position(|&m| m == cx.me.index()).unwrap_or(0) as f32;
                let step = core::f32::consts::TAU / n;
                let base = atan2(v[1], v[0]);
                let a = base + step * (k + cx.rng.range(-0.2, 0.2));
                let r = n * SEARCH_SPACING / core::f32::consts::TAU * cx.rng.range(0.8, 1.2);
                let (s, c) = crate::math::sin_cos(a);
                let point = [p[0] + r * c, p[1] + r * s, p[2]];
                self.search_point = Some((point, normalize(sub(p, point))));
            }
            self.stop_patrol_at = Some(now);
        }
        if old == AlertLevel::Search {
            self.give_up_at = Some(now + cx.rng.range(GIVE_UP_DELAY.0, GIVE_UP_DELAY.1));
        }
    }

    fn can_attack(&self, cx: &Cx<'_>, sees: bool) -> bool {
        if cx.world.shared.player.invulnerable || cx.world.shared.player.hiding != Hiding::None {
            return false;
        }
        if cx.time < self.attack_ready_at || (self.kind == GuardKind::Flashlight && !sees) {
            return false;
        }
        let Some((_, p)) = Self::target(cx) else { return false };
        let o = cx.world.options(cx.me);
        let [rmin, rmax] = o.and_then(|o| o.pair(opt::ATTACK_RANGE)).unwrap_or([0.0, 300.0]);
        let me = cx.world.world_transform(cx.me).unwrap_or(self.start);
        let d = sub(p, me.t);
        let dist = length(d);
        if dist < rmin || dist > rmax {
            return false;
        }
        let max_angle = o.and_then(|o| o.f32(opt::ATTACK_ANGLE)).unwrap_or(10.0) * core::f32::consts::PI / 180.0;
        let a = angle_between(heading(&me), normalize([d[0], d[1], 0.0])) * (ATTACK_ANGLE_SCALE * dist).min(1.0);
        a <= max_angle
    }

    /// Turns toward a point at the turn rate, and moves by the state animation's root motion.
    fn steer(&mut self, cx: &mut Cx<'_>, goal: Option<[f32; 3]>) {
        let Some(mut w) = cx.world.world_transform(cx.me) else { return };
        let ratio = cx.world.options(cx.me).map_or(1.0, |o| o.f32_or(opt::TURN_RATE, 1.0));
        let mut angle = 0.0;
        if let Some(goal) = goal {
            let want = normalize(sub([goal[0], goal[1], 0.0], [w.t[0], w.t[1], 0.0]));
            if length(want) > 0.0 {
                let h = heading(&w);
                let cross = h[0] * want[1] - h[1] * want[0];
                angle = if cross < 0.0 { -angle_between(h, want) } else { angle_between(h, want) };
            }
        }
        let (speed, turned) = turn_step(angle, self.turn_speed, ratio, cx.dt);
        self.turn_speed = speed;
        if turned != 0.0 {
            let r = crate::world::rotation_from_vector([0.0, 0.0, turned]);
            w.cols = crate::math::mat_mul(&r, &w.cols);
        }
        if let Some(v) = cx.anim.root_motion(cx.me) {
            let world_v = w.transform_vector(v);
            w.t = crate::math::madd(w.t, world_v, cx.dt);
            if let Some((g, _, _)) = cx.collision.ground(cx.world, add(w.t, [0.0, 0.0, 50.0]), 150.0) {
                w.t[2] = g[2];
            }
        }
        cx.world.set_world_transform(cx.me, w);
    }

    fn take_hit(&mut self, cx: &mut Cx<'_>) {
        if cx.world.options(cx.me).is_some_and(|o| o.bool_or(opt::INVULNERABLE, false)) || self.is_down() {
            return;
        }
        if self.stuns > 0 {
            self.stuns -= 1;
            self.enter(cx, state::STUNNED);
            return;
        }
        self.enter(cx, state::DYING);
        cx.world.shared.intensity.remove(&cx.me.index());
        let world_id = cx.world.info.id;
        if let Some(k) = cx.world.persist_key(cx.me) {
            cx.progress.level(world_id).defeated.insert(k);
        }
        let coins = cx.world.options(cx.me).and_then(|o| o.i32(opt::COINS)).unwrap_or(-1);
        let at = add(cx.my_position(), cx.world.options(cx.me).and_then(|o| o.vec3(679)).unwrap_or([0.0; 3]));
        super::breakable::drop_reward(cx, coins, at);
        if let Some(f) = &self.flash {
            let _ = f;
        }
        Self::update_group(cx, self.group);
    }
}

/// Whether an object's convex volume (classes 123, 96, ...) or solid contains a world point.
pub fn volume_contains(world: &World, v: Handle, p: [f32; 3]) -> bool {
    let Some(rec) = world.record(v) else { return false };
    let Some(vr) = world.level.stream.volumes.iter().find(|x| x.record == rec) else { return false };
    let Ok(cv) = ConvexVolume::parse(&world.level.bytes, vr.offset) else { return false };
    let Some(w) = world.world_transform(v) else { return false };
    crate::world::volume::PlacedVolume::new(cv, &w).contains(p)
}

/// The group a guard belongs to: the class-137 object listing its OID (option 694), else a new group of its own.
fn assign_group(world: &mut World, me: Handle) -> usize {
    let oid = world.oid(me).unwrap_or(-1);
    let mut group_obj = None;
    for &s in world.slots_of_class(id::GUARD_GROUP) {
        let Some(h) = world.handle(s) else { continue };
        if world.options(h).is_some_and(|o| o.shorts(opt::GROUP_MEMBERS).any(|m| m == oid)) {
            group_obj = Some(h);
            break;
        }
    }
    let gi = match group_obj.and_then(|g| world.shared.groups.iter().position(|x| x.object == Some(g))) {
        Some(i) => i,
        None => {
            world.shared.groups.push(AlertGroup { object: group_obj, ..Default::default() });
            world.shared.groups.len() - 1
        }
    };
    world.shared.groups[gi].members.push(me.index());
    gi
}

impl Guard {
    /// Resolves the attack effects' objects (once).
    fn resolve_effects(&mut self, cx: &mut Cx<'_>) {
        if self.effects.len() == self.effect_objects.len() {
            return;
        }
        self.effect_objects.clear();
        for &(oid, kind) in &self.effects {
            if let Some(h) = cx.world.find_oid(oid, Some(cx.me)) {
                self.effect_objects.push((h, kind, None));
            }
        }
        if self.effect_objects.is_empty() {
            // None found: keep the effects from being looked up every frame.
            self.effects.clear();
        }
    }

    /// Keeps the effect objects' positions for their motion.
    fn track_effects(&mut self, cx: &mut Cx<'_>) {
        for (h, _, last) in &mut self.effect_objects {
            *last = cx.world.world_transform(*h).map(|w| w.t);
        }
    }

    /// The attack lands (guards.md 6.2): through its first attack effect still in the world, else as a blunt hit.
    /// Blunt, and thrown from a guard that is not flying, throw the player by the effect's motion; thrown from a
    /// flying guard throws with no velocity; fire and crush hurt without a throw. The effect is used up.
    fn land_attack(&mut self, cx: &mut Cx<'_>) {
        self.resolve_effects(cx);
        let Some(i) = self.effect_objects.iter().position(|e| cx.world.in_world(e.0)) else {
            cx.world.hurt_player(cx.me, damage::BLUNT, None);
            return;
        };
        let (h, kind, last) = self.effect_objects[i];
        let motion = match (last, cx.world.world_transform(h)) {
            (Some(a), Some(b)) if cx.dt > 0.0 => scale(sub(b.t, a), 1.0 / cx.dt),
            _ => [0.0; 3],
        };
        match kind {
            damage::BLUNT => cx.world.hurt_player_thrown(cx.me, kind, throw_velocity(motion)),
            damage::THROWN if !self.flying => cx.world.hurt_player_thrown(cx.me, kind, throw_velocity(motion)),
            damage::THROWN => cx.world.hurt_player_thrown(cx.me, kind, [0.0; 3]),
            _ => cx.world.hurt_player(cx.me, kind, None),
        }
        cx.world.set_in_world(h, false);
    }
}

impl ClassBehavior for Guard {
    fn ticks(&self) -> bool {
        true
    }

    /// Option 657, the call form of 656: binds an animation segment to a state (guards.md 3).
    fn action(&mut self, _world: &mut World, _me: Handle, id: u16, args: &[Arg]) -> bool {
        if id == opt::ATTACK_EFFECT {
            if let Some(e) = attack_effect(args)
                && self.effects.len() < MAX_EFFECTS
            {
                self.effects.push(e);
            }
            return true;
        }
        if id != opt::CALL_STATE_ANIM {
            return false;
        }
        if let Some((s, a)) = binding_of(args)
            && (s as usize) < state::COUNT
        {
            self.anims[s as usize] = Some(a);
        }
        true
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let world_id = cx.world.info.id;
        if cx
            .world
            .persist_key(cx.me)
            .is_some_and(|k| cx.progress.level_ref(world_id).is_some_and(|l| l.defeated.contains(&k)))
        {
            self.state = state::DYING;
            cx.world.remove_quietly(cx.me);
        }
        self.group = assign_group(cx.world, cx.me);
        if self.kind == GuardKind::Flashlight
            && let Some(f) = cx.world.options(cx.me).and_then(|o| o.i16(opt::FLASH))
            && let Some(obj) = cx.world.find_nearest(f, Some(cx.me))
            && let Some(rec) = cx.world.record(obj)
            && let Some(vr) = cx.world.level.stream.volumes.iter().find(|x| x.record == rec)
            && let Ok(volume) = ConvexVolume::parse(&cx.world.level.bytes, vr.offset)
        {
            self.flash = Some(Flash { object: obj, volume, scale: 1.0, smoother: Smoother { speed: 0.0 } });
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        self.state_time += cx.dt;
        let notices = cx.world.take_inbox(cx.me.index());
        let hit = notices.iter().any(|n| {
            matches!(n, Notice::Hit(_) | Notice::Attack { .. } | Notice::Blast(..))
                && !matches!(n, Notice::Hit(s) if *s == cx.me)
        });
        if hit {
            self.take_hit(cx);
        }
        if self.state == state::DYING {
            if let Some(f) = &mut self.flash {
                f.scale = (f.scale - cx.dt).max(0.1);
            }
            return;
        }
        if self.group == usize::MAX {
            self.group = assign_group(cx.world, cx.me);
        }
        if !self.anim_done && !cx.anim.is_playing(cx.me, self.state as u32) && self.state_time > 0.0 {
            self.anim_done = true;
        }

        // Perception, reports and the group.
        let level = cx.world.shared.groups[self.group].level;
        self.sees = self.detect(cx, level);
        let me_slot = cx.me.index();
        let target = Self::target(cx).map(|(_, p)| p);
        {
            let vel = cx.world.shared.player.velocity;
            let g = &mut cx.world.shared.groups[self.group];
            g.seeing.retain(|&m| m != me_slot);
            if self.sees {
                g.seeing.push(me_slot);
                if let Some(p) = target {
                    g.last_seen = Some((p, vel));
                }
            }
        }
        if cx.world.options(cx.me).is_some_and(|o| o.bool_or(opt::FORCE_GIVE_UP, false)) {
            cx.world.shared.groups[self.group].give_up = true;
        }
        Self::update_group(cx, self.group);
        self.react(cx);
        if self.flash.is_some() {
            let lvl = cx.world.shared.groups[self.group].level;
            self.aim_flash(cx, lvl);
        }
        self.sneak_music(cx);

        // Transitions (guards.md 3).
        let level = cx.world.shared.groups[self.group].level;
        let now = cx.time;
        let stop_patrol = self.stop_patrol_at.is_some_and(|t| now >= t);
        let lost = self.lost_at.is_some_and(|t| now >= t);
        let done = self.anim_done;
        let attack_ok = level == AlertLevel::Attack && self.can_attack(cx, self.sees);
        use state::*;
        let next = match (self.state, level) {
            (DECIDE, AlertLevel::Patrol) => Some(RETURN),
            (DECIDE, AlertLevel::Search) => Some(GO_SEARCH),
            (DECIDE, AlertLevel::Attack) => Some(SPOT),
            (PATROL | STAND | WAYPOINT | RETURN, AlertLevel::Search) if stop_patrol => Some(GO_SEARCH),
            (PATROL | STAND | WAYPOINT | RETURN, AlertLevel::Attack) if stop_patrol => Some(SPOT),
            (PATROL, AlertLevel::Patrol) if done => Some(STAND),
            (STAND, AlertLevel::Patrol) if self.anims[PATROL as usize].is_some() && self.state_time > 1.0 => {
                Some(PATROL)
            }
            (RETURN, AlertLevel::Patrol) if dist_sq(cx.my_position(), self.start.t) < 50.0 * 50.0 => {
                Some(if self.anims[PATROL as usize].is_some() { PATROL } else { STAND })
            }
            (GO_SEARCH, AlertLevel::Patrol) => Some(GIVE_UP),
            (GO_SEARCH, AlertLevel::Attack) => Some(SPOT),
            (GO_SEARCH, AlertLevel::Search)
                if self.search_point.is_none_or(|(p, _)| dist_sq(cx.my_position(), p) < 100.0 * 100.0) =>
            {
                Some(SEARCH)
            }
            (SEARCH, AlertLevel::Patrol) if self.give_up_at.is_none_or(|t| now >= t) => Some(GIVE_UP),
            (SEARCH, AlertLevel::Attack) => Some(SPOT),
            (SPOT, _) if done => Some(CHASE),
            (LOST | TAUNT | STUNNED, _) if done => Some(DECIDE),
            (GIVE_UP, AlertLevel::Patrol) if done => Some(RETURN),
            (GIVE_UP, AlertLevel::Search) => Some(GO_SEARCH),
            (GIVE_UP, AlertLevel::Attack) if stop_patrol => Some(SPOT),
            (CHASE | WAIT | ATTACK, AlertLevel::Patrol | AlertLevel::Search) if lost => Some(LOST),
            (CHASE, AlertLevel::Attack) if attack_ok => Some(ATTACK),
            (CHASE, AlertLevel::Attack) if done && self.anims[CHASE as usize].is_some() => Some(WAIT),
            (WAIT, AlertLevel::Attack) if attack_ok => Some(ATTACK),
            (ATTACK, AlertLevel::Attack) if done => Some(CHASE),
            _ => None,
        };
        if let Some(n) = next {
            self.enter(cx, n);
        }

        // Searching guards report when done.
        if self.state == SEARCH && self.kind == GuardKind::Flashlight && self.state_time >= SEARCH_DONE_AFTER {
            let g = &mut cx.world.shared.groups[self.group];
            if !g.done_searching.contains(&me_slot) {
                g.done_searching.push(me_slot);
            }
        }

        // The attack lands once per attack state.
        if self.state == ATTACK && !self.hurt_this_attack {
            self.hurt_this_attack = true;
            let o = cx.world.options(cx.me);
            let [a, b] = o.and_then(|o| o.pair(opt::ATTACK_INTERVAL)).unwrap_or([0.2, 0.5]);
            self.attack_ready_at = now + cx.rng.range(a, b.max(a));
            self.land_attack(cx);
        }
        self.track_effects(cx);

        // Movement.
        let goal = match self.state {
            CHASE | WAIT | ATTACK | SPOT => target,
            RETURN => Some(self.start.t),
            GO_SEARCH => self.search_point.map(|(p, _)| p),
            SEARCH => self.search_point.map(|(p, d)| add(p, d)),
            STAND => Some(add(self.start.t, scale(heading(&self.start), 100.0))),
            _ => None,
        };
        if !matches!(self.state, DYING) {
            self.steer(cx, goal);
        }
    }

    fn animation_finished(&mut self, _world: &mut World, _me: Handle, tag: u32) {
        if tag == self.state as u32 {
            self.anim_done = true;
        }
    }

    fn get_option(&self, _world: &World, _me: Handle, _id: u16) -> Option<OptionValue> {
        None
    }
}

impl Guard {
    /// Sneaking music near a flashlight guard (guards.md 4.4).
    fn sneak_music(&mut self, cx: &mut Cx<'_>) {
        if self.kind != GuardKind::Flashlight {
            return;
        }
        let o = cx.world.options(cx.me).cloned().unwrap_or_default();
        let near = Self::target(cx).is_some_and(|(_, p)| {
            if let Some(v) = o.i16(700).and_then(|v| cx.world.find_nearest(v, Some(cx.me))) {
                return volume_contains(cx.world, v, p);
            }
            let d = sub(p, cx.my_position());
            length([d[0], d[1], 0.0]) < o.f32_or(opt::SNEAK_DISTANCE, 2000.0)
                && d[2].abs() < o.f32_or(opt::SNEAK_HEIGHT, 300.0)
        }) && !self.is_down();
        if near != self.sneaking {
            self.sneaking = near;
            if near {
                cx.world.shared.intensity.insert(cx.me.index(), INTENSITY_SNEAK);
            } else {
                cx.world.shared.intensity.remove(&cx.me.index());
            }
        }
    }
}

/// Whether an object counts as a guard for the rules here (inactive objects excluded).
pub fn is_guard_class(class: i16) -> bool {
    matches!(class, 9..=15 | 17..=19 | 74 | 75)
}

/// Every guard slot of a world that is active.
pub fn active_guards(world: &World) -> Vec<u32> {
    world
        .handles()
        .filter(|&h| world.class(h).is_some_and(is_guard_class) && world.flags(h) & flags::ACTIVE != 0)
        .map(|h| h.index())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_effects_throw_by_their_motion() {
        // Times 1.25, raised to 600 up.
        assert_eq!(throw_velocity([100.0, 0.0, 0.0]), [125.0, 0.0, 600.0]);
        // Shortened to 1,200 first.
        let v = throw_velocity([2000.0, 0.0, 0.0]);
        assert!((v[0] - 1200.0).abs() < 1e-3 && v[2] == 600.0);
        assert_eq!(attack_effect(&[Arg::Short(6, 1234), Arg::Int(6)]), Some((1234, damage::THROWN)));
    }
}
