//! Sly 3's biplanes (class 82, specs/sly3/vehicles-and-minigames.md 7.2).
//!
//! **Flight step** (7.2.1, all modes): the plane eases toward a speed command and toward heading and pitch targets
//! (pitch limited to ±70°), banks by its bank command, and its velocity is set directly along its heading and pitch
//! (no forces). Working assumption [I]: heading and pitch ease with a time constant of 0.5 s, the speed with gain 2
//! per second; no take-off ramp.
//!
//! **Modes** (option 1686): 0 flown by the player, 1 the computer pilot, 2 flying to a point. A plane the player
//! drives is flown in mode 0 whatever its option; a plane nobody drives with mode 0 stays put.
//!
//! - **Player** (7.2.1): the cruise speed (1,800), × 1.6 holding Cross or × 0.7 holding Square; stick axes below 0.2
//!   count as 0; stick x banks (x × 65°) and moves the heading target to heading − x × the turn angle (option 1694,
//!   default 30°); stick y moves the pitch target to pitch + y × 15°.
//! - **Computer** (7.2.2): every 0.5 s the chase target is re-chosen among the planes of another side by the lowest
//!   score ([`chase_score`]); every 0.25 s the threat, the nearest plane of another side that is player-flown or
//!   targets this one (the current threat −500), dropped after 2.5 s without one. Sub-modes: cruise, chase, and evade
//!   once a threat has lasted 1 s. Chasing re-aims at random intervals at the target's predicted position (lead and
//!   re-aim delay 0.25–1.5 s for a player's plane, else 0.25–0.5 s; aim offset −1,500 or +4,500 along the target's
//!   axis); the speed blends from the cruise toward the fast speed by distance × 0.0001. Evading picks one of 8
//!   manoeuvres every 2 s (1 s with probability 1/3, [`evasion`]). The bank target is the heading error limited to
//!   65°. Avoidance of other planes and the world's height limits are not applied (their records are not read) [I].
//! - **To a point** (mode 2): toward option 1699's object [I: the point at +0x3A0 is not optioned]; speed unchanged.
//!
//! **Shots** (7.2.2): guns are the option-1701 entries (template, effect), used in turn; a shot leaves the nose at
//! the forward axis × the shot speed (1703, default 15,000) plus the plane's velocity, at most the rate (1702, default
//! 10 per second). The player fires holding R1 (ammunition not limited [I]); the computer fires in bursts (gaps
//! 0.5–1.5 s, windows 1.5–2 s) while its target is within 30° of the nose and 8,000 units [I: the firing zone's
//! object is not read]. Shots are recorded ([`Shot`]); their projectiles are not spawned yet.

use alloc::vec::Vec;

use super::{Pad, button, orientation, pad_for, place, wrap};
use crate::behavior::{ClassBehavior, Cx};
use crate::math::{Vec3, atan2, cos, dot, length, sin, sqrt, sub};
use crate::object::{Arg, Handle};
use crate::world::World;

/// One degree in radians (the spec gives the angles as exact degrees).
const DEGREE: f32 = core::f32::consts::PI / 180.0;
/// Cruise speed (+0x19C), Cross and Square factors (+0x1A0, +0x1A4).
pub const CRUISE: f32 = 1800.0;
pub const BOOST: f32 = 1.6;
pub const BRAKE: f32 = 0.7;
/// Bank (+0x1B0, 65°), pitch step (+0x1B4, 15°), pitch limit (+0x1AC, 70°), turn angle (+0x1B8, 30°).
pub const BANK: f32 = 65.0 * DEGREE;
pub const PITCH_STEP: f32 = 15.0 * DEGREE;
pub const PITCH_LIMIT: f32 = 70.0 * DEGREE;
pub const TURN_ANGLE: f32 = core::f32::consts::FRAC_PI_6;
/// Stick dead zone.
pub const DEAD_ZONE: f32 = 0.2;
/// Shot speed (+0x280) and rate (+0x27C) defaults.
pub const SHOT_SPEED: f32 = 15_000.0;
pub const SHOT_RATE: f32 = 10.0;

/// The computer's chase score of another plane (lower is better): its distance, plus the cone penalty
/// (1 − u)·500 + u·3,000 when its bearing is outside ±`cone` (u = (bearing − cone)/(π − cone)), −1,000 for the
/// current target and −750 for a player-flown plane.
pub fn chase_score(distance: f32, bearing: f32, cone: f32, current: bool, player: bool) -> f32 {
    use core::f32::consts::PI;
    let b = bearing.abs();
    let mut s = distance;
    if b > cone {
        let u = ((b - cone) / (PI - cone)).clamp(0.0, 1.0);
        s += (1.0 - u) * 500.0 + u * 3000.0;
    }
    if current {
        s -= 1000.0;
    }
    if player {
        s -= 750.0;
    }
    s
}

/// Evasion manoeuvre `k` (0 to 7) from two random draws in 0..1: (heading change in turn steps, pitch target, speed
/// factor). The heading turns by −0.6, 0 or +0.6 turn steps; the pitch target is +70°, −70° or 0; the speed lies
/// between the slow and fast factors.
pub fn evasion(k: u32, r: f32) -> (f32, f32, f32) {
    let heading = [-0.6, 0.0, 0.6][(k % 3) as usize];
    let pitch = [PITCH_LIMIT, -PITCH_LIMIT, 0.0][((k / 3) % 3) as usize];
    (heading, pitch, BRAKE + (BOOST - BRAKE) * r)
}

/// One shot: where it left and its velocity, and which gun entry fired it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub from: Vec3,
    pub velocity: Vec3,
    pub gun: usize,
}

/// What a plane knows of another for the computer pilot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub plane: Handle,
    pub position: Vec3,
    pub forward: Vec3,
    pub side: i32,
    pub player: bool,
    /// Whether that plane targets this one.
    pub targets_me: bool,
}

/// A biplane.
#[derive(Clone, Debug, PartialEq)]
pub struct Biplane {
    pub mode: i32,
    pub side: i32,
    pub cruise: f32,
    pub turn_angle: f32,
    pub heading: f32,
    pub pitch: f32,
    pub bank: f32,
    pub speed: f32,
    pub velocity: Vec3,
    shot_speed: f32,
    shot_rate: f32,
    guns: usize,
    next_gun: usize,
    shot_wait: f32,
    pub shots: Vec<Shot>,
    /// Shots already sent as projectiles.
    launched: usize,
    // The computer pilot.
    pub target: Option<Handle>,
    pub threat: Option<Handle>,
    threat_time: f32,
    no_threat_time: f32,
    pub sub_mode: u8,
    pick_wait: f32,
    threat_wait: f32,
    aim_wait: f32,
    aim: Option<Vec3>,
    evade_wait: f32,
    evade: (f32, f32, f32),
    burst_wait: f32,
    burst_left: f32,
    point: Option<Handle>,
}

impl Biplane {
    pub fn new(world: &World, me: Handle) -> Biplane {
        let (heading, pitch) = world.world_transform(me).map_or((0.0, 0.0), |w| super::heading_of(&w.cols));
        let o = world.options(me);
        let guns = world.sly3.as_deref().map_or(0, |l| {
            let slot = me.index();
            l.actions.iter().filter(|(s, id, _)| *s == slot && *id == 1701).count()
        });
        let point = o.and_then(|o| o.i32(1699)).and_then(|id| world.find_sly3_id(id, Some(me)));
        Biplane {
            mode: o.and_then(|o| o.i32(1686)).unwrap_or(0),
            side: o.and_then(|o| o.i32(1684)).unwrap_or(0),
            cruise: CRUISE,
            turn_angle: o.map_or(TURN_ANGLE, |o| o.f32_or(1694, TURN_ANGLE)),
            heading,
            pitch,
            bank: 0.0,
            speed: CRUISE,
            velocity: [0.0; 3],
            shot_speed: o.map_or(SHOT_SPEED, |o| o.f32_or(1703, SHOT_SPEED)),
            shot_rate: o.map_or(SHOT_RATE, |o| o.f32_or(1702, SHOT_RATE)).max(0.1),
            guns: guns.max(1),
            next_gun: 0,
            shot_wait: 0.0,
            shots: Vec::new(),
            launched: 0,
            target: None,
            threat: None,
            threat_time: 0.0,
            no_threat_time: 0.0,
            sub_mode: 0,
            pick_wait: 0.0,
            threat_wait: 0.0,
            aim_wait: 0.0,
            aim: None,
            evade_wait: 0.0,
            evade: (0.0, 0.0, 1.0),
            burst_wait: 0.5,
            burst_left: 0.0,
            point,
        }
    }

    fn forward(&self) -> Vec3 {
        [cos(self.heading) * cos(self.pitch), sin(self.heading) * cos(self.pitch), sin(self.pitch)]
    }

    /// The flight step toward a speed command and heading and pitch targets.
    fn apply(&mut self, command: f32, heading_target: f32, pitch_target: f32, dt: f32) -> Vec3 {
        let pitch_target = pitch_target.clamp(-PITCH_LIMIT, PITCH_LIMIT);
        let k = (dt / 0.5).min(1.0);
        self.heading = wrap(self.heading + wrap(heading_target - self.heading) * k);
        self.pitch += (pitch_target - self.pitch) * k;
        self.speed += (command - self.speed) * (2.0 * dt).min(1.0);
        self.velocity = crate::math::scale(self.forward(), self.speed);
        self.velocity
    }

    /// One step of player flight; returns the velocity.
    pub fn fly(&mut self, pad: &Pad, dt: f32) -> Vec3 {
        let mut command = self.cruise;
        if pad.held(button::CROSS) {
            command *= BOOST;
        } else if pad.held(button::SQUARE) {
            command *= BRAKE;
        }
        let dz = |v: f32| if v.abs() < DEAD_ZONE { 0.0 } else { v };
        let (x, y) = (dz(pad.stick[0]), dz(pad.stick[1]));
        self.bank = x * BANK;
        let (h, p) = (self.heading - x * self.turn_angle, self.pitch + y * PITCH_STEP);
        self.apply(command, h, p, dt)
    }

    /// Heading and pitch toward a point.
    fn toward(&self, from: Vec3, to: Vec3) -> (f32, f32) {
        let d = sub(to, from);
        (atan2(d[1], d[0]), atan2(d[2], sqrt(d[0] * d[0] + d[1] * d[1])))
    }

    /// One step of the computer pilot against the other planes; returns the velocity.
    pub fn pilot(&mut self, me: Vec3, others: &[Contact], rand: &mut dyn FnMut() -> f32, dt: f32) -> Vec3 {
        let enemies: Vec<&Contact> = others.iter().filter(|c| c.side != self.side).collect();
        self.pick_wait -= dt;
        if self.pick_wait <= 0.0 {
            self.pick_wait = 0.5;
            let fwd = self.forward();
            self.target = enemies
                .iter()
                .map(|c| {
                    let to = sub(c.position, me);
                    let d = length(to);
                    let bearing = if d > 1e-3 { crate::math::acos(dot(to, fwd) / d) } else { 0.0 };
                    (c.plane, chase_score(d, bearing, self.turn_angle, Some(c.plane) == self.target, c.player))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|x| x.0);
        }
        self.threat_wait -= dt;
        if self.threat_wait <= 0.0 {
            self.threat_wait = 0.25;
            let found = enemies
                .iter()
                .filter(|c| c.player || c.targets_me)
                .map(|c| {
                    (c.plane, length(sub(c.position, me)) - if Some(c.plane) == self.threat { 500.0 } else { 0.0 })
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|x| x.0);
            match found {
                Some(t) => {
                    if self.threat != Some(t) {
                        self.threat_time = 0.0;
                    }
                    self.threat = Some(t);
                    self.no_threat_time = 0.0;
                }
                None => {
                    self.no_threat_time += 0.25;
                    if self.no_threat_time >= 2.5 {
                        self.threat = None;
                    }
                }
            }
        }
        if self.threat.is_some() {
            self.threat_time += dt;
        } else {
            self.threat_time = 0.0;
        }
        self.sub_mode = if self.threat.is_some() && self.threat_time > 1.0 {
            2
        } else if self.target.is_some() {
            1
        } else {
            0
        };
        let target = self.target.and_then(|t| enemies.iter().find(|c| c.plane == t).copied());
        let (command, h, p) = match (self.sub_mode, target) {
            (2, _) => {
                self.evade_wait -= dt;
                if self.evade_wait <= 0.0 {
                    self.evade_wait = if rand() < 1.0 / 3.0 { 1.0 } else { 2.0 };
                    let k = (rand() * 8.0) as u32 % 8;
                    self.evade = evasion(k, rand());
                }
                let (dh, pitch, f) = self.evade;
                (self.cruise * f, self.heading + dh * self.turn_angle, pitch)
            }
            (1, Some(t)) => {
                self.aim_wait -= dt;
                if self.aim_wait <= 0.0 || self.aim.is_none() {
                    let (lo, hi) = if t.player { (0.25, 1.5) } else { (0.25, 0.5) };
                    let lead = lo + (hi - lo) * rand();
                    self.aim_wait = lo + (hi - lo) * rand();
                    let offset = if t.player { -1500.0 } else { 4500.0 };
                    let ahead = crate::math::scale(t.forward, self.cruise * lead + offset);
                    self.aim = Some(crate::math::add(t.position, ahead));
                }
                let aim = self.aim.unwrap_or(t.position);
                let (h, p) = self.toward(me, aim);
                let blend = (length(sub(t.position, me)) * 0.0001).clamp(0.0, 1.0);
                (self.cruise * (1.0 + (BOOST - 1.0) * blend), h, p)
            }
            _ => (self.cruise, self.heading, 0.0),
        };
        self.bank = wrap(h - self.heading).clamp(-BANK, BANK);
        self.apply(command, h, p, dt)
    }

    /// Fires a shot when allowed (`trigger`) and the rate permits.
    pub fn guns(&mut self, from: Vec3, trigger: bool, dt: f32) {
        self.shot_wait -= dt;
        if trigger && self.shot_wait <= 0.0 {
            let v = crate::math::madd(self.velocity, self.forward(), self.shot_speed);
            self.shots.push(Shot { from, velocity: v, gun: self.next_gun });
            self.next_gun = (self.next_gun + 1) % self.guns;
            self.shot_wait = 1.0 / self.shot_rate;
        }
    }

    /// Sends the shots fired since the last call as projectiles (1.5 s of flight, 10 damage [I]).
    fn launch(&mut self, world: &mut World, me: Handle) {
        for s in &self.shots[self.launched..] {
            super::shots::fire(
                world,
                super::shots::Projectile {
                    position: crate::math::madd(s.from, s.velocity, 0.01),
                    velocity: s.velocity,
                    life: 1.5,
                    owner: me,
                    damage: 10.0,
                    gravity: 0.0,
                },
            );
        }
        self.launched = self.shots.len();
    }

    /// The computer's burst window: whether it may fire now.
    pub fn burst(&mut self, rand: &mut dyn FnMut() -> f32, dt: f32) -> bool {
        if self.burst_left > 0.0 {
            self.burst_left -= dt;
            if self.burst_left <= 0.0 {
                self.burst_wait = 0.5 + rand();
            }
            return true;
        }
        self.burst_wait -= dt;
        if self.burst_wait <= 0.0 {
            self.burst_left = 1.5 + 0.5 * rand();
        }
        false
    }
}

/// The other planes as this one sees them.
fn contacts(world: &World, me: Handle) -> Vec<Contact> {
    world
        .handles()
        .filter(|&h| h != me && world.class(h) == Some(82) && world.is_active(h))
        .filter_map(|h| {
            let w = world.world_transform(h)?;
            let o = world.options(h);
            let player = world.shared.driving == Some(h);
            Some(Contact {
                plane: h,
                position: w.t,
                forward: w.cols[0],
                side: o.and_then(|o| o.i32(1684)).unwrap_or(0),
                player,
                targets_me: false,
            })
        })
        .collect()
}

impl ClassBehavior for Biplane {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let driven = super::driven(cx.world, cx.me);
        let Some(p) = cx.world.position(cx.me) else { return };
        let dt = cx.dt;
        let v = if driven {
            let pad = pad_for(cx.world, cx.me);
            let v = self.fly(&pad, dt);
            self.guns(p, pad.held(button::R1), dt);
            self.launch(cx.world, cx.me);
            v
        } else {
            match self.mode {
                1 => {
                    let others = contacts(cx.world, cx.me);
                    let rng = &mut *cx.rng;
                    let mut rand = || rng.next_f32();
                    let v = self.pilot(p, &others, &mut rand, dt);
                    let window = self.burst(&mut rand, dt);
                    let on_target = self.target.and_then(|t| cx.world.position(t)).is_some_and(|t| {
                        let to = sub(t, p);
                        let d = length(to);
                        d < 8000.0 && d > 1.0 && dot(to, self.forward()) / d > cos(self.turn_angle)
                    });
                    self.guns(p, window && on_target, dt);
                    self.launch(cx.world, cx.me);
                    v
                }
                2 => {
                    let goal = self.point.and_then(|h| cx.world.position(h));
                    let (h, pitch) = goal.map_or((self.heading, self.pitch), |g| self.toward(p, g));
                    self.bank = wrap(h - self.heading).clamp(-BANK, BANK);
                    let s = self.speed;
                    self.apply(s, h, pitch, dt)
                }
                _ => return,
            }
        };
        place(cx.world, cx.me, crate::math::madd(p, v, dt), orientation(self.heading, self.pitch, self.bank));
    }
}

/// Gun entries of option 1701 for a plane (template, effect): its option-list actions in order.
pub fn gun_entries(world: &World, me: Handle) -> Vec<(i32, i32)> {
    let Some(l) = world.sly3.as_deref() else { return Vec::new() };
    let slot = me.index();
    l.actions
        .iter()
        .filter(|(s, id, _)| *s == slot && *id == 1701)
        .map(|(_, _, a)| {
            let v = |i: usize| match a.get(i) {
                Some(Arg::Int(x)) => *x,
                Some(Arg::Short(_, x)) => i32::from(*x),
                _ => -1,
            };
            (v(0), v(1))
        })
        .collect()
}
