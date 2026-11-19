//! The free camera and the controls that drive it.
//!
//! The camera works in game space (right-handed, Z up, game units of about a centimetre). The renderer's camera
//! takes an `up` vector, so passing +Z makes the view matrix do the Y-up conversion of transforms.md 7.1 (a
//! rotation, so triangle winding is unchanged); no vertex or transform is converted.

use render::Camera;
use render::math::{Vec3, cos, sin};

/// Button bits (the platform layer's `Buttons`, which the harness's input uses too).
pub mod bits {
    pub const A: u32 = 1 << 0;
    pub const B: u32 = 1 << 1;
    pub const X: u32 = 1 << 2;
    pub const Y: u32 = 1 << 3;
    pub const L: u32 = 1 << 6;
    pub const R: u32 = 1 << 7;
    pub const ZL: u32 = 1 << 8;
    pub const ZR: u32 = 1 << 9;
    pub const PLUS: u32 = 1 << 10;
    pub const MINUS: u32 = 1 << 11;
    pub const LEFT: u32 = 1 << 12;
    pub const UP: u32 = 1 << 13;
    pub const RIGHT: u32 = 1 << 14;
    pub const DOWN: u32 = 1 << 15;
}

/// One frame of viewer controls, from any controller source.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    /// Left stick: x strafes right, y moves forward.
    pub fly: [f32; 2],
    /// Right stick: x turns right, y looks up.
    pub look: [f32; 2],
    /// ZR held: rise.
    pub rise: bool,
    /// ZL held: sink.
    pub sink: bool,
    /// B held: fly faster.
    pub boost: bool,
    /// R pressed: next world.
    pub next_world: bool,
    /// L pressed: previous world.
    pub prev_world: bool,
    /// X pressed: show or hide the stats overlay.
    pub toggle_overlay: bool,
    /// Y pressed: back to the spawn point.
    pub reset: bool,
    /// D-pad up / down pressed: double or halve the base speed.
    pub faster: bool,
    pub slower: bool,
    /// + pressed: quit.
    pub quit: bool,
}

impl Controls {
    /// From button bits (held and newly pressed this frame) and the two sticks (x right, y up, -1..1).
    pub fn from_bits(held: u32, pressed: u32, left: [f32; 2], right: [f32; 2]) -> Controls {
        let on = |b: u32| held & b != 0;
        let hit = |b: u32| pressed & b != 0;
        Controls {
            fly: left,
            look: right,
            rise: on(bits::ZR),
            sink: on(bits::ZL),
            boost: on(bits::B),
            next_world: hit(bits::R),
            prev_world: hit(bits::L),
            toggle_overlay: hit(bits::X),
            reset: hit(bits::Y),
            faster: hit(bits::UP),
            slower: hit(bits::DOWN),
            quit: hit(bits::PLUS),
        }
    }

    /// Player 1 of the platform's input.
    pub fn from_platform(input: &platform_api::InputState) -> Controls {
        let p = input.p1();
        Controls::from_bits(p.held.0, p.pressed.0, [p.left.x, p.left.y], [p.right.x, p.right.y])
    }
}

/// Where a level starts the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawn {
    pub eye: Vec3,
    pub target: Vec3,
}

/// A flying camera: position, heading (yaw, from +X towards +Y) and pitch (up positive).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreeCamera {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Base speed in game units per second.
    pub speed: f32,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
}

/// Default base speed: about 12 m/s.
pub const DEFAULT_SPEED: f32 = 1200.0;
/// Speed multiplier while boosting.
pub const BOOST: f32 = 5.0;
/// Turn rate at full stick, radians per second.
pub const TURN_RATE: f32 = 2.2;
const PITCH_LIMIT: f32 = 1.5;

impl Default for FreeCamera {
    fn default() -> Self {
        FreeCamera {
            pos: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            speed: DEFAULT_SPEED,
            fov_y: 0.95,
            near: 20.0,
            far: 150_000.0,
        }
    }
}

fn atan2(y: f32, x: f32) -> f32 {
    // A polynomial atan2 (max error about 1e-5 rad), so the result is the same on every target.
    let (ax, ay) = (x.abs(), y.abs());
    if ax == 0.0 && ay == 0.0 {
        return 0.0;
    }
    let a = ax.min(ay) / ax.max(ay);
    let s = a * a;
    let mut r = a
        * (0.999_977_26
            + s * (-0.332_623_47 + s * (0.193_543_46 + s * (-0.116_432_87 + s * (0.052_653_32 - s * 0.011_721_2)))));
    if ay > ax {
        r = core::f32::consts::FRAC_PI_2 - r;
    }
    if x < 0.0 {
        r = core::f32::consts::PI - r;
    }
    if y < 0.0 { -r } else { r }
}

impl FreeCamera {
    /// Places the camera at `spawn`, looking at its target.
    pub fn place(&mut self, spawn: Spawn) {
        self.pos = spawn.eye;
        let d = spawn.target - spawn.eye;
        let flat = render::math::sqrt(d.x * d.x + d.y * d.y);
        self.yaw = atan2(d.y, d.x);
        self.pitch = atan2(d.z, flat).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Unit view direction.
    pub fn forward(&self) -> Vec3 {
        let (cp, sp) = (cos(self.pitch), sin(self.pitch));
        Vec3::new(cp * cos(self.yaw), cp * sin(self.yaw), sp)
    }

    /// Unit vector to the right, horizontal.
    pub fn right(&self) -> Vec3 {
        Vec3::new(sin(self.yaw), -cos(self.yaw), 0.0)
    }

    /// One step of `dt` seconds.
    pub fn update(&mut self, c: &Controls, dt: f32) {
        if c.faster {
            self.speed = (self.speed * 2.0).min(DEFAULT_SPEED * 32.0);
        }
        if c.slower {
            self.speed = (self.speed * 0.5).max(DEFAULT_SPEED / 32.0);
        }
        self.yaw -= c.look[0] * TURN_RATE * dt;
        self.pitch = (self.pitch + c.look[1] * TURN_RATE * dt).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        let tau = core::f32::consts::TAU;
        if self.yaw > core::f32::consts::PI {
            self.yaw -= tau;
        } else if self.yaw < -core::f32::consts::PI {
            self.yaw += tau;
        }
        let speed = self.speed * if c.boost { BOOST } else { 1.0 } * dt;
        let vertical = c.rise as i32 as f32 - c.sink as i32 as f32;
        let step =
            self.forward() * (c.fly[1] * speed) + self.right() * (c.fly[0] * speed) + Vec3::Z * (vertical * speed);
        self.pos = self.pos + step;
    }

    /// The renderer's camera.
    pub fn camera(&self) -> Camera {
        Camera {
            eye: self.pos,
            target: self.pos + self.forward(),
            up: Vec3::Z,
            fov_y: self.fov_y,
            near: self.near,
            far: self.far,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-3
    }

    #[test]
    fn place_and_fly() {
        let mut c = FreeCamera::default();
        c.place(Spawn { eye: Vec3::new(0.0, 0.0, 100.0), target: Vec3::new(0.0, 500.0, 100.0) });
        assert!(close(c.forward(), Vec3::Y), "{:?}", c.forward());
        assert!(close(c.right(), Vec3::X));
        // Full stick forward for one second at base speed.
        c.update(&Controls { fly: [0.0, 1.0], ..Default::default() }, 1.0);
        assert!(close(c.pos, Vec3::new(0.0, DEFAULT_SPEED, 100.0)), "{:?}", c.pos);
        // Rise with boost.
        c.update(&Controls { rise: true, boost: true, ..Default::default() }, 0.5);
        assert!((c.pos.z - (100.0 + DEFAULT_SPEED * BOOST * 0.5)).abs() < 1e-2);
        // Looking right turns the heading clockwise seen from above.
        let yaw = c.yaw;
        c.update(&Controls { look: [1.0, 0.0], ..Default::default() }, 0.1);
        assert!(c.yaw < yaw);
        let cam = c.camera();
        assert_eq!(cam.up, Vec3::Z);
    }

    #[test]
    fn pitch_is_clamped_and_atan2_is_close() {
        let mut c = FreeCamera::default();
        c.update(&Controls { look: [0.0, 1.0], ..Default::default() }, 10.0);
        assert!(c.pitch <= PITCH_LIMIT);
        for (y, x) in [(1.0f32, 1.0f32), (-2.0, 0.5), (0.3, -4.0), (-1.0, -1.0), (0.0, -1.0)] {
            assert!((atan2(y, x) - y.atan2(x)).abs() < 2e-4, "{y} {x}");
        }
    }

    #[test]
    fn bits_map_to_controls() {
        let c = Controls::from_bits(bits::ZR | bits::B, bits::R | bits::X, [0.5, 0.0], [0.0, -1.0]);
        assert!(c.rise && c.boost && c.next_world && c.toggle_overlay);
        assert!(!c.sink && !c.prev_world && !c.quit);
        assert_eq!(c.fly, [0.5, 0.0]);
    }
}
