//! Driven machines and minigames of Sly 2 and Sly 3 (specs/sly2/vehicles-and-minigames.md,
//! specs/sly3/vehicles-and-minigames.md).
//!
//! | Module | Sly 2 classes | Sly 3 classes |
//! |---|---|---|
//! | [`turret`] | 69 turrets | 68 turret |
//! | [`hack`] | 72 hack ship, 73 enemies, 75 dock | 74 hack ship, 75 enemies, 77 dock |
//! | [`car`] | 77 combat car (with 108 ammo) | 81 truck, 85 remote cars |
//! | [`air`] | | 82 biplanes (player and computer flight, shots) |
//! | [`craft`] | | 83 raft, 84 jolly boat, 87 swimmer, 88 ships |
//! | [`dance`] | 121 dance director (the scoring of 5) | |
//! | [`boss`] | 59 to 67 bosses' hit points and block | |
//!
//! **Who drives.** A machine reads the controller only while `World::shared.driving` names it (the session sets it;
//! the gang players then get an idle pad). Without a driver a machine runs its computer control where the spec has
//! one (the combat car), else stays put. Machines move by setting their world transform, following the ground
//! through the physics bridge's collision queries ([`ground_follow`]).
//!
//! Not covered: Sly 2's class-50 machines (the RC chopper, the tank) are flown by their scripts (2); the Sly 3 lock
//! minigame (79) has no rules in the spec; the Sly 3 safe (80) is `behavior::sly3::devices::Safe`.

pub mod air;
pub mod boss;
pub mod car;
pub mod craft;
pub mod dance;
pub mod hack;
pub mod shots;
pub mod turret;

use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::behavior::{Behavior, ClassBehavior, Cx};
use crate::game::systems::FrameInput;
use crate::math::{Mat3, Vec3, atan2, cos, length, sin, sqrt};
use crate::object::Handle;
use crate::world::World;

/// Controller buttons in the games' 16-bit mask (specs/sly3/vehicles-and-minigames.md 7).
pub mod button {
    pub const L2: u32 = 0x1;
    pub const R2: u32 = 0x2;
    pub const L1: u32 = 0x4;
    pub const R1: u32 = 0x8;
    pub const TRIANGLE: u32 = 0x10;
    pub const CIRCLE: u32 = 0x20;
    pub const CROSS: u32 = 0x40;
    pub const SQUARE: u32 = 0x80;
    pub const L3: u32 = 0x200;
    pub const R3: u32 = 0x400;
    /// The eight face and shoulder buttons (Sly 2 turret fire).
    pub const FACE_AND_SHOULDER: u32 = L2 | R2 | L1 | R1 | TRIANGLE | CIRCLE | CROSS | SQUARE;
}

/// The controller as the machines read it: sticks, held and pressed buttons in [`button`] bits (mapped from the
/// platform buttons as the gang maps them: B Cross, A Circle, Y Square, X Triangle, ZR R1, L L1, ZL L2, R R2).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pad {
    pub stick: [f32; 2],
    pub right: [f32; 2],
    pub held: u32,
    pub pressed: u32,
}

/// Platform button bit to game mask.
const MAP: [(u32, u32); 10] = [
    (1 << 1, button::CROSS),
    (1 << 0, button::CIRCLE),
    (1 << 3, button::SQUARE),
    (1 << 2, button::TRIANGLE),
    (1 << 9, button::R1),
    (1 << 6, button::L1),
    (1 << 8, button::L2),
    (1 << 7, button::R2),
    (1 << 4, button::L3),
    (1 << 5, button::R3),
];

fn map_buttons(b: u32) -> u32 {
    MAP.iter().filter(|(p, _)| b & p != 0).fold(0, |acc, (_, g)| acc | g)
}

impl Pad {
    /// The frame's controller.
    pub fn from_frame(i: &FrameInput) -> Pad {
        Pad { stick: i.left, right: i.right, held: map_buttons(i.buttons), pressed: map_buttons(i.pressed) }
    }

    /// Left-stick magnitude (0 to 1).
    pub fn magnitude(&self) -> f32 {
        sqrt(self.stick[0] * self.stick[0] + self.stick[1] * self.stick[1]).min(1.0)
    }

    pub fn held(&self, mask: u32) -> bool {
        self.held & mask != 0
    }
}

/// "Pressed within the last t seconds" (the games' shared press routine, 7.0.1): the time since each button was
/// last pressed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Presses {
    age: [f32; 11],
}

impl Default for Presses {
    fn default() -> Self {
        Presses { age: [f32::INFINITY; 11] }
    }
}

impl Presses {
    /// Ages the record and notes this frame's presses.
    pub fn update(&mut self, pad: &Pad, dt: f32) {
        for (i, a) in self.age.iter_mut().enumerate() {
            *a = if pad.pressed & (1 << i) != 0 { 0.0 } else { *a + dt };
        }
    }

    /// Whether a button of `mask` was pressed within `t`; consumes the press when it was.
    pub fn take(&mut self, mask: u32, t: f32) -> bool {
        let mut hit = false;
        for (i, a) in self.age.iter_mut().enumerate() {
            if mask & (1 << i) != 0 && *a <= t {
                *a = f32::INFINITY;
                hit = true;
            }
        }
        hit
    }
}

/// Whether `me` is the machine the player drives.
pub fn driven(world: &World, me: Handle) -> bool {
    world.shared.driving == Some(me)
}

/// The controller for `me`: the frame's when it is driven, else idle.
pub fn pad_for(world: &World, me: Handle) -> Pad {
    if driven(world, me) { Pad::from_frame(&world.shared.input) } else { Pad::default() }
}

/// The stick turned into a world heading relative to the camera (as the gang reads it): `None` when centred.
pub fn camera_heading(world: &World, stick: [f32; 2]) -> Option<f32> {
    if stick[0] * stick[0] + stick[1] * stick[1] < 1e-4 {
        return None;
    }
    // Stick up is away from the camera: the camera's yaw, turned by the stick's angle from up.
    Some(wrap(world.shared.camera_yaw + atan2(-stick[0], stick[1])))
}

/// An angle wrapped to −π..π.
pub fn wrap(a: f32) -> f32 {
    use core::f32::consts::{PI, TAU};
    crate::math::rem_euclid(a + PI, TAU) - PI
}

/// The rotation with forward (+X) at `yaw` about +Z and `pitch` up, rolled by `roll` about its forward axis
/// (columns: the images of the local axes).
pub fn orientation(yaw: f32, pitch: f32, roll: f32) -> Mat3 {
    let (sy, cy) = (sin(yaw), cos(yaw));
    let (sp, cp) = (sin(pitch), cos(pitch));
    let (sr, cr) = (sin(roll), cos(roll));
    let x = [cy * cp, sy * cp, sp];
    let y0 = [-sy, cy, 0.0];
    let z0 = [-cy * sp, -sy * sp, cp];
    let y = [cr * y0[0] + sr * z0[0], cr * y0[1] + sr * z0[1], cr * y0[2] + sr * z0[2]];
    let z = [-sr * y0[0] + cr * z0[0], -sr * y0[1] + cr * z0[1], -sr * y0[2] + cr * z0[2]];
    [x, y, z]
}

/// Yaw and pitch of a rotation's forward axis.
pub fn heading_of(m: &Mat3) -> (f32, f32) {
    let x = m[0];
    (atan2(x[1], x[0]), atan2(x[2], sqrt(x[0] * x[0] + x[1] * x[1])))
}

/// Sets `me`'s world position and rotation.
pub fn place(world: &mut World, me: Handle, p: Vec3, r: Mat3) {
    let mut w = world.world_transform(me).unwrap_or_default();
    w.cols = r;
    w.t = p;
    world.set_world_transform(me, w);
}

/// The ground under `p` (searched from `rise` above it, `drop` below), skipping the machine's own body and its
/// children: (height, normal).
pub fn ground_below(cx: &Cx<'_>, p: Vec3, rise: f32, drop: f32) -> Option<(f32, Vec3)> {
    let mut from = [p[0], p[1], p[2] + rise];
    let mut left = rise + drop;
    for _ in 0..4 {
        let (hit, n, obj) = cx.collision.ground(cx.world, from, left)?;
        let own = obj.is_some_and(|o| o == cx.me || is_below(cx.world, o, cx.me));
        if !own {
            return Some((hit[2], n));
        }
        // Start again just under our own surface.
        left -= from[2] - hit[2] + 1.0;
        from = [hit[0], hit[1], hit[2] - 1.0];
        if left <= 0.0 {
            return None;
        }
    }
    None
}

fn is_below(world: &World, h: Handle, root: Handle) -> bool {
    let mut p = world.parent(h);
    while let Some(q) = p {
        if q == root {
            return true;
        }
        p = world.parent(q);
    }
    false
}

/// Vertical motion under `gravity` (negative, units/s²) with the ground as a floor: the new height and vertical
/// speed. Without collision (or no ground within reach) the machine keeps its height.
pub fn ground_follow(cx: &Cx<'_>, p: Vec3, vz: f32, gravity: f32, dt: f32) -> (f32, f32, bool) {
    match ground_below(cx, p, 200.0, 4000.0) {
        Some((g, _)) => {
            let vz = vz + gravity * dt;
            let z = p[2] + vz * dt;
            if z <= g { (g, 0.0, true) } else { (z, vz, false) }
        }
        None => (p[2], 0.0, false),
    }
}

/// The horizontal distance between two points.
pub fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    length([a[0] - b[0], a[1] - b[1], 0.0])
}

/// Which game a machine runs for (the option numbers differ).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Title {
    Sly2,
    Sly3,
}

/// The behaviour for one object of a Sly 2 or Sly 3 world, if it is one of the machines here.
pub fn for_class(world: &World, h: Handle) -> Option<Box<dyn ClassBehavior>> {
    let class = world.class(h)?;
    if world.sly2.is_some() {
        return match class {
            69 => Some(Box::new(turret::Turret::new(world, h, Title::Sly2))),
            72 => Some(Box::new(hack::HackShip::new(world, h, Title::Sly2))),
            73 => Some(Box::new(hack::HackEnemy::new(world, h, Title::Sly2))),
            77 => Some(Box::new(car::CombatCar::new(world, h))),
            121 => Some(Box::new(dance::Director::new(world, h))),
            59..=67 => Some(Box::new(boss::Boss::new(world, h))),
            _ => None,
        };
    }
    if world.sly3.is_some() {
        return match class {
            68 => Some(Box::new(turret::Turret::new(world, h, Title::Sly3))),
            74 => Some(Box::new(hack::HackShip::new(world, h, Title::Sly3))),
            75 => Some(Box::new(hack::HackEnemy::new(world, h, Title::Sly3))),
            81 | 85 => Some(Box::new(car::Car::new(world, h, class))),
            82 => Some(Box::new(air::Biplane::new(world, h))),
            83 | 84 => Some(Box::new(craft::Raft::new(world, h, class == 84))),
            87 => Some(Box::new(craft::Swimmer::new(world, h))),
            88 => Some(Box::new(craft::Ship::new(world, h))),
            _ => None,
        };
    }
    None
}

/// Attaches the machines' behaviours to a Sly 2 or Sly 3 world's objects that have none yet.
pub fn attach(world: &mut World) {
    if world.sly2.is_none() && world.sly3.is_none() {
        return;
    }
    let handles: Vec<Handle> = world.handles().collect();
    for h in handles {
        if !matches!(world.behavior(h), None | Some(Behavior::None)) {
            continue;
        }
        if let Some(b) = for_class(world, h) {
            world.set_behavior(h, Behavior::Custom(b));
        }
    }
}

/// Queues a Sly 2 task event (missions.md), for the job system.
pub fn task_event(world: &mut World, event: u32) {
    if let Some(l) = world.sly2.as_deref_mut() {
        l.task_events.push((event, None));
    }
}

#[cfg(test)]
mod tests;
