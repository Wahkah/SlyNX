//! Damage kinds, charms and lives, power-ups and the hidden rules (player-moves.md 7, 8 and 8.1, guards.md 4.3,
//! pickups-and-progress.md 3, interactables.md 5).

use crate::math::{Vec3, dot, sub};

/// Damage kinds (option 609; interactables.md 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageKind {
    /// 0.
    Blunt,
    /// 1.
    Electric,
    /// 2.
    Fire,
    /// 3: falling into water.
    Water,
    /// 4.
    Crush,
    /// 5: falling into a pit.
    Pit,
    /// 6: thrown.
    Thrown,
}

impl DamageKind {
    /// From option 609's value (unknown values are blunt, the default).
    pub fn from_code(v: i16) -> DamageKind {
        match v {
            1 => DamageKind::Electric,
            2 => DamageKind::Fire,
            3 => DamageKind::Water,
            4 => DamageKind::Crush,
            5 => DamageKind::Pit,
            6 => DamageKind::Thrown,
            _ => DamageKind::Blunt,
        }
    }

    /// Whether a charm absorbs it: every kind, water and pits included (player-moves.md 8.1).
    pub fn charm_absorbs(self) -> bool {
        true
    }
}

/// What a hit did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageOutcome {
    /// Ignored: the player is flashing after an earlier hit, or already dead.
    Ignored,
    /// A charm was lost; the player flashes.
    CharmLost,
    /// A life was lost; the player dies and is respawned.
    LifeLost,
    /// No life left: the count went to −1 (pickups-and-progress.md 3.3).
    GameOver,
}

/// Charms and lives (pickups-and-progress.md 3.2, 3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Vitals {
    /// Charms, 0 to 2.
    pub charms: u8,
    /// Lives (−1 is game over).
    pub lives: i32,
}

impl Default for Vitals {
    fn default() -> Self {
        Vitals { charms: 0, lives: 5 }
    }
}

impl Vitals {
    /// Applies a hit of `kind`.
    pub fn hit(&mut self, kind: DamageKind) -> DamageOutcome {
        if kind.charm_absorbs() && self.charms > 0 {
            self.charms -= 1;
            return DamageOutcome::CharmLost;
        }
        self.lives -= 1;
        if self.lives < 0 { DamageOutcome::GameOver } else { DamageOutcome::LifeLost }
    }
}

/// Power-ups and the reward-mask bits each needs (player-moves.md 8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerUp {
    /// Dive attack (0x0012).
    Dive,
    /// Rolling ball (0x0004).
    RollingBall,
    /// Slow motion (0x1008).
    SlowMotion,
    /// Mine (0x0040).
    Mine,
    /// Speed-up (0x0080).
    SpeedUp,
    /// Decoy (0x0200).
    Decoy,
    /// Stun (0x8000).
    Stun,
}

impl PowerUp {
    /// Every power-up, in selection order.
    pub const ALL: [PowerUp; 7] = [
        PowerUp::Dive,
        PowerUp::RollingBall,
        PowerUp::SlowMotion,
        PowerUp::Mine,
        PowerUp::SpeedUp,
        PowerUp::Decoy,
        PowerUp::Stun,
    ];

    /// The reward-mask bits it needs.
    pub fn bits(self) -> u32 {
        match self {
            PowerUp::Dive => 0x0012,
            PowerUp::RollingBall => 0x0004,
            PowerUp::SlowMotion => 0x1008,
            PowerUp::Mine => 0x0040,
            PowerUp::SpeedUp => 0x0080,
            PowerUp::Decoy => 0x0200,
            PowerUp::Stun => 0x8000,
        }
    }

    /// Whether `mask` has every bit it needs.
    pub fn available(self, mask: u32) -> bool {
        mask & self.bits() == self.bits()
    }
}

/// Reward bit that pulls coins and clue bottles toward the player.
pub const MAGNET_BIT: u32 = 0x0020;
/// Reward bits that make the fourth hiding mode count as hidden (guards.md 4.3).
pub const FOURTH_HIDING_BITS: u32 = 0x12000;

/// How the player is hiding (guards.md 4.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hiding {
    /// Not hiding.
    None,
    /// Flattened against a wall; `facing` is the player's facing (away from the wall).
    Flattened {
        /// Facing direction.
        facing: Vec3,
        /// The spot is marked detectable.
        detectable: bool,
    },
    /// Ducking in cover.
    Ducking {
        /// Facing direction.
        facing: Vec3,
        /// The spot is marked detectable.
        detectable: bool,
    },
    /// In a hiding place; hidden while standing still.
    InHidingPlace {
        /// Whether the player is standing still.
        still: bool,
    },
    /// The fourth hiding mode.
    Fourth,
}

impl Hiding {
    /// Whether a guard at `guard` cannot see the player at `player`, given the reward mask (guards.md 4.3).
    pub fn hidden_from(&self, player: Vec3, guard: Vec3, reward_mask: u32) -> bool {
        let to_guard = sub(guard, player);
        match *self {
            Hiding::None => false,
            // The guard is on the wall side: behind the player's facing.
            Hiding::Flattened { facing, detectable } => !detectable && dot(facing, to_guard) < 0.0,
            // The guard is in front of the player.
            Hiding::Ducking { facing, detectable } => !detectable && dot(facing, to_guard) > 0.0,
            Hiding::InHidingPlace { still } => still,
            Hiding::Fourth => reward_mask & FOURTH_HIDING_BITS == FOURTH_HIDING_BITS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charms_lives_and_power_bits() {
        let mut v = Vitals { charms: 1, lives: 0 };
        assert_eq!(v.hit(DamageKind::Fire), DamageOutcome::CharmLost);
        assert_eq!(v.hit(DamageKind::Fire), DamageOutcome::GameOver);
        let mut v = Vitals { charms: 2, lives: 3 };
        // Water and pits use a charm too (player-moves.md 8.1).
        assert_eq!(v.hit(DamageKind::Water), DamageOutcome::CharmLost);
        assert_eq!((v.charms, v.lives), (1, 3));
        assert_eq!(DamageKind::from_code(6), DamageKind::Thrown);
        assert!(PowerUp::SlowMotion.available(0x1008));
        assert!(!PowerUp::SlowMotion.available(0x1000));
        assert!(PowerUp::Dive.available(0xFFFF));
        assert_eq!(DamageKind::from_code(5), DamageKind::Pit);
    }

    #[test]
    fn hidden_rules() {
        let f = Hiding::Flattened { facing: [1.0, 0.0, 0.0], detectable: false };
        assert!(f.hidden_from([0.0; 3], [-100.0, 0.0, 0.0], 0));
        assert!(!f.hidden_from([0.0; 3], [100.0, 0.0, 0.0], 0));
        let d = Hiding::Ducking { facing: [1.0, 0.0, 0.0], detectable: false };
        assert!(d.hidden_from([0.0; 3], [100.0, 0.0, 0.0], 0));
        assert!(!Hiding::Ducking { facing: [1.0, 0.0, 0.0], detectable: true }.hidden_from(
            [0.0; 3],
            [100.0, 0.0, 0.0],
            0
        ));
        assert!(Hiding::InHidingPlace { still: true }.hidden_from([0.0; 3], [1.0, 0.0, 0.0], 0));
        assert!(!Hiding::InHidingPlace { still: false }.hidden_from([0.0; 3], [1.0, 0.0, 0.0], 0));
        assert!(Hiding::Fourth.hidden_from([0.0; 3], [1.0; 3], 0x12000));
        assert!(!Hiding::Fourth.hidden_from([0.0; 3], [1.0; 3], 0x10000));
    }
}
