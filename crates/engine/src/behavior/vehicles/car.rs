//! Wheeled machines: Sly 3's truck (81) and remote cars (85) (specs/sly3/vehicles-and-minigames.md 7.1), and Sly 2's
//! combat car (77) with its ammunition pickups (108) (specs/sly2/vehicles-and-minigames.md 6).
//!
//! **Truck and remote cars** (mode 1, driven): the steering angle eases toward −(stick x) × the steering limit
//! (1624 / 1814, default 0.8 rad); Cross gives the forward force (1625 / 1815, default 1,100), otherwise Circle or
//! Square the reverse force (1816, default 700; the truck has no reverse option and uses the default). On the cars
//! a press of L3 (within 0.15 s) plays sound 42. Not driven: mode 0, steering and force 0 (computer driving along a
//! curve needs the curve, which no option names [I]). Gravity is option 35 (truck −4,500, cars −900 or −2,000).
//! Working assumptions [I]: the steering smoothing record at `0x003292E8` is not in the spec, so (4, 0, 0.25) rad/s;
//! the force acts as an acceleration along the machine's forward axis against a drag of 1.5 per second; the turn
//! follows a bicycle model, rate = speed × tan(steer) / wheelbase, the wheelbase being the distance between the
//! front and back wheel pairs (options 1618 to 1621 / 1810 to 1813), else 300.
//!
//! **Combat car**: health option 1426 (default 100); a hit costs 10 [I] (a blast 20 [I]); at 0 it is defeated
//! (mode 3) and raises its event 5. Three ammunition kinds of up to 30 each, refilled by touching a class-108
//! pickup (kind 1435, amount 1436, default 30; within 150 units [I]), which is then collected (leaves the world). A
//! weapon fires when its kind has at least its cost (1 [I]): driven, R1 fires the first and L1 the second [I].
//! Driven, the stick's camera-relative heading steers (turning at 3 rad/s [I]) and holding Cross drives at the
//! maximum speed (500). Not driven, the computer drives against the driven car: approach to 600 units [I], attack
//! (stop, turn, fire when the aim error is under 0.3 rad; next attack after a random 1 to 1.5 s), fetch ammunition
//! when out (pickups scored by distance, the one already chosen by −200 + 0.8·d), retreat at 0.85 × the maximum
//! speed below a quarter of its health [I].

use super::{Pad, Presses, button, camera_heading, flat_distance, ground_follow, orientation, pad_for, place, wrap};
use crate::behavior::{ClassBehavior, Cx};
use crate::math::{Vec3, cos, sin, sub};
use crate::object::Handle;
use crate::physics::spring::{SmoothParams, Smoother};
use crate::world::{Notice, World};

/// Steering smoothing (working assumption; the record at `0x003292E8` is not in the spec).
pub const STEER: SmoothParams = SmoothParams { fast: 4.0, slow: 0.0, fast_time: 0.25, max_speed_change: None };
/// Drag per second on the drive speed [I].
pub const DRAG: f32 = 1.5;

/// The truck or a remote car.
#[derive(Clone, Debug, PartialEq)]
pub struct Car {
    pub forward_force: f32,
    pub reverse_force: f32,
    pub steer_limit: f32,
    pub gravity: f32,
    pub wheelbase: f32,
    pub horn: bool,
    /// 0 stopped, 1 driven.
    pub mode: u8,
    pub steer: f32,
    steer_smooth: Smoother,
    pub speed: f32,
    pub vz: f32,
    pub heading: f32,
    presses: Presses,
    /// Horn presses (sound 42 played).
    pub honks: u32,
    honk: bool,
}

impl Car {
    pub fn new(world: &World, me: Handle, class: i16) -> Car {
        let truck = class == 81;
        let o = world.options(me);
        let f = |id: u16, d: f32| o.map_or(d, |o| o.f32_or(id, d));
        let (fwd, steer) = if truck { (f(1625, 1100.0), f(1624, 0.8)) } else { (f(1815, 1100.0), f(1814, 0.8)) };
        let reverse = if truck { 700.0 } else { f(1816, 700.0) };
        let gravity = o.and_then(|o| o.vec3(35)).map_or(-1750.0, |g| g[2]);
        let wheels: alloc::vec::Vec<Vec3> = (if truck { 1618..=1621 } else { 1810..=1813 })
            .filter_map(|id| o.and_then(|o| o.i32(id)))
            .filter_map(|id| world.find_sly3_id(id, Some(me)))
            .filter_map(|h| world.position(h))
            .collect();
        let w = world.world_transform(me);
        let wheelbase = match (wheels.len(), w) {
            (4, Some(w)) => {
                let fwd_axis = w.cols[0];
                let along: alloc::vec::Vec<f32> =
                    wheels.iter().map(|p| crate::math::dot(sub(*p, w.t), fwd_axis)).collect();
                let max = along.iter().copied().fold(f32::MIN, f32::max);
                let min = along.iter().copied().fold(f32::MAX, f32::min);
                if max - min > 10.0 { max - min } else { 300.0 }
            }
            _ => 300.0,
        };
        Car {
            forward_force: fwd,
            reverse_force: reverse,
            steer_limit: steer,
            gravity,
            wheelbase,
            horn: !truck,
            mode: 0,
            steer: 0.0,
            steer_smooth: Smoother::default(),
            speed: 0.0,
            vz: 0.0,
            heading: w.map_or(0.0, |w| super::heading_of(&w.cols).0),
            presses: Presses::default(),
            honks: 0,
            honk: false,
        }
    }

    /// Steering and drive force for this frame (7.1, mode 1 or 0).
    pub fn control(&mut self, pad: &Pad, driven: bool, dt: f32) -> f32 {
        self.mode = u8::from(driven);
        self.presses.update(pad, dt);
        if !driven {
            self.steer = self.steer_smooth.step(&STEER, self.steer, 0.0, dt);
            return 0.0;
        }
        let want = -pad.stick[0] * self.steer_limit;
        self.steer = self.steer_smooth.step(&STEER, self.steer, want, dt);
        if self.horn && self.presses.take(button::L3, 0.15) {
            self.honks += 1;
            self.honk = true;
        }
        if pad.held(button::CROSS) {
            self.forward_force
        } else if pad.held(button::CIRCLE | button::SQUARE) {
            -self.reverse_force
        } else {
            0.0
        }
    }

    /// Integrates the drive speed and heading for one step.
    pub fn drive(&mut self, force: f32, dt: f32) {
        self.speed += (force - DRAG * self.speed) * dt;
        let tan = sin(self.steer) / cos(self.steer).max(1e-3);
        self.heading = wrap(self.heading + self.speed * tan / self.wheelbase * dt);
    }
}

impl ClassBehavior for Car {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let driven = super::driven(cx.world, cx.me);
        if !driven && self.speed.abs() < 1.0 && self.mode == 0 {
            self.speed = 0.0;
            return;
        }
        let pad = pad_for(cx.world, cx.me);
        let force = self.control(&pad, driven, cx.dt);
        if core::mem::take(&mut self.honk) {
            cx.world.play_sound(42, Some(cx.me));
        }
        self.drive(force, cx.dt);
        let Some(p) = cx.world.position(cx.me) else { return };
        let fwd = [cos(self.heading), sin(self.heading), 0.0];
        let mut np = crate::math::madd(p, fwd, self.speed * cx.dt);
        let (z, vz, _) = ground_follow(cx, np, self.vz, self.gravity, cx.dt);
        np[2] = z;
        self.vz = vz;
        place(cx.world, cx.me, np, orientation(self.heading, 0.0, 0.0));
    }
}

/// Combat car ammunition: three kinds of up to 30.
pub const AMMO_MAX: i32 = 30;
/// The combat car's maximum speed (default).
pub const COMBAT_SPEED: f32 = 500.0;

/// The combat car (Sly 2 class 77).
#[derive(Clone, Debug, PartialEq)]
pub struct CombatCar {
    pub health: f32,
    pub max_health: f32,
    /// 0 to 2 computer states (approach, attack, fetch), 3 defeated; 4 retreat.
    pub mode: u8,
    pub ammo: [i32; 3],
    pub heading: f32,
    pub speed: f32,
    pub shots: u32,
    attack_wait: f32,
    chosen_pickup: Option<Handle>,
}

impl CombatCar {
    pub fn new(world: &World, me: Handle) -> CombatCar {
        let max = world.options(me).map_or(100.0, |o| o.f32_or(1426, 100.0));
        CombatCar {
            health: max,
            max_health: max,
            mode: 0,
            ammo: [AMMO_MAX; 3],
            heading: world.world_transform(me).map_or(0.0, |w| super::heading_of(&w.cols).0),
            speed: 0.0,
            shots: 0,
            attack_wait: 0.0,
            chosen_pickup: None,
        }
    }

    fn fire(&mut self, weapon: usize) -> bool {
        if self.ammo[weapon] >= 1 {
            self.ammo[weapon] -= 1;
            self.shots += 1;
            true
        } else {
            false
        }
    }

    /// Takes damage; returns whether it was defeated by it.
    pub fn damage(&mut self, d: f32) -> bool {
        if self.mode == 3 {
            return false;
        }
        self.health = (self.health - d).max(0.0);
        if self.health <= 0.0 {
            self.mode = 3;
            self.speed = 0.0;
            return true;
        }
        false
    }

    /// The ammunition pickup to fetch: scored by distance, the chosen one by −200 + 0.8·d [D 0x002C20C0].
    fn pick(&self, cx: &Cx<'_>, p: Vec3) -> Option<Handle> {
        cx.world
            .handles()
            .filter(|&h| cx.world.class(h) == Some(108) && cx.world.in_world(h))
            .filter_map(|h| cx.world.position(h).map(|q| (h, flat_distance(p, q))))
            .map(|(h, d)| (h, if Some(h) == self.chosen_pickup { -200.0 + 0.8 * d } else { d }))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|x| x.0)
    }

    fn turn_toward(&mut self, h: f32, rate: f32, dt: f32) -> f32 {
        let e = wrap(h - self.heading);
        let step = rate * dt;
        self.heading = wrap(self.heading + e.clamp(-step, step));
        e.abs()
    }
}

impl ClassBehavior for CombatCar {
    fn ticks(&self) -> bool {
        self.mode != 3
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if self.mode == 3 {
            return;
        }
        let dt = cx.dt;
        for n in cx.world.notices(cx.me).to_vec() {
            let d = match n {
                Notice::Hit(_) | Notice::Attack { .. } => 10.0,
                Notice::Blast(..) => 20.0,
                _ => 0.0,
            };
            if d > 0.0 && self.damage(d) {
                cx.raise(5, alloc::vec::Vec::new());
                return;
            }
        }
        let Some(p) = cx.world.position(cx.me) else { return };
        // Ammunition pickups within reach.
        let near: alloc::vec::Vec<Handle> = cx
            .world
            .handles()
            .filter(|&h| cx.world.class(h) == Some(108) && cx.world.in_world(h))
            .filter(|&h| cx.world.position(h).is_some_and(|q| flat_distance(p, q) <= 150.0))
            .collect();
        for h in near {
            let o = cx.world.options(h);
            let kind = o.and_then(|o| o.i32(1435)).unwrap_or(0).clamp(0, 2) as usize;
            let amount = o.and_then(|o| o.i32(1436)).unwrap_or(AMMO_MAX);
            self.ammo[kind] = (self.ammo[kind] + amount).min(AMMO_MAX);
            cx.world.set_in_world(h, false);
        }
        let driven = super::driven(cx.world, cx.me);
        let mut want_speed = 0.0;
        if driven {
            let pad = pad_for(cx.world, cx.me);
            if let Some(h) = camera_heading(cx.world, pad.stick) {
                self.turn_toward(h, 3.0, dt);
            }
            if pad.held(button::CROSS) {
                want_speed = COMBAT_SPEED;
            }
            if pad.pressed & button::R1 != 0 {
                self.fire(0);
            }
            if pad.pressed & button::L1 != 0 {
                self.fire(1);
            }
        } else if let Some(t) = cx.world.shared.driving.filter(|&d| d != cx.me && cx.world.class(d) == Some(77))
            && let Some(tp) = cx.world.position(t)
        {
            let d = flat_distance(p, tp);
            let to = crate::math::atan2(tp[1] - p[1], tp[0] - p[0]);
            self.attack_wait -= dt;
            let out_of_ammo = self.ammo.iter().all(|&a| a == 0);
            self.mode = if self.health < 0.25 * self.max_health {
                4
            } else if out_of_ammo {
                2
            } else if d > 600.0 {
                0
            } else {
                1
            };
            match self.mode {
                0 => {
                    self.turn_toward(to, 3.0, dt);
                    want_speed = COMBAT_SPEED;
                }
                1 => {
                    let err = self.turn_toward(to, 3.0, dt);
                    if err < 0.3 && self.attack_wait <= 0.0 {
                        let w = (0..3).find(|&k| self.ammo[k] > 0).unwrap_or(0);
                        self.fire(w);
                        self.attack_wait = 1.0 + 0.5 * cx.rng.next_f32();
                    }
                }
                2 => {
                    self.chosen_pickup = self.pick(cx, p);
                    if let Some(q) = self.chosen_pickup.and_then(|h| cx.world.position(h)) {
                        self.turn_toward(crate::math::atan2(q[1] - p[1], q[0] - p[0]), 3.0, dt);
                        want_speed = COMBAT_SPEED;
                    }
                }
                _ => {
                    self.turn_toward(wrap(to + core::f32::consts::PI), 3.0, dt);
                    want_speed = 0.85 * COMBAT_SPEED;
                }
            }
        }
        self.speed += (want_speed - self.speed) * (5.0 * dt).min(1.0);
        if self.speed.abs() < 0.5 {
            self.speed = 0.0;
            return;
        }
        let fwd = [cos(self.heading), sin(self.heading), 0.0];
        let mut np = crate::math::madd(p, fwd, self.speed * dt);
        let (z, _, _) = ground_follow(cx, np, 0.0, -1750.0, dt);
        np[2] = z;
        place(cx.world, cx.me, np, orientation(self.heading, 0.0, 0.0));
    }
}
