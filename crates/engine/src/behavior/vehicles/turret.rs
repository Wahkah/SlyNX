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
        if !super::driven(cx.world, cx.me) && self.pan_speed == 0.0 && self.tilt_speed == 0.0 {
            return;
        }
        let pad = pad_for(cx.world, cx.me);
        if self.step(&pad, cx.dt) {
            cx.raise(5, alloc::vec::Vec::new());
        }
        if let Some(mut w) = cx.world.world_transform(cx.me) {
            w.cols = self.rotation();
            cx.world.set_world_transform(cx.me, w);
        }
    }
}
