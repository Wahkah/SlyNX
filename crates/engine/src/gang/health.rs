//! Health, damage scaling, the damage flash and hit reactions (characters.md 2).

use super::consts::{GangConstants, spec};
use crate::math::{Vec3, cos, normalize, scale, sin, sqrt};

/// Hit kind bits (characters.md 2).
pub mod kind {
    /// Touch (never fatal).
    pub const TOUCH: u32 = 0x1;
    /// Hit by Murray.
    pub const MURRAY: u32 = 0x40;
    /// Hit by a thrown or fired object.
    pub const THROWN: u32 = 0x100;
    /// Knocked back along the hit's vector.
    pub const KNOCKBACK: u32 = 0x200;
    /// Crushed down.
    pub const CRUSH: u32 = 0x400;
    /// Stunned (two kinds).
    pub const STUN: u32 = 0x800 | 0x1000;
    /// Falls into hazards (two kinds).
    pub const HAZARD: u32 = 0x2000 | 0x4000;
    /// Heal.
    pub const HEAL: u32 = 0x10_0000;
}

/// A hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Amount (health units).
    pub amount: f32,
    /// Kind bits.
    pub kind: u32,
    /// Where the hit came from (the source's position).
    pub source: Vec3,
    /// The source's heading (Murray's frame for his knock-aside).
    pub source_yaw: f32,
    /// The hit's own vector (knock-back kind).
    pub vector: Vec3,
    /// The hitter was shrunk (Size Destabilizer, Reduction Bomb): half damage again.
    pub shrunk: bool,
}

/// The reaction a hit asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reaction {
    /// Ignored (flashing, or the safe house is the player).
    Ignored,
    /// Healed.
    Healed,
    /// A reaction with a launch velocity; `fatal` when the counter is empty.
    Hurt {
        /// Velocity set.
        velocity: Vec3,
        /// The counter went to 0.
        fatal: bool,
        /// The character cries out (launched faster than 400 upward).
        cry: bool,
    },
    /// Stopped dead (stunned).
    Stunned,
}

/// A character's health counter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    /// Current value.
    pub value: f32,
    /// Capacity (40).
    pub capacity: f32,
    /// Flash time left (no damage while flashing).
    pub flash: f32,
}

impl Default for Health {
    fn default() -> Self {
        Health { value: spec::HEALTH, capacity: spec::HEALTH, flash: 0.0 }
    }
}

impl Health {
    /// The scale of a change for difficulty help `h` (0 to 1): `(1 − h)·1 + h·0.5` (characters.md 2).
    pub fn scale(c: &GangConstants, help: f32) -> f32 {
        let h = help.clamp(0.0, 1.0);
        (1.0 - h) * c.help_limits[0] + h * c.help_limits[1]
    }

    /// Ages the flash.
    pub fn step(&mut self, dt: f32) {
        self.flash = (self.flash - dt).max(0.0);
    }

    /// Applies a hit to a character at `pos` facing `yaw`, with difficulty help `help`. `immune` is true while the
    /// safe house is the current player.
    pub fn hit(&mut self, c: &GangConstants, h: &Hit, pos: Vec3, help: f32, immune: bool) -> Reaction {
        if h.kind & kind::HEAL != 0 {
            self.value = (self.value + h.amount).min(self.capacity);
            return Reaction::Healed;
        }
        if immune || self.flash > 0.0 {
            return Reaction::Ignored;
        }
        let mut amount = h.amount * Health::scale(c, help);
        if h.shrunk {
            amount *= 0.5;
        }
        // Kinds 0 and 1 are touches: never fatal outright.
        let touch = h.kind & !kind::TOUCH == 0;
        if h.amount >= self.capacity && !touch {
            self.value = 0.0;
        } else {
            self.value = (self.value - amount).max(0.0);
        }
        self.flash = spec::FLASH;
        let fatal = self.value <= 0.0;
        if h.kind & kind::STUN != 0 {
            return Reaction::Stunned;
        }
        let velocity = if h.kind & kind::MURRAY != 0 {
            // In Murray's frame; the side by which side of the point 150 behind him the victim is.
            let (s, co) = (sin(h.source_yaw), cos(h.source_yaw));
            let fwd = [co, s, 0.0];
            let left = [-s, co, 0.0];
            let behind = [h.source[0] - 150.0 * co, h.source[1] - 150.0 * s, h.source[2]];
            let side = (pos[0] - behind[0]) * left[0] + (pos[1] - behind[1]) * left[1];
            let v = if side >= 0.0 { c.murray_knock[0] } else { c.murray_knock[1] };
            [fwd[0] * v[0] + left[0] * v[1], fwd[1] * v[0] + left[1] * v[1], v[2]]
        } else if h.kind & kind::THROWN != 0 {
            let away = normalize([pos[0] - h.source[0], pos[1] - h.source[1], 0.0]);
            let mut v = scale(away, c.thrown_knock[0]);
            v[2] = c.thrown_knock[1];
            v
        } else if h.kind & kind::KNOCKBACK != 0 {
            h.vector
        } else if h.kind & kind::CRUSH != 0 {
            c.crush
        } else {
            [0.0; 3]
        };
        Reaction::Hurt { velocity, fatal, cry: velocity[2] > spec::CRY_SPEED }
    }

    /// How far a fatal knock-back turns the character toward its flight: `clamp(−1 + 0.004·|v_xy|, 0, 1)`, when
    /// the knock-back is faster than 250 against the facing (characters.md 2).
    pub fn flight_turn(v: Vec3) -> f32 {
        let s = sqrt(v[0] * v[0] + v[1] * v[1]);
        if s <= 250.0 {
            return 0.0;
        }
        (-1.0 + 0.004 * s).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(amount: f32, kind: u32) -> Hit {
        Hit { amount, kind, source: [0.0; 3], source_yaw: 0.0, vector: [0.0; 3], shrunk: false }
    }

    #[test]
    fn scaling_flash_and_reactions() {
        let c = GangConstants::SPEC;
        assert_eq!(Health::scale(&c, 0.0), 1.0);
        assert_eq!(Health::scale(&c, 1.0), 0.5);
        assert_eq!(Health::scale(&c, 0.5), 0.75);
        let mut h = Health::default();
        assert!(matches!(
            h.hit(&c, &hit(10.0, kind::CRUSH), [0.0; 3], 0.0, false),
            Reaction::Hurt { velocity: [0.0, 0.0, -1000.0], fatal: false, .. }
        ));
        assert_eq!(h.value, 30.0);
        // Flashing for 2 s: the next hit is ignored.
        assert_eq!(h.hit(&c, &hit(10.0, kind::CRUSH), [0.0; 3], 0.0, false), Reaction::Ignored);
        h.step(2.0);
        // Full help halves; a shrunk hitter halves again.
        h.hit(&c, &Hit { shrunk: true, ..hit(8.0, kind::CRUSH) }, [0.0; 3], 1.0, false);
        assert_eq!(h.value, 28.0);
        h.step(2.0);
        // A hit of the capacity or more is fatal.
        assert!(matches!(h.hit(&c, &hit(40.0, kind::CRUSH), [0.0; 3], 1.0, false), Reaction::Hurt { fatal: true, .. }));
        assert_eq!(h.value, 0.0);
        assert_eq!(h.hit(&c, &hit(10.0, kind::HEAL), [0.0; 3], 0.0, false), Reaction::Healed);
        assert_eq!(h.value, 10.0);
        // Immune while the safe house is the player.
        h.step(2.0);
        assert_eq!(h.hit(&c, &hit(5.0, kind::CRUSH), [0.0; 3], 0.0, true), Reaction::Ignored);
    }

    #[test]
    fn knock_velocities() {
        let c = GangConstants::SPEC;
        let mut h = Health::default();
        // Murray at the origin facing +x; a victim to his left gets (1000, 900, 950) in his frame.
        let r = h.hit(&c, &hit(1.0, kind::MURRAY), [50.0, 30.0, 0.0], 0.0, false);
        assert!(matches!(r, Reaction::Hurt { velocity: [1000.0, 900.0, 950.0], cry: true, .. }), "{r:?}");
        h.step(3.0);
        let r = h.hit(&c, &hit(1.0, kind::MURRAY), [50.0, -30.0, 0.0], 0.0, false);
        assert!(matches!(r, Reaction::Hurt { velocity: [1000.0, -900.0, 950.0], .. }));
        h.step(3.0);
        // Thrown object from −x: 600 away plus 800 up.
        let r = h.hit(&c, &Hit { source: [-100.0, 0.0, 0.0], ..hit(1.0, kind::THROWN) }, [0.0; 3], 0.0, false);
        assert!(matches!(r, Reaction::Hurt { velocity: [600.0, 0.0, 800.0], .. }), "{r:?}");
        assert_eq!(Health::flight_turn([500.0, 0.0, 0.0]), 1.0);
        assert!((Health::flight_turn([300.0, 0.0, 0.0]) - 0.2).abs() < 1e-6);
        assert_eq!(Health::flight_turn([200.0, 0.0, 0.0]), 0.0);
    }
}
