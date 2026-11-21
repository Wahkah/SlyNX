//! Bentley's hacking game: the ship (Sly 2 class 72, Sly 3 74), enemies (73, 75) and the exit dock (75, 77)
//! (specs/sly2/vehicles-and-minigames.md 4; Sly 3 3 with its option numbers).
//!
//! - **Ship.** States −1 out of the world, 0 idle, 1 driven, 2 hit, 3 docked, 4 destroyed. Driven, the target
//!   velocity is the stick's direction (camera-relative) × deflection × 1000; the ship accelerates toward it with
//!   gain 5 per second and turns toward the stick's heading with the smoothing (25, 0, 0.25). No gravity. The guns
//!   fire while the right stick is deflected (one shot every 0.15 s here [I]; the shots' objects are not spawned).
//!   A hit (while not hit, docked or destroyed) costs hit points; one that leaves the ship alive queues task event
//!   15 and holds state 2 for 0.5 s (the hit animation's length, not read [I]); at 0 the ship is destroyed and
//!   queues task event 16, which the job's macro answers by resetting the hack and restarting its sequence (10.2).
//!   Within 50 units of a dock's point (its option 1394 / 1593, else the dock itself) it docks (state 3) and queues
//!   task event 57 (no script handles it: the hack ends through the dock's own sequence; 10.2). Docking is armed
//!   once the ship has been clear of every dock point [I]. The disc's ship sets 1363 to 100.
//! - **Hit points**: maximum option 1363 (Sly 3: 937), default 10; a hit lowers them by its damage (1 for any hit
//!   here [I]). At 0 an enemy leaves the world (its burst object, 1366 / 1553, is not set off yet).
//! - **Knock-back** on a non-fatal hit: away from the hitter horizontally at `a + d·(b + d·c)` with (a, b, c) option
//!   1369 / 1555 (default (300, 50, 0)), clamped to 1370 / 1556 (default (0, 800)).
//! - **Enemies** chase the driven ship within 3,000 units [I] at their speed scale (1382 / 1576, default 250); one
//!   that carries points (1365) dies when it touches the ship (within 100 units [I]), hitting it. A killed enemy
//!   queues task event 24.

use super::{Title, camera_heading, flat_distance, pad_for, place, task_event};
use crate::behavior::{ClassBehavior, Cx};
use crate::math::{Vec3, length, scale, sub};
use crate::object::Handle;
use crate::physics::spring::{SmoothParams, Smoother};
use crate::world::{Notice, World};

/// Ship turn smoothing (25, 0, 0.25) [D, specs/sly2/vehicles-and-minigames.md 4.2].
pub const TURN: SmoothParams = SmoothParams { fast: 25.0, slow: 0.0, fast_time: 0.25, max_speed_change: None };
/// Ship acceleration gain per second [D, 4.2].
pub const ACCEL_GAIN: f32 = 5.0;
/// Ship speed at full deflection [D, 4.2].
pub const SPEED: f32 = 1000.0;
/// Docking distance [D, 4.4].
pub const DOCK_RANGE: f32 = 50.0;

/// Option numbers per game.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ids {
    hit_points: u16,
    knock: u16,
    knock_clamp: u16,
    points: u16,
    speed: u16,
    dock_point: u16,
    dock_class: i16,
    ship_class: i16,
}

fn ids(t: Title) -> Ids {
    match t {
        Title::Sly2 => Ids {
            hit_points: 1363,
            knock: 1369,
            knock_clamp: 1370,
            points: 1365,
            speed: 1382,
            dock_point: 1394,
            dock_class: 75,
            ship_class: 72,
        },
        Title::Sly3 => Ids {
            hit_points: 937,
            knock: 1555,
            knock_clamp: 1556,
            points: 0,
            speed: 1576,
            dock_point: 1593,
            dock_class: 77,
            ship_class: 74,
        },
    }
}

/// The knock-back speed for damage `d` (4.1).
pub fn knock_speed(abc: Vec3, clamp: [f32; 2], d: f32) -> f32 {
    (abc[0] + d * (abc[1] + d * abc[2])).clamp(clamp[0], clamp[1])
}

/// The hacking ship.
#[derive(Clone, Debug, PartialEq)]
pub struct HackShip {
    ids: Ids,
    pub state: i32,
    pub hit_points: f32,
    pub max_hit_points: f32,
    pub velocity: Vec3,
    pub heading: f32,
    turn: Smoother,
    hit_time: f32,
    gun_cool: f32,
    pub shots: u32,
    dock_armed: bool,
}

impl HackShip {
    pub fn new(world: &World, me: Handle, title: Title) -> HackShip {
        let ids = ids(title);
        let max = world.options(me).map_or(10.0, |o| o.f32_or(ids.hit_points, 10.0));
        let heading = world.world_transform(me).map_or(0.0, |w| super::heading_of(&w.cols).0);
        HackShip {
            ids,
            state: 0,
            hit_points: max,
            max_hit_points: max,
            velocity: [0.0; 3],
            heading,
            turn: Smoother::default(),
            hit_time: 0.0,
            gun_cool: 0.0,
            shots: 0,
            dock_armed: false,
        }
    }

    /// Takes a hit of `damage`; returns whether it counted.
    pub fn hit(&mut self, world: &mut World, damage: f32) -> bool {
        if matches!(self.state, 2..=4) {
            return false;
        }
        self.hit_points = (self.hit_points - damage).max(0.0);
        if self.hit_points > 0.0 {
            task_event(world, 15);
        }
        if self.hit_points <= 0.0 {
            self.state = 4;
            // Lost: task event 16, as a defeated gang member; the job's macro resets the hack and restarts its
            // sequence (10.2).
            task_event(world, 16);
        } else {
            self.state = 2;
            self.hit_time = 0.0;
        }
        true
    }

    fn dock_point(&self, cx: &Cx<'_>) -> Option<Vec3> {
        let docks: alloc::vec::Vec<Handle> =
            cx.world.handles().filter(|&h| cx.world.class(h) == Some(self.ids.dock_class)).collect();
        let me = cx.world.position(cx.me)?;
        docks
            .into_iter()
            .filter_map(|d| {
                let point = cx.world.options(d).and_then(|o| o.i32(self.ids.dock_point)).and_then(|id| {
                    if cx.world.sly2.is_some() {
                        cx.world.find_sly2_id(id, Some(d))
                    } else {
                        cx.world.find_sly3_id(id, Some(d))
                    }
                });
                cx.world.position(point.unwrap_or(d))
            })
            .min_by(|a, b| flat_distance(*a, me).total_cmp(&flat_distance(*b, me)))
    }
}

impl ClassBehavior for HackShip {
    fn ticks(&self) -> bool {
        true
    }

    /// Option 1364 (1363's current value in Sly 3's 937 terms): the hit points left.
    fn get_option(&self, _world: &World, _me: Handle, id: u16) -> Option<crate::object::OptionValue> {
        (id == 1364).then_some(crate::object::OptionValue::Float(self.hit_points))
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let dt = cx.dt;
        let driven = super::driven(cx.world, cx.me);
        for n in cx.world.notices(cx.me).to_vec() {
            if matches!(n, Notice::Hit(_) | Notice::Attack { .. }) {
                self.hit(cx.world, 1.0);
            }
        }
        match self.state {
            2 => {
                self.hit_time += dt;
                if self.hit_time >= 0.5 {
                    self.state = if driven { 1 } else { 0 };
                }
            }
            3 | 4 => {
                self.velocity = [0.0; 3];
                return;
            }
            _ => self.state = if driven { 1 } else { 0 },
        }
        let pad = pad_for(cx.world, cx.me);
        let target = match camera_heading(cx.world, pad.stick) {
            Some(h) => {
                self.heading = self.turn.step(&TURN, self.heading, unwrap_near(h, self.heading), dt);
                let m = pad.magnitude() * SPEED;
                [crate::math::cos(h) * m, crate::math::sin(h) * m, 0.0]
            }
            None => [0.0; 3],
        };
        let k = (ACCEL_GAIN * dt).min(1.0);
        self.velocity = crate::math::madd(self.velocity, sub(target, self.velocity), k);
        self.gun_cool -= dt;
        if pad.right[0] * pad.right[0] + pad.right[1] * pad.right[1] > 0.04 && self.gun_cool <= 0.0 {
            self.shots += 1;
            self.gun_cool = 0.15;
        }
        let Some(p) = cx.world.position(cx.me) else { return };
        let p = crate::math::madd(p, self.velocity, dt);
        place(cx.world, cx.me, p, super::orientation(self.heading, 0.0, 0.0));
        // Docking is armed once the ship has been clear of every dock point (it may start beside one) [I].
        let dock = self.dock_point(cx);
        if dock.is_none_or(|d| flat_distance(p, d) > 2.0 * DOCK_RANGE) {
            self.dock_armed = true;
        }
        if self.state == 1
            && self.dock_armed
            && let Some(d) = dock
            && flat_distance(p, d) <= DOCK_RANGE
        {
            self.state = 3;
            task_event(cx.world, 57);
            cx.raise(5, alloc::vec::Vec::new());
        }
    }
}

/// `a` moved by whole turns to within π of `near`.
fn unwrap_near(a: f32, near: f32) -> f32 {
    near + super::wrap(a - near)
}

/// A hacking enemy, turret or generator.
#[derive(Clone, Debug, PartialEq)]
pub struct HackEnemy {
    ids: Ids,
    pub hit_points: f32,
    pub points: i32,
    speed: f32,
    knock: Vec3,
    knock_clamp: [f32; 2],
    pub velocity: Vec3,
    pub dead: bool,
}

impl HackEnemy {
    pub fn new(world: &World, me: Handle, title: Title) -> HackEnemy {
        let ids = ids(title);
        let o = world.options(me);
        HackEnemy {
            ids,
            hit_points: o.map_or(10.0, |o| o.f32_or(ids.hit_points, 10.0)),
            points: if ids.points == 0 { 0 } else { o.and_then(|o| o.i32(ids.points)).unwrap_or(0) },
            speed: o.map_or(250.0, |o| o.f32_or(ids.speed, 250.0)),
            knock: o.and_then(|o| o.vec3(ids.knock)).unwrap_or([300.0, 50.0, 0.0]),
            knock_clamp: o.and_then(|o| o.pair(ids.knock_clamp)).unwrap_or([0.0, 800.0]),
            velocity: [0.0; 3],
            dead: false,
        }
    }

    fn die(&mut self, cx: &mut Cx<'_>) {
        self.dead = true;
        // An enemy killed: task event 24 (10.2).
        task_event(cx.world, 24);
        cx.world.set_in_world(cx.me, false);
    }
}

impl ClassBehavior for HackEnemy {
    fn ticks(&self) -> bool {
        !self.dead
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if self.dead {
            return;
        }
        let Some(p) = cx.world.position(cx.me) else { return };
        for n in cx.world.notices(cx.me).to_vec() {
            if let Notice::Hit(src) | Notice::Attack { by: src, .. } = n {
                self.hit_points -= 1.0;
                if self.hit_points <= 0.0 {
                    self.die(cx);
                    return;
                }
                if let Some(s) = cx.world.position(src) {
                    let away = sub(p, s);
                    let d = length([away[0], away[1], 0.0]).max(1e-3);
                    let v = knock_speed(self.knock, self.knock_clamp, 1.0);
                    self.velocity = [away[0] / d * v, away[1] / d * v, 0.0];
                }
            }
        }
        // The driven ship, if any, of this game.
        let ship = cx.world.shared.driving.filter(|&s| cx.world.class(s) == Some(self.ids.ship_class));
        let target = ship.and_then(|s| cx.world.position(s));
        let mut want = [0.0; 3];
        if let Some(t) = target {
            let d = flat_distance(p, t);
            if self.points > 0 && d <= 100.0 {
                if let Some(s) = ship {
                    cx.world.notify(s, Notice::Hit(cx.me));
                }
                self.die(cx);
                return;
            }
            if d <= 3000.0 && d > 1.0 {
                let to = sub(t, p);
                want = scale([to[0], to[1], 0.0], self.speed / d);
            }
        }
        let k = (5.0 * cx.dt).min(1.0);
        self.velocity = crate::math::madd(self.velocity, sub(want, self.velocity), k);
        if length(self.velocity) > 1e-3 {
            let np = crate::math::madd(p, self.velocity, cx.dt);
            let rot = cx.world.world_transform(cx.me).map_or(crate::math::MAT3_IDENTITY, |w| w.cols);
            place(cx.world, cx.me, np, rot);
        }
    }
}
