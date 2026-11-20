//! Sly 3's boats and the swimmer (specs/sly3/vehicles-and-minigames.md 7.3 to 7.5): the raft (83) and the jolly
//! boat (84), the swimmer (87) and the ships (88). Each moves only while driven. The biplanes are in `vehicles::air`.
//!
//! - **Raft** (7.3.1): with the stick beyond 0.5 the wanted heading is its camera-relative direction, else the
//!   current heading. Cross pressed within 0.15 s gives a forward stroke, Square a backward one: an acceleration
//!   of ±6,000 (+0x1C0) along the raft's +X for one step. Each step the raft turns toward the wanted heading at the
//!   rate 2.5 (+0x1C4) with a smoothing time of 0.25 s. The water's drag is option 62 per second [I] (0.5 on the
//!   disc), and the raft stays at its height (no water model) [I]. L3 plays sound 42.
//! - **Jolly boat** (7.3.2): the raft's control, then its cannon on R1: idle → charging (charge = 600 + t·(a +
//!   t·b) with the held time t, limited to 600 … 2,600; a = 2,000, b = 0 [I], the fields at +0x244 and +0x248 not
//!   being in the spec) → fire on release or at the limit (a shot of that range, counted) → cooldown 0.5 s.
//! - **Swimmer** (7.4.1, 7.4.2): steering: the stick, scaled by the pan and tilt rates (−1.17, −1.04) and
//!   normalised, sets target turn rates; the actual rates chase them with at most 12 rad/s²; pan and tilt integrate
//!   them, the pan wrapping and the tilt limited to ±π/2. A stroke (state 2: Triangle with the tilt below 0, Cross
//!   above it) sets the push direction to the swimmer's +X or −X and adds 1 to the push amount u when u is below
//!   0.5, then state 3. Each step u falls by 2 per second (within 0..1) and while u > 0 the swimmer is pushed along
//!   the direction by 2,250 × (1 − 0.5·cos(π·u)) (3,375 at u = 1, 1,125 near 0) as an acceleration, against a water
//!   drag of 1.5 per second [I]. State 3 lasts 0.5 s, then state 4 (steering as state 1) for 0.5 s [I: the length of
//!   animation 1009], then back to state 2 [I]. State 5 takes no input. Moving to the clear spot of state 3 is not
//!   done (its point is found by an untraced routine).
//! - **Ship** (7.5.1, 7.5.2): the sailing records of options 1954 to 1961 (one per state: speed 1955, rudder turn
//!   value 1959); a driven ship uses the record of state 2 (the first record when none matches). L2 gives +1 and R2
//!   −1 (both: 0); the rudder eases toward that × the record's turn value at 1 rad/s [I: the easing records 1958 and
//!   1960 are not decoded], 2.5× faster when the input opposes its swing; the turn rate is speed × sin(rudder) /
//!   length (option 1979, default 4,000). The speed is the record's speed × the throttle factor of the 13-value
//!   ladder ([`THROTTLE`]) at |a| / π × 12, a being the heading minus (the wind angle + π); the wind angle is 0 [I:
//!   it is a world value at `0x47A2C8` not in the data].

use super::{Pad, Presses, button, camera_heading, orientation, pad_for, place, wrap};
use crate::behavior::{ClassBehavior, Cx};
use crate::math::{Vec3, cos, sin, sqrt};
use alloc::vec::Vec;

use crate::object::{Arg, Handle};
use crate::world::World;

fn yaw_pitch(world: &World, me: Handle) -> (f32, f32) {
    world.world_transform(me).map_or((0.0, 0.0), |w| super::heading_of(&w.cols))
}

/// A heading and pitch as a unit direction.
pub fn direction(yaw: f32, pitch: f32) -> Vec3 {
    [cos(yaw) * cos(pitch), sin(yaw) * cos(pitch), sin(pitch)]
}

/// Raft and jolly boat stroke acceleration (+0x1C0), turn rate (+0x1C4) and smoothing time.
pub const STROKE: f32 = 6000.0;
pub const RAFT_TURN: f32 = 2.5;
pub const RAFT_TURN_TIME: f32 = 0.25;
/// Jolly boat cannon charge limits (+0x250, +0x254) and cooldown (+0x22C).
pub const CHARGE: [f32; 2] = [600.0, 2600.0];
pub const COOLDOWN: f32 = 0.5;

/// The raft (83) or the jolly boat (84).
#[derive(Clone, Debug, PartialEq)]
pub struct Raft {
    pub jolly: bool,
    pub heading: f32,
    pub wanted: f32,
    pub velocity: Vec3,
    pub drag: f32,
    presses: Presses,
    pub strokes: u32,
    /// Cannon state 0 idle, 1 charging, 2 fire, 3 cooldown; its time and charge; shots fired (their ranges).
    pub cannon: u8,
    cannon_time: f32,
    pub charge: f32,
    pub shots: alloc::vec::Vec<f32>,
}

impl Raft {
    pub fn new(world: &World, me: Handle, jolly: bool) -> Raft {
        let (heading, _) = yaw_pitch(world, me);
        Raft {
            jolly,
            heading,
            wanted: heading,
            velocity: [0.0; 3],
            drag: world.options(me).map_or(0.5, |o| o.f32_or(62, 0.5)),
            presses: Presses::default(),
            strokes: 0,
            cannon: 0,
            cannon_time: 0.0,
            charge: CHARGE[0],
            shots: alloc::vec::Vec::new(),
        }
    }

    /// Paddling and steering for one step, `heading` being the stick's camera-relative heading.
    pub fn paddle(&mut self, pad: &Pad, stick_heading: Option<f32>, dt: f32) {
        self.presses.update(pad, dt);
        self.wanted = match stick_heading {
            Some(h) if pad.magnitude() > 0.5 => h,
            _ => self.heading,
        };
        let fwd = [cos(self.heading), sin(self.heading), 0.0];
        if self.presses.take(button::CROSS, 0.15) {
            self.velocity = crate::math::madd(self.velocity, fwd, STROKE * dt);
            self.strokes += 1;
        } else if self.presses.take(button::SQUARE, 0.15) {
            self.velocity = crate::math::madd(self.velocity, fwd, -STROKE * dt);
            self.strokes += 1;
        }
        // Turn toward the wanted heading at the rate, eased over the smoothing time.
        let e = wrap(self.wanted - self.heading);
        let rate = (e / RAFT_TURN_TIME).clamp(-RAFT_TURN, RAFT_TURN);
        self.heading = wrap(self.heading + rate * dt);
        self.velocity = crate::math::scale(self.velocity, (1.0 - self.drag * dt).max(0.0));
        if self.jolly {
            self.cannon_step(pad, dt);
        }
    }

    fn cannon_step(&mut self, pad: &Pad, dt: f32) {
        self.cannon_time += dt;
        match self.cannon {
            0 if pad.held(button::R1) => {
                self.cannon = 1;
                self.cannon_time = 0.0;
                self.charge = CHARGE[0];
            }
            1 => {
                let t = self.cannon_time;
                self.charge = (CHARGE[0] + t * 2000.0).clamp(CHARGE[0], CHARGE[1]);
                if !pad.held(button::R1) || self.charge >= CHARGE[1] {
                    self.cannon = 2;
                }
            }
            2 => {
                self.shots.push(self.charge);
                self.cannon = 3;
                self.cannon_time = 0.0;
            }
            3 if self.cannon_time >= COOLDOWN => self.cannon = 0,
            _ => {}
        }
    }
}

impl ClassBehavior for Raft {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let driven = super::driven(cx.world, cx.me);
        if !driven && crate::math::length(self.velocity) < 1.0 {
            return;
        }
        let pad = pad_for(cx.world, cx.me);
        let h = camera_heading(cx.world, pad.stick);
        if driven && pad.pressed & button::L3 != 0 {
            cx.world.play_sound(42, Some(cx.me));
        }
        self.paddle(&pad, h, cx.dt);
        let Some(p) = cx.world.position(cx.me) else { return };
        place(cx.world, cx.me, crate::math::madd(p, self.velocity, cx.dt), orientation(self.heading, 0.0, 0.0));
    }
}

/// The swimmer's pan and tilt rate scales (+0x1EC, +0x1F0), turn acceleration limit and tilt limit.
pub const SWIM_RATES: [f32; 2] = [-1.17, -1.04];
pub const SWIM_ACCEL: f32 = 12.0;
pub const SWIM_TILT: f32 = core::f32::consts::FRAC_PI_2;
/// The push: base strength, the decay of the push amount per second, and the water drag (working assumption).
pub const PUSH: f32 = 2250.0;
pub const PUSH_DECAY: f32 = 2.0;
pub const SWIM_DRAG: f32 = 1.5;

/// The push strength for push amount `u` (7.4.2): 2,250 × (1 − 0.5·cos(π·u)).
pub fn push_strength(u: f32) -> f32 {
    PUSH * (1.0 - 0.5 * cos(core::f32::consts::PI * u))
}

/// The swimmer (87).
#[derive(Clone, Debug, PartialEq)]
pub struct Swimmer {
    pub pan: f32,
    pub tilt: f32,
    pub rates: [f32; 2],
    /// State: 1 aim, 2 swim, 3 after a stroke, 4 recovering, 5 no input.
    pub state: u8,
    state_time: f32,
    /// Push amount u and direction (+1 forward along +X, −1 back).
    pub push: f32,
    pub push_dir: f32,
    pub velocity: Vec3,
    pub strokes: u32,
}

impl Swimmer {
    pub fn new(world: &World, me: Handle) -> Swimmer {
        let (pan, tilt) = yaw_pitch(world, me);
        Swimmer {
            pan,
            tilt,
            rates: [0.0; 2],
            state: 2,
            state_time: 0.0,
            push: 0.0,
            push_dir: 1.0,
            velocity: [0.0; 3],
            strokes: 0,
        }
    }

    /// Steering for one step (states 1, 2 and 4).
    pub fn steer(&mut self, pad: &Pad, dt: f32) {
        let mut t = [pad.stick[0] * SWIM_RATES[0], pad.stick[1] * SWIM_RATES[1]];
        let n = sqrt(pad.stick[0] * pad.stick[0] + pad.stick[1] * pad.stick[1]);
        if n > 1.0 {
            t = [t[0] / n, t[1] / n];
        }
        for (r, w) in self.rates.iter_mut().zip(t) {
            let step = SWIM_ACCEL * dt;
            *r += (w - *r).clamp(-step, step);
        }
        self.pan = wrap(self.pan + self.rates[0] * dt);
        self.tilt = (self.tilt - self.rates[1] * dt).clamp(-SWIM_TILT, SWIM_TILT);
    }

    /// One step of input, states and the push; returns the velocity.
    pub fn step(&mut self, pad: &Pad, dt: f32) -> Vec3 {
        self.state_time += dt;
        match self.state {
            1 | 4 => self.steer(pad, dt),
            2 => {
                self.steer(pad, dt);
                let stroke = (pad.pressed & button::TRIANGLE != 0 && self.tilt < 0.0)
                    || (pad.pressed & button::CROSS != 0 && self.tilt > 0.0);
                if stroke {
                    self.push_dir = if pad.pressed & button::TRIANGLE != 0 && self.tilt < 0.0 { 1.0 } else { -1.0 };
                    if self.push < 0.5 {
                        self.push += 1.0;
                    }
                    self.strokes += 1;
                    self.state = 3;
                    self.state_time = 0.0;
                }
            }
            _ => {}
        }
        if self.state == 3 && self.state_time >= 0.5 {
            self.state = 4;
            self.state_time = 0.0;
        } else if self.state == 4 && self.state_time >= 0.5 {
            self.state = 2;
            self.state_time = 0.0;
        }
        self.push = (self.push - PUSH_DECAY * dt).clamp(0.0, 1.0);
        let a = if self.push > 0.0 { push_strength(self.push) * self.push_dir } else { 0.0 };
        let fwd = direction(self.pan, self.tilt);
        self.velocity = crate::math::madd(self.velocity, fwd, a * dt);
        self.velocity = crate::math::scale(self.velocity, (1.0 - SWIM_DRAG * dt).max(0.0));
        self.velocity
    }
}

impl ClassBehavior for Swimmer {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let driven = super::driven(cx.world, cx.me);
        if !driven && crate::math::length(self.velocity) < 1.0 {
            return;
        }
        let pad = if self.state == 5 { Pad::default() } else { pad_for(cx.world, cx.me) };
        if driven && self.state != 5 && pad.pressed & button::L3 != 0 {
            cx.world.play_sound(42, Some(cx.me));
        }
        let v = self.step(&pad, cx.dt);
        let Some(p) = cx.world.position(cx.me) else { return };
        place(cx.world, cx.me, crate::math::madd(p, v, cx.dt), orientation(self.pan, self.tilt, 0.0));
    }
}

/// The rudder's easing rate (working assumption) and the default turn value.
pub const RUDDER_RATE: f32 = 1.0;
pub const RUDDER: f32 = 0.5;
/// The throttle ladder of 13 factors over |a| = 0 to π (7.5.2): a sailing polar.
pub const THROTTLE: [f32; 13] = [0.3, 0.5, 0.743, 0.9, 1.1, 1.2, 1.55, 1.5, 1.45, 1.4, 1.35, 1.3, 1.3];

/// The throttle factor for the angle `a` between the heading and the wind's back (wrapped to ±π): the ladder at
/// |a| / π × 12, interpolated.
pub fn throttle(a: f32) -> f32 {
    let x = (wrap(a).abs() / core::f32::consts::PI * 12.0).clamp(0.0, 12.0);
    let i = (x as usize).min(11);
    let f = x - i as f32;
    THROTTLE[i] + (THROTTLE[i + 1] - THROTTLE[i]) * f
}

/// One sailing record (options 1954 to 1961).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SailRecord {
    pub state: i32,
    pub speed: f32,
    pub distance: f32,
    pub factor: f32,
    pub turn: f32,
}

/// A ship's sailing records from its option-list actions: 1954 (state) opens a record, 1955 to 1960 fill it, 1961
/// closes it; at most 4.
pub fn sail_records(actions: &[(u16, &[Arg])]) -> Vec<SailRecord> {
    let num = |a: &[Arg]| match a.first() {
        Some(Arg::Float(f)) => Some(*f),
        Some(Arg::Int(i)) => Some(*i as f32),
        Some(Arg::Short(_, s)) => Some(f32::from(*s)),
        _ => None,
    };
    let mut out = Vec::new();
    let mut open: Option<SailRecord> = None;
    for &(id, args) in actions {
        match id {
            1954 => {
                let state = num(args).unwrap_or(0.0) as i32;
                open = Some(SailRecord { state, speed: 1000.0, distance: 0.0, factor: 1.0, turn: RUDDER });
            }
            1955 => open.iter_mut().for_each(|r| r.speed = num(args).unwrap_or(r.speed)),
            1956 => open.iter_mut().for_each(|r| r.distance = num(args).unwrap_or(r.distance)),
            1957 => open.iter_mut().for_each(|r| r.factor = num(args).unwrap_or(r.factor)),
            1959 => open.iter_mut().for_each(|r| r.turn = num(args).unwrap_or(r.turn)),
            1961 => {
                if let Some(r) = open.take()
                    && out.len() < 4
                {
                    out.push(r);
                }
            }
            _ => {}
        }
    }
    out
}

/// A sailing ship (88).
#[derive(Clone, Debug, PartialEq)]
pub struct Ship {
    pub length: f32,
    pub records: Vec<SailRecord>,
    pub record: SailRecord,
    pub speed: f32,
    pub rudder: f32,
    pub heading: f32,
    /// The wind angle (a world value not in the data: 0).
    pub wind: f32,
}

impl Ship {
    pub fn new(world: &World, me: Handle) -> Ship {
        let o = world.options(me);
        let slot = me.index();
        let actions: Vec<(u16, &[Arg])> = world.sly3.as_deref().map_or_else(Vec::new, |l| {
            l.actions.iter().filter(|(s, _, _)| *s == slot).map(|(_, id, a)| (*id, &a[..])).collect()
        });
        let records = sail_records(&actions);
        let fallback = SailRecord {
            state: 2,
            speed: o.map_or(1000.0, |o| o.f32_or(1955, 1000.0)),
            distance: 0.0,
            factor: 1.0,
            turn: RUDDER,
        };
        let record = records.iter().find(|r| r.state == 2).or(records.first()).copied().unwrap_or(fallback);
        Ship {
            length: o.map_or(4000.0, |o| o.f32_or(1979, 4000.0)),
            records,
            record,
            speed: 0.0,
            rudder: 0.0,
            heading: yaw_pitch(world, me).0,
            wind: 0.0,
        }
    }

    /// Rudder, speed and turn for one step.
    pub fn steer(&mut self, pad: &Pad, dt: f32) {
        let input = match (pad.held(button::L2), pad.held(button::R2)) {
            (true, false) => 1.0,
            (false, true) => -1.0,
            _ => 0.0,
        };
        let want = input * self.record.turn;
        let e = want - self.rudder;
        let rate = if e * self.rudder < 0.0 { RUDDER_RATE * 2.5 } else { RUDDER_RATE };
        self.rudder += e.clamp(-rate * dt, rate * dt);
        self.speed = self.record.speed * throttle(self.heading - (self.wind + core::f32::consts::PI));
        self.heading = wrap(self.heading + self.speed * sin(self.rudder) / self.length * dt);
    }
}

impl ClassBehavior for Ship {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if !super::driven(cx.world, cx.me) {
            return;
        }
        let pad = pad_for(cx.world, cx.me);
        self.steer(&pad, cx.dt);
        let Some(p) = cx.world.position(cx.me) else { return };
        let v = [cos(self.heading) * self.speed, sin(self.heading) * self.speed, 0.0];
        place(cx.world, cx.me, crate::math::madd(p, v, cx.dt), orientation(self.heading, 0.0, 0.0));
    }
}
