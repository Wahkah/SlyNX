//! The free camera: the camera system until the follow camera of camera.md exists (it is also that spec's
//! "manual" development mode). Left stick flies, right stick looks, ZR / ZL rise and sink, B speeds up, the d-pad
//! doubles or halves the speed, Y goes back to the last cut.

use platform_api::Buttons;

use super::render_scene::{CameraView, lens};
use super::systems::{CameraSystem, FrameInput};
use crate::math::{add, cross, normalize, scale, sin_cos, sub};
use crate::world::World;

/// Base flying speed, units per second.
pub const BASE_SPEED: f32 = 1200.0;
/// Look speed at full deflection, radians per second.
pub const LOOK_SPEED: f32 = 2.0;
/// Speed multiplier while B is held.
pub const BOOST: f32 = 5.0;

/// The free camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreeCamera {
    /// Eye position.
    pub pos: [f32; 3],
    /// Heading around +Z, radians (0 looks along +X).
    pub yaw: f32,
    /// Elevation, radians.
    pub pitch: f32,
    /// Flying speed.
    pub speed: f32,
    /// Lens.
    pub fov_y: f32,
    /// Near clip.
    pub near: f32,
    /// Far clip.
    pub far: f32,
    /// Where Y puts the camera back: eye, yaw, pitch.
    pub home: ([f32; 3], f32, f32),
    /// Follow the player when there is one (the default framing of camera.md 3 and the spring of 4), until the
    /// follow camera of `engine::camera` takes over.
    pub follow: bool,
    /// Current flying velocity.
    pub velocity: [f32; 3],
}

/// Default framing (camera.md 3): distance behind the focus and height above it.
pub const FOLLOW_DISTANCE: f32 = 450.0;
/// See [`FOLLOW_DISTANCE`].
pub const FOLLOW_HEIGHT: f32 = 125.0;
/// Position spring strength and damping (camera.md 4).
pub const FOLLOW_SPRING: (f32, f32) = (15.0, -8.0);
/// Manual orbit at full deflection, radians per second (camera.md 5).
pub const ORBIT_SPEED: f32 = 2.5;

impl Default for FreeCamera {
    fn default() -> Self {
        let v = CameraView::default();
        FreeCamera {
            pos: v.eye,
            yaw: core::f32::consts::FRAC_PI_2,
            pitch: -0.3,
            speed: BASE_SPEED,
            fov_y: v.fov_y,
            near: v.near,
            far: v.far,
            home: (v.eye, core::f32::consts::FRAC_PI_2, -0.3),
            follow: true,
            velocity: [0.0; 3],
        }
    }
}

fn atan2(y: f32, x: f32) -> f32 {
    // A short polynomial atan, accurate to about 1e-4 rad; only used to aim the camera.
    let ax = x.abs();
    let ay = y.abs();
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

impl FreeCamera {
    /// The view direction.
    pub fn forward(&self) -> [f32; 3] {
        let (sy, cy) = sin_cos(self.yaw);
        let (sp, cp) = sin_cos(self.pitch);
        [cp * cy, cp * sy, sp]
    }

    /// Places the eye at `eye` looking at `target`.
    pub fn look_at(&mut self, eye: [f32; 3], target: [f32; 3]) {
        let d = normalize(sub(target, eye));
        self.pos = eye;
        self.yaw = atan2(d[1], d[0]);
        self.pitch = atan2(d[2], crate::math::sqrt(d[0] * d[0] + d[1] * d[1]));
        self.home = (self.pos, self.yaw, self.pitch);
    }

    /// The current view.
    pub fn view(&self) -> CameraView {
        CameraView {
            eye: self.pos,
            target: add(self.pos, self.forward()),
            up: [0.0, 0.0, 1.0],
            fov_y: self.fov_y,
            near: self.near,
            far: self.far,
        }
    }

    /// One step of the controls.
    pub fn step(&mut self, input: &FrameInput, dt: f32) {
        if input.just_pressed(Buttons::UP) {
            self.speed = (self.speed * 2.0).min(BASE_SPEED * 64.0);
        }
        if input.just_pressed(Buttons::DOWN) {
            self.speed = (self.speed * 0.5).max(BASE_SPEED / 64.0);
        }
        if input.just_pressed(Buttons::Y) {
            (self.pos, self.yaw, self.pitch) = self.home;
        }
        self.yaw -= input.right[0] * LOOK_SPEED * dt;
        self.pitch = (self.pitch + input.right[1] * LOOK_SPEED * dt).clamp(-1.55, 1.55);
        let f = self.forward();
        let right = normalize(cross(f, [0.0, 0.0, 1.0]));
        let mut v = add(scale(f, input.left[1]), scale(right, input.left[0]));
        if input.held(Buttons::ZR) {
            v[2] += 1.0;
        }
        if input.held(Buttons::ZL) {
            v[2] -= 1.0;
        }
        let boost = if input.held(Buttons::B) { BOOST } else { 1.0 };
        self.pos = add(self.pos, scale(v, self.speed * boost * dt));
    }
}

impl CameraSystem for FreeCamera {
    fn update(
        &mut self,
        world: &World,
        input: &FrameInput,
        real_dt: f32,
        _collision: &dyn super::systems::CollisionQueries,
    ) -> CameraView {
        let focus = if self.follow { world.player.and_then(|p| world.position(p)) } else { None };
        match focus {
            Some(f) => {
                // Orbit with the right stick, keep the default framing, ease with the spring.
                let x = input.right[0];
                self.yaw -= x * x.abs() * ORBIT_SPEED * real_dt;
                let (sy, cy) = sin_cos(self.yaw);
                let goal = [f[0] - cy * FOLLOW_DISTANCE, f[1] - sy * FOLLOW_DISTANCE, f[2] + FOLLOW_HEIGHT + 100.0];
                let (k, c) = FOLLOW_SPRING;
                let acc = add(scale(sub(goal, self.pos), k), scale(self.velocity, c));
                self.velocity = add(self.velocity, scale(acc, real_dt));
                self.pos = add(self.pos, scale(self.velocity, real_dt));
                let target = [f[0], f[1], f[2] + FOLLOW_HEIGHT];
                let d = normalize(sub(target, self.pos));
                self.pitch = crate::math::atan2(d[2], crate::math::sqrt(d[0] * d[0] + d[1] * d[1]));
                CameraView { target, ..self.view() }
            }
            None => {
                self.step(input, real_dt);
                self.view()
            }
        }
    }

    fn cut(&mut self, eye: [f32; 3], target: [f32; 3]) {
        self.look_at(eye, target);
    }

    fn configure(&mut self, world: &World) {
        // Sly 1 meanings only (see `Game::new`); other games keep the defaults.
        let defaults = crate::object::OptionStore::new();
        let o = if world.sly1_meanings() { &world.world_options } else { &defaults };
        (self.fov_y, self.near, self.far) = lens(o);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_at_and_fly() {
        let mut c = FreeCamera::default();
        c.look_at([0.0, 0.0, 0.0], [100.0, 100.0, 0.0]);
        assert!((c.yaw - core::f32::consts::FRAC_PI_4).abs() < 1e-3);
        assert!(c.pitch.abs() < 1e-3);
        let input = FrameInput { left: [0.0, 1.0], ..Default::default() };
        c.step(&input, 1.0);
        let d = BASE_SPEED / core::f32::consts::SQRT_2;
        assert!((c.pos[0] - d).abs() < 1.0 && (c.pos[1] - d).abs() < 1.0);
        let v = c.view();
        assert!((v.target[2] - v.eye[2]).abs() < 1e-3);
        assert!((atan2(-1.0, -1.0) + 3.0 * core::f32::consts::FRAC_PI_4).abs() < 1e-3);
    }
}
