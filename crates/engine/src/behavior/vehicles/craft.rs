//! Sly 3's flying and swimming machines (specs/sly3/vehicles-and-minigames.md 7.2 to 7.5): biplanes (82), the raft
//! (83) and the jolly boat (84), the swimmer (87) and the ships (88). Each moves only while driven.
//!
//! - **Biplane** (7.2.1, player flight): the speed command is the cruise speed (1,800), × 1.6 holding Cross or ×
//!   0.7 holding Square; stick axes below 0.2 count as 0; stick x banks (x × 65°) and moves the heading target to
//!   heading − x × the turn angle (option 1694, default 30°); stick y moves the pitch target to pitch + y × 15°,
//!   limited to ±70°. The plane's velocity is then set directly along its heading and pitch (no forces). Holding R1
//!   fires the guns (counted). Working assumption [I]: heading and pitch move toward their targets with a time
//!   constant of 0.5 s, the speed toward its command with gain 2 per second; no take-off ramp.
//! - **Raft** (7.3.1): with the stick beyond 0.5 the wanted heading is its camera-relative direction, else the
//!   current heading. Cross pressed within 0.15 s gives a forward stroke, Square a backward one: an acceleration
//!   of ±6,000 (+0x1C0) along the raft's +X for one step. Each step the raft turns toward the wanted heading at the
//!   rate 2.5 (+0x1C4) with a smoothing time of 0.25 s. The water's drag is option 62 per second [I] (0.5 on the
//!   disc), and the raft stays at its height (no water model) [I]. L3 plays sound 42.
//! - **Jolly boat** (7.3.2): the raft's control, then its cannon on R1: idle → charging (charge = 600 + t·(a +
//!   t·b) with the held time t, limited to 600 … 2,600; a = 2,000, b = 0 [I], the fields at +0x244 and +0x248 not
//!   being in the spec) → fire on release or at the limit (a shot of that range, counted) → cooldown 0.5 s.
//! - **Swimmer** (7.4.1, steering): the stick, scaled by the pan and tilt rates (−1.17, −1.04) and normalised, sets
//!   target turn rates; the actual rates chase them with at most 12 rad/s²; pan and tilt integrate them, the pan
//!   wrapping and the tilt limited to ±π/2. Triangle with the tilt negative and Cross with it positive push it back
//!   toward level (by 0.5 rad over the stroke [I]). The swimmer moves forward along its pan and tilt at 400 units/s
//!   [I] (the swim speed is not in the spec).
//! - **Ship** (7.5.1): L2 gives +1 and R2 −1 (both: 0); the rudder eases toward that × the turn value (0.5 rad
//!   [I]) at 1 rad/s [I], 2.5× faster when the input opposes its swing; the turn rate is speed × sin(rudder) /
//!   length (option 1979, default 4,000); the speed is the ship's speed (option 1955, else 1,000 [I]) × a throttle
//!   of 1 [I].

use super::{Pad, Presses, button, camera_heading, orientation, pad_for, place, wrap};
use crate::behavior::{ClassBehavior, Cx};
use crate::math::{Vec3, cos, sin, sqrt};
use crate::object::Handle;
use crate::world::World;

fn yaw_pitch(world: &World, me: Handle) -> (f32, f32) {
    world.world_transform(me).map_or((0.0, 0.0), |w| super::heading_of(&w.cols))
}

/// A heading and pitch as a unit direction.
pub fn direction(yaw: f32, pitch: f32) -> Vec3 {
    [cos(yaw) * cos(pitch), sin(yaw) * cos(pitch), sin(pitch)]
}

/// A biplane (class 82).
#[derive(Clone, Debug, PartialEq)]
pub struct Biplane {
    pub cruise: f32,
    pub turn_angle: f32,
    pub heading: f32,
    pub pitch: f32,
    pub bank: f32,
    pub speed: f32,
    pub shots: u32,
}

/// One degree in radians (the spec gives the biplane's angles as exact degrees).
const DEGREE: f32 = core::f32::consts::PI / 180.0;
/// Biplane defaults (7.2): cruise speed, Cross and Square factors, bank (65°), pitch step (15°) and limit (70°).
pub const CRUISE: f32 = 1800.0;
pub const BOOST: f32 = 1.6;
pub const BRAKE: f32 = 0.7;
pub const BANK: f32 = 65.0 * DEGREE;
pub const PITCH_STEP: f32 = 15.0 * DEGREE;
pub const PITCH_LIMIT: f32 = 70.0 * DEGREE;
/// The default turn angle (+0x1B8): 30°.
pub const TURN_ANGLE: f32 = core::f32::consts::FRAC_PI_6;
pub const DEAD_ZONE: f32 = 0.2;

impl Biplane {
    pub fn new(world: &World, me: Handle) -> Biplane {
        let (heading, pitch) = yaw_pitch(world, me);
        Biplane {
            cruise: CRUISE,
            turn_angle: world.options(me).map_or(TURN_ANGLE, |o| o.f32_or(1694, TURN_ANGLE)),
            heading,
            pitch,
            bank: 0.0,
            speed: CRUISE,
            shots: 0,
        }
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
        let heading_target = self.heading - x * self.turn_angle;
        let pitch_target = (self.pitch + y * PITCH_STEP).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        let k = (dt / 0.5).min(1.0);
        self.heading = wrap(self.heading + wrap(heading_target - self.heading) * k);
        self.pitch += (pitch_target - self.pitch) * k;
        self.speed += (command - self.speed) * (2.0 * dt).min(1.0);
        if pad.held(button::R1) {
            self.shots += 1;
        }
        crate::math::scale(direction(self.heading, self.pitch), self.speed)
    }
}

impl ClassBehavior for Biplane {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if !super::driven(cx.world, cx.me) {
            return;
        }
        let pad = pad_for(cx.world, cx.me);
        let v = self.fly(&pad, cx.dt);
        let Some(p) = cx.world.position(cx.me) else { return };
        place(cx.world, cx.me, crate::math::madd(p, v, cx.dt), orientation(self.heading, self.pitch, self.bank));
    }
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
/// Swim speed (working assumption).
pub const SWIM_SPEED: f32 = 400.0;

/// The swimmer (87).
#[derive(Clone, Debug, PartialEq)]
pub struct Swimmer {
    pub pan: f32,
    pub tilt: f32,
    pub rates: [f32; 2],
    pub strokes: u32,
}

impl Swimmer {
    pub fn new(world: &World, me: Handle) -> Swimmer {
        let (pan, tilt) = yaw_pitch(world, me);
        Swimmer { pan, tilt, rates: [0.0; 2], strokes: 0 }
    }

    /// Steering for one step.
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
        if (pad.pressed & button::TRIANGLE != 0 && self.tilt < 0.0)
            || (pad.pressed & button::CROSS != 0 && self.tilt > 0.0)
        {
            self.tilt -= self.tilt.signum() * self.tilt.abs().min(0.5);
            self.strokes += 1;
        }
    }
}

impl ClassBehavior for Swimmer {
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
        let v = crate::math::scale(direction(self.pan, self.tilt), SWIM_SPEED);
        place(cx.world, cx.me, crate::math::madd(p, v, cx.dt), orientation(self.pan, self.tilt, 0.0));
    }
}

/// Ship rudder turn value and easing rate (working assumptions).
pub const RUDDER: f32 = 0.5;
pub const RUDDER_RATE: f32 = 1.0;

/// A sailing ship (88).
#[derive(Clone, Debug, PartialEq)]
pub struct Ship {
    pub length: f32,
    pub speed: f32,
    pub rudder: f32,
    pub heading: f32,
}

impl Ship {
    pub fn new(world: &World, me: Handle) -> Ship {
        let o = world.options(me);
        Ship {
            length: o.map_or(4000.0, |o| o.f32_or(1979, 4000.0)),
            speed: o.map_or(1000.0, |o| o.f32_or(1955, 1000.0)),
            rudder: 0.0,
            heading: yaw_pitch(world, me).0,
        }
    }

    /// Rudder and turn for one step.
    pub fn steer(&mut self, pad: &Pad, dt: f32) {
        let input = match (pad.held(button::L2), pad.held(button::R2)) {
            (true, false) => 1.0,
            (false, true) => -1.0,
            _ => 0.0,
        };
        let want = input * RUDDER;
        let e = want - self.rudder;
        let rate = if e * self.rudder < 0.0 { RUDDER_RATE * 2.5 } else { RUDDER_RATE };
        self.rudder += e.clamp(-rate * dt, rate * dt);
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
