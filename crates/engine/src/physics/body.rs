//! Bodies: mass, gravity, terminal fall speed, water, axis locks and contact probes (physics.md 1 and 2,
//! options-by-class.md 2.2).

use alloc::vec::Vec;

use formats::collision::CollisionMesh;
use formats::disc::Executable;
use formats::geometry::Affine;

use crate::collision::BodyId as CollisionBodyId;
use crate::math::{MAT3_IDENTITY, Mat3, Vec3, dot, inverse, mat_mul, mat_mul_vec, scale, sqrt, transpose};

/// The world's default gravity, units/s² along z (physics.md 2).
pub const WORLD_GRAVITY: f32 = -1500.0;
/// The player character's base gravity before its gravity ratio (physics.md 2).
pub const PLAYER_GRAVITY: f32 = -1750.0;
/// The player character's gravity ratio (physics.md 2).
pub const PLAYER_GRAVITY_RATIO: f32 = 1.3;
/// Sly 2 world gravity (Sly 2 physics.md); the characters use 1.3 times it.
pub const SLY2_GRAVITY: f32 = -1750.0;
/// The gravity of dropped pickups (physics.md 2).
pub const PICKUP_GRAVITY: f32 = -2250.0;
/// Default buoyancy (option 134).
pub const DEFAULT_BUOYANCY: f32 = 0.5;
/// Default viscosity (option 135).
pub const DEFAULT_VISCOSITY: f32 = 1.0;

/// Virtual address of the base terminal fall speed (−750) in SCUS_971.98 (physics.md 2).
pub const TERMINAL_SPEED_VA: u32 = 0x0026_3E78;
/// Virtual address of the terminal-speed relaxation rate (5 per second).
pub const TERMINAL_RATE_VA: u32 = 0x0026_3E7C;

/// The terminal fall rule (physics.md 2): below `v_t = speed · √ratio` an extra `rate · (v_t − v_z)` is added to the
/// frame's acceleration on top of gravity, so the fall settles at `v_t + g / rate` (−1310.1 for the player).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerminalFall {
    /// Base terminal vertical speed (negative), before the gravity ratio.
    pub speed: f32,
    /// Relaxation rate, per second.
    pub rate: f32,
}

impl TerminalFall {
    /// Reads the two constants from the player's executable, after checking its SHA-256.
    pub fn from_executable(exe: &Executable) -> formats::Result<TerminalFall> {
        if !exe.is_supported() {
            return Err(formats::Error::UnknownExecutable);
        }
        let f = |va: u32| -> formats::Result<f32> {
            let b = exe.bytes_at(va, 4)?;
            Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        Ok(TerminalFall { speed: f(TERMINAL_SPEED_VA)?, rate: f(TERMINAL_RATE_VA)? })
    }

    /// The speed at which the drag starts, for a gravity ratio (−855.1 for the player).
    pub fn speed_for(&self, ratio: f32) -> f32 {
        self.speed * sqrt(ratio)
    }

    /// The extra vertical acceleration at vertical speed `vz` (0 above the drag speed).
    pub fn drag(&self, ratio: f32, vz: f32) -> f32 {
        let vt = self.speed_for(ratio);
        if vz < vt { self.rate * (vt - vz) } else { 0.0 }
    }

    /// The speed the fall settles at under gravity `g` (negative): `v_t + g / rate`.
    pub fn settling_speed(&self, ratio: f32, g: f32) -> f32 {
        self.speed_for(ratio) + if self.rate > 0.0 { g / self.rate } else { 0.0 }
    }
}

/// An axis lock (options 131 and 132): which directions a body may move or turn in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AxisLock {
    /// 0: free.
    #[default]
    Free,
    /// 1: locked.
    Locked,
    /// 2, 3, 4: only along local x, y, z (index 0, 1, 2).
    Along(u8),
    /// 5, 6, 7: only in the plane across local z, y, x (index of the blocked axis: 2, 1, 0).
    Plane(u8),
}

impl AxisLock {
    /// From the option value.
    pub fn from_code(v: i16) -> Option<AxisLock> {
        Some(match v {
            0 => AxisLock::Free,
            1 => AxisLock::Locked,
            2..=4 => AxisLock::Along((v - 2) as u8),
            5 => AxisLock::Plane(2),
            6 => AxisLock::Plane(1),
            7 => AxisLock::Plane(0),
            _ => return None,
        })
    }

    /// Projects a world-space vector onto what the lock allows, given the body's rotation.
    pub fn apply(&self, rotation: &Mat3, v: Vec3) -> Vec3 {
        match *self {
            AxisLock::Free => v,
            AxisLock::Locked => [0.0; 3],
            AxisLock::Along(i) => {
                let a = rotation[i as usize];
                scale(a, dot(a, v))
            }
            AxisLock::Plane(i) => {
                let a = rotation[i as usize];
                crate::math::madd(v, a, -dot(a, v))
            }
        }
    }
}

/// A contact probe: a sphere in the body's local space that is tested against the collision world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    /// Centre, local space.
    pub center: Vec3,
    /// Radius.
    pub radius: f32,
}

/// Probes at a collision mesh's positions (corners of boxes and the like), with radius `radius`.
pub fn probes_from_mesh(mesh: &CollisionMesh, radius: f32) -> Vec<Probe> {
    mesh.positions.iter().map(|&center| Probe { center, radius }).collect()
}

/// How a body moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// Moved by gravity, forces and contacts.
    Dynamic,
    /// Moved by the caller (animation, scripts); infinite mass in contacts.
    Kinematic,
}

/// What a body is made of.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyDesc {
    /// Dynamic or kinematic.
    pub motion: Motion,
    /// Mass (option 127).
    pub mass: f32,
    /// Moment of inertia in local space (classes/solid-object.md row 5); the identity times the mass if unknown.
    pub inertia: Mat3,
    /// Centre of mass, local space.
    pub center_of_mass: Vec3,
    /// Gravity (option 129 sets a vector; option 138 gives zero).
    pub gravity: Vec3,
    /// Gravity ratio (option 711; 1 for objects that are not actors).
    pub gravity_ratio: f32,
    /// Terminal fall rule (the player).
    pub terminal: Option<TerminalFall>,
    /// Material (option 128).
    pub material: i16,
    /// Buoyancy (option 134).
    pub buoyancy: f32,
    /// Viscosity (option 135).
    pub viscosity: f32,
    /// Takes part in collisions with other solids (option 130 clears it).
    pub interacts: bool,
    /// Translation lock (option 131).
    pub lock_translation: AxisLock,
    /// Rotation lock (option 132).
    pub lock_rotation: AxisLock,
    /// Contact probes (local space).
    pub probes: Vec<Probe>,
    /// The body's own entry in the collision world, if it has one (ignored by its own probes and moved with it).
    pub collision: Option<CollisionBodyId>,
}

impl Default for BodyDesc {
    fn default() -> Self {
        BodyDesc {
            motion: Motion::Dynamic,
            mass: 1.0,
            inertia: MAT3_IDENTITY,
            center_of_mass: [0.0; 3],
            gravity: [0.0, 0.0, WORLD_GRAVITY],
            gravity_ratio: 1.0,
            terminal: None,
            material: 0,
            buoyancy: DEFAULT_BUOYANCY,
            viscosity: DEFAULT_VISCOSITY,
            interacts: true,
            lock_translation: AxisLock::Free,
            lock_rotation: AxisLock::Free,
            probes: Vec::new(),
            collision: None,
        }
    }
}

/// A body in a [`crate::physics::PhysicsWorld`].
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    /// Settings.
    pub desc: BodyDesc,
    /// Local to world (the body's origin, not its centre of mass).
    pub transform: Affine,
    /// Velocity of the centre of mass.
    pub velocity: Vec3,
    /// Angular velocity (world space, radians per second).
    pub angular_velocity: Vec3,
    /// Force accumulated for the next step (cleared by the step).
    pub force: Vec3,
    /// Torque accumulated for the next step.
    pub torque: Vec3,
    /// Fraction of the body under water (0 to 1), set by the caller from the water volumes.
    pub submerged: f32,
    /// Velocity of the water's current (viscosity pulls toward it), set by the caller with `submerged`.
    pub current: Vec3,
    /// The water surface's normal (up by default).
    pub water_normal: Vec3,
    /// The water object's buoyancy and viscosity factors (both 1 by default), multiplied into the body's own.
    pub water_factors: [f32; 2],
    pub(crate) still_frames: u8,
    pub(crate) asleep: bool,
    pub(crate) inv_inertia_local: Mat3,
}

impl Body {
    /// A body at `transform`.
    pub fn new(desc: BodyDesc, transform: Affine) -> Body {
        let inv_inertia_local = inverse(&desc.inertia).unwrap_or([[0.0; 3]; 3]);
        Body {
            desc,
            transform,
            velocity: [0.0; 3],
            angular_velocity: [0.0; 3],
            force: [0.0; 3],
            torque: [0.0; 3],
            submerged: 0.0,
            current: [0.0; 3],
            water_normal: [0.0, 0.0, 1.0],
            water_factors: [1.0, 1.0],
            still_frames: 0,
            asleep: false,
            inv_inertia_local,
        }
    }

    /// 1 / mass for a dynamic body, 0 for a kinematic one.
    pub fn inv_mass(&self) -> f32 {
        match self.desc.motion {
            Motion::Dynamic if self.desc.mass > 0.0 => 1.0 / self.desc.mass,
            _ => 0.0,
        }
    }

    /// The world-space inverse inertia `R I⁻¹ Rᵀ` (zero for a kinematic body).
    pub fn inv_inertia_world(&self) -> Mat3 {
        if self.inv_mass() == 0.0 {
            return [[0.0; 3]; 3];
        }
        let r = &self.transform.cols;
        mat_mul(&mat_mul(r, &self.inv_inertia_local), &transpose(r))
    }

    /// The world position of the centre of mass.
    pub fn center_of_mass(&self) -> Vec3 {
        self.transform.transform_point(self.desc.center_of_mass)
    }

    /// Velocity of the body's material at world point `p`.
    pub fn velocity_at(&self, p: Vec3) -> Vec3 {
        let r = crate::math::sub(p, self.center_of_mass());
        crate::math::add(self.velocity, crate::math::cross(self.angular_velocity, r))
    }

    /// Whether the body is resting (static after two still frames, physics.md 3.2 step 7).
    pub fn is_at_rest(&self) -> bool {
        self.asleep
    }

    /// Wakes the body.
    pub fn wake(&mut self) {
        self.asleep = false;
        self.still_frames = 0;
    }

    /// Changes the gravity ratio: gravity scales by `new / old` and mass by `old / new` (physics.md 2).
    pub fn set_gravity_ratio(&mut self, ratio: f32) {
        let old = self.desc.gravity_ratio;
        if ratio <= 0.0 || old <= 0.0 {
            return;
        }
        self.desc.gravity = scale(self.desc.gravity, ratio / old);
        self.desc.mass *= old / ratio;
        self.desc.inertia = [
            scale(self.desc.inertia[0], old / ratio),
            scale(self.desc.inertia[1], old / ratio),
            scale(self.desc.inertia[2], old / ratio),
        ];
        self.inv_inertia_local = inverse(&self.desc.inertia).unwrap_or([[0.0; 3]; 3]);
        self.desc.gravity_ratio = ratio;
    }

    /// Applies an impulse at world point `p` and wakes the body.
    pub fn apply_impulse(&mut self, impulse: Vec3, p: Vec3) {
        self.add_impulse(impulse, p);
        if self.inv_mass() != 0.0 {
            self.wake();
        }
    }

    /// Applies an impulse without waking the body (the solver's own impulses).
    pub(crate) fn add_impulse(&mut self, impulse: Vec3, p: Vec3) {
        let im = self.inv_mass();
        if im == 0.0 {
            return;
        }
        self.velocity = crate::math::madd(self.velocity, impulse, im);
        let r = crate::math::sub(p, self.center_of_mass());
        let dw = mat_mul_vec(&self.inv_inertia_world(), crate::math::cross(r, impulse));
        self.angular_velocity = crate::math::add(self.angular_velocity, dw);
    }
}

/// The player character's body settings: gravity −1750 × 1.3 = −2275 and the terminal fall rule (physics.md 2,
/// player-moves.md 1).
pub fn player_desc(terminal: TerminalFall) -> BodyDesc {
    BodyDesc {
        gravity: [0.0, 0.0, PLAYER_GRAVITY * PLAYER_GRAVITY_RATIO],
        gravity_ratio: PLAYER_GRAVITY_RATIO,
        terminal: Some(terminal),
        ..BodyDesc::default()
    }
}

/// A dropped pickup's settings: gravity −2250.
pub fn pickup_desc() -> BodyDesc {
    BodyDesc { gravity: [0.0, 0.0, PICKUP_GRAVITY], ..BodyDesc::default() }
}

/// The submerged fraction of a sphere of radius `r` centred at height `z` under a water surface at `surface`
/// (volume fraction of the spherical cap below the surface).
pub fn sphere_submerged_fraction(z: f32, r: f32, surface: f32) -> f32 {
    if r <= 0.0 {
        return if z < surface { 1.0 } else { 0.0 };
    }
    let h = (surface - (z - r)).clamp(0.0, 2.0 * r);
    // Cap volume pi h² (3r − h) / 3 over the sphere's 4 pi r³ / 3.
    h * h * (3.0 * r - h) / (4.0 * r * r * r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gravity_ratio_scales_gravity_and_mass() {
        let mut b =
            Body::new(BodyDesc { mass: 2.0, gravity: [0.0, 0.0, -1750.0], ..Default::default() }, Affine::IDENTITY);
        b.set_gravity_ratio(1.3);
        assert!((b.desc.gravity[2] + 2275.0).abs() < 1e-3);
        assert!((b.desc.mass - 2.0 / 1.3).abs() < 1e-6);
        let t = TerminalFall { speed: -750.0, rate: 5.0 };
        assert!((t.speed_for(1.3) + 855.1).abs() < 0.05);
    }

    #[test]
    fn locks_and_water() {
        let v = [1.0, 2.0, 3.0];
        assert_eq!(AxisLock::from_code(4).unwrap().apply(&MAT3_IDENTITY, v), [0.0, 0.0, 3.0]);
        assert_eq!(AxisLock::from_code(5).unwrap().apply(&MAT3_IDENTITY, v), [1.0, 2.0, 0.0]);
        assert_eq!(AxisLock::from_code(1).unwrap().apply(&MAT3_IDENTITY, v), [0.0; 3]);
        assert_eq!(sphere_submerged_fraction(0.0, 1.0, 5.0), 1.0);
        assert_eq!(sphere_submerged_fraction(0.0, 1.0, -5.0), 0.0);
        assert!((sphere_submerged_fraction(0.0, 1.0, 0.0) - 0.5).abs() < 1e-6);
    }
}
