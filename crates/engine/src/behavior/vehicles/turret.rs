//! Turrets: Sly 2 class 69 (specs/sly2/vehicles-and-minigames.md 3), Sly 3 class 68 (Sly 3 2, the same rules with
//! options 1094, 1095, 1103, 1104).
//!
//! Driven, the left stick aims: each axis is cubed and scaled by `g = (1 − f)·0.5 + f` (f the field of view, rad);
//! the angular acceleration is `10·g` times that; the angular speed decays by the smoothing (5, 0, 0.5), is cut to 0
//! when the input opposes it, and is capped at `1.3·f·0.9` (pan) and `1.3·f·0.8` (tilt); pan and tilt are clamped
//! to the limits. The right stick's vertical axis zooms: `u` changes at `(2 − u²)·y·|y|` per second and the field
//! of view runs from the far value at u = 0 to the near value at 1 (to half of it when they are equal). Any of the
//! eight face and shoulder buttons pressed fires: script event 5 on the turret (taking a projectile from the pool
//! of option 1002 and throwing it from 1003 is left to the scripts that set them). Pan turns about the turret's
//! own +Z, tilt about its +Y [I]. The reverse-aim setting is not read (normal aim).

use super::{Pad, Title, button, pad_for};
use crate::behavior::{ClassBehavior, Cx};
use crate::math::{Mat3, mat_mul};
use crate::object::Handle;
use crate::physics::spring::{SmoothParams, Smoother};
use crate::world::World;

/// A turret shot's default speed: the missile family's, used when the turret has no launcher or its launcher has no
/// speed setting (10.4).
pub const SHOT_SPEED: f32 = 800.0;
/// The Sly 2 turret's launcher option (class 112's object; set by the turret jobs' scripts) and the launcher's speed
/// setting (script ID 509) (10.4).
pub const LAUNCHER_OPTION: u16 = 1003;
pub const LAUNCHER_SPEED: u16 = 509;

/// The object a turret option names (a Sly 2 or Sly 3 ID, resolved from the turret).
fn referenced(world: &World, me: Handle, option: u16) -> Option<Handle> {
    if option == 0 {
        return None;
    }
    let id = world.options(me)?.i32(option)?;
    if world.sly2.is_some() { world.find_sly2_id(id, Some(me)) } else { world.find_sly3_id(id, Some(me)) }
}

/// A turret's shot speed: its launcher's speed setting (Sly 2 option 1003's object, option 509), else
/// [`SHOT_SPEED`] (10.4).
pub fn shot_speed(world: &World, me: Handle) -> f32 {
    if world.sly2.is_none() {
        return SHOT_SPEED;
    }
    referenced(world, me, LAUNCHER_OPTION)
        .and_then(|l| world.options(l))
        .map_or(SHOT_SPEED, |o| o.f32_or(LAUNCHER_SPEED, SHOT_SPEED))
        .max(0.0)
}

/// The angular-speed decay record (5, 0, 0.5) [D 0x002C4408].
pub const DECAY: SmoothParams = SmoothParams { fast: 5.0, slow: 0.0, fast_time: 0.5, max_speed_change: None };

/// Turret state.
#[derive(Clone, Debug, PartialEq)]
pub struct Turret {
    /// The turret's rotation at load (pan and tilt are relative to it).
    base: Mat3,
    pub pan: f32,
    pub tilt: f32,
    pan_speed: f32,
    tilt_speed: f32,
    pan_decay: Smoother,
    tilt_decay: Smoother,
    /// Zoom fraction (0 far, 1 near).
    pub zoom: f32,
    /// Tilt and pan limits, field-of-view range (near, far).
    pub tilt_limits: [f32; 2],
    pub pan_limits: [f32; 2],
    pub fov_range: [f32; 2],
    /// Shots fired.
    pub shots: u32,
    /// The camera mount option (Sly 2 1008; Sly 3 has none: 0).
    mount_option: u16,
}

impl Turret {
    pub fn new(world: &World, me: Handle, title: Title) -> Turret {
        let (tilt, pan, fov) = match title {
            Title::Sly2 => (1000, 1001, 1010),
            Title::Sly3 => (1094, 1095, 1104),
        };
        let o = world.options(me);
        let pair = |id: u16, d: [f32; 2]| o.and_then(|o| o.pair(id)).unwrap_or(d);
        Turret {
            base: world.world_transform(me).map_or(crate::math::MAT3_IDENTITY, |w| w.cols),
            pan: 0.0,
            tilt: 0.0,
            pan_speed: 0.0,
            tilt_speed: 0.0,
            pan_decay: Smoother::default(),
            tilt_decay: Smoother::default(),
            zoom: 0.0,
            tilt_limits: pair(tilt, [-0.7, 0.7]),
            pan_limits: pair(pan, [-1.5, 1.5]),
            fov_range: pair(fov, [0.1, 1.0]),
            shots: 0,
            mount_option: if title == Title::Sly2 { 1008 } else { 0 },
        }
    }

    /// The field of view now (rad).
    pub fn fov(&self) -> f32 {
        let [near, far] = self.fov_range;
        let near = if (near - far).abs() < 1e-6 { far * 0.5 } else { near };
        far + (near - far) * self.zoom
    }

    /// One step of aim, zoom and fire; returns whether it fired.
    pub fn step(&mut self, pad: &Pad, dt: f32) -> bool {
        let f = self.fov();
        let g = (1.0 - f) * 0.5 + f;
        let axis = |speed: &mut f32, decay: &mut Smoother, input: f32, cap: f32| {
            let a = 10.0 * g * input * input * input;
            if a != 0.0 && *speed != 0.0 && a.signum() != speed.signum() {
                *speed = 0.0;
            }
            *speed = decay.step(&DECAY, *speed, 0.0, dt) + a * dt;
            *speed = speed.clamp(-cap, cap);
        };
        axis(&mut self.pan_speed, &mut self.pan_decay, -pad.stick[0], 1.3 * f * 0.9);
        axis(&mut self.tilt_speed, &mut self.tilt_decay, pad.stick[1], 1.3 * f * 0.8);
        self.pan = (self.pan + self.pan_speed * dt).clamp(self.pan_limits[0], self.pan_limits[1]);
        self.tilt = (self.tilt + self.tilt_speed * dt).clamp(self.tilt_limits[0], self.tilt_limits[1]);
        let y = pad.right[1];
        self.zoom = (self.zoom + (2.0 - self.zoom * self.zoom) * y * y.abs() * dt).clamp(0.0, 1.0);
        let fire = pad.pressed & button::FACE_AND_SHOULDER != 0;
        if fire {
            self.shots += 1;
        }
        fire
    }

    /// The turret's rotation for its pan and tilt.
    pub fn rotation(&self) -> Mat3 {
        let pan = crate::math::axis_angle([0.0, 0.0, self.pan]);
        let tilt = crate::math::axis_angle([0.0, -self.tilt, 0.0]);
        mat_mul(&self.base, &mat_mul(&pan, &tilt))
    }
}

impl ClassBehavior for Turret {
    fn ticks(&self) -> bool {
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let driven = super::driven(cx.world, cx.me);
        // The camera rides the mount (option 1008's object, else the turret) while the turret is driven; no eye
        // offset (none in the spec) [I].
        let mount = referenced(cx.world, cx.me, self.mount_option).unwrap_or(cx.me);
        if driven {
            cx.world.shared.camera_mount = Some((mount, [0.0; 3]));
        } else if cx.world.shared.camera_mount.is_some_and(|m| m.0 == mount) {
            cx.world.shared.camera_mount = None;
        }
        if !driven && self.pan_speed == 0.0 && self.tilt_speed == 0.0 {
            return;
        }
        let pad = pad_for(cx.world, cx.me);
        let fired = self.step(&pad, cx.dt);
        if let Some(mut w) = cx.world.world_transform(cx.me) {
            w.cols = self.rotation();
            cx.world.set_world_transform(cx.me, w);
            if fired {
                // A shot along the barrel (+X) at its launcher's speed (10.4); its blast and damage are
                // `shots::BLAST_RADIUS` and `shots::DAMAGE`.
                let fwd = w.cols[0];
                let speed = shot_speed(cx.world, cx.me);
                super::shots::fire(
                    cx.world,
                    super::shots::Projectile {
                        position: crate::math::madd(w.t, fwd, 100.0),
                        velocity: crate::math::scale(fwd, speed),
                        life: 4.0,
                        owner: cx.me,
                        damage: super::shots::DAMAGE,
                        gravity: 0.0,
                    },
                );
            }
        }
        if fired {
            cx.raise(5, alloc::vec::Vec::new());
        }
    }
}
