//! The class table (object-stream.md section 6): every class ID with its body layout and a descriptive name.
//!
//! 147 class IDs exist; 126 occur on the USA disc. The layout decides what an object carries (a transform, a model,
//! collision, children); the behaviour of a class is chosen by [`crate::behavior::Behavior::for_class`].

use formats::stream::{Layout, layout_of};

/// What the engine knows about a class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassInfo {
    /// Class ID.
    pub id: i16,
    /// Descriptive name (ours; roles marked "?" in the spec are tentative).
    pub name: &'static str,
    /// Body layout.
    pub layout: Layout,
}

impl ClassInfo {
    /// The layout carries a stored transform (rotation and position, or a position only).
    pub fn has_transform(&self) -> bool {
        !matches!(
            self.layout,
            Layout::Base
                | Layout::Shape
                | Layout::AnimationSegment
                | Layout::VisibilityMap
                | Layout::StateMachine
                | Layout::PathZone
                | Layout::ReachMap
                | Layout::Keyhole
        )
    }

    /// The layout contains the placed layout (a model, an extension block and a child list).
    pub fn is_placed(&self) -> bool {
        matches!(
            self.layout,
            Layout::Placed | Layout::Solid | Layout::Conveyor | Layout::Emitter | Layout::Flash | Layout::Dialogue
        )
    }

    /// The layout carries collision data (the solid layout).
    pub fn is_solid(&self) -> bool {
        matches!(self.layout, Layout::Solid | Layout::Conveyor)
    }
}

/// Number of class IDs (0 to 146).
pub const CLASS_COUNT: usize = 147;

/// Class IDs used by the engine by name.
pub mod id {
    /// Placed object.
    pub const PLACED: i16 = 2;
    /// Solid object.
    pub const SOLID: i16 = 3;
    /// Static world mesh.
    pub const WORLD_MESH: i16 = 4;
    /// Player character.
    pub const PLAYER: i16 = 7;
    /// Clue bottle.
    pub const CLUE: i16 = 46;
    /// Dialogue.
    pub const DIALOGUE: i16 = 101;
    /// Flashlight guard.
    pub const GUARD_FLASHLIGHT: i16 = 9;
    /// Proximity guard.
    pub const GUARD_PROXIMITY: i16 = 10;
    /// Companion character.
    pub const COMPANION: i16 = 14;
    /// Breakable piece.
    pub const PIECE: i16 = 39;
    /// Guard group.
    pub const GUARD_GROUP: i16 = 137;
    /// Button.
    pub const BUTTON: i16 = 40;
    /// Trigger volume button.
    pub const TRIGGER_VOLUME: i16 = 41;
    /// Trigger volume variant.
    pub const TRIGGER_VOLUME_2: i16 = 42;
    /// Checkpoint.
    pub const CHECKPOINT: i16 = 56;
    /// Proxy.
    pub const PROXY: i16 = 83;
    /// Charm.
    pub const CHARM: i16 = 86;
    /// Life token.
    pub const LIFE: i16 = 87;
    /// Coin.
    pub const COIN: i16 = 88;
    /// Key.
    pub const KEY: i16 = 89;
    /// Sprint hourglass (the "gold prize" of the class table).
    pub const HOURGLASS: i16 = 90;
    /// The world itself (never a record).
    pub const WORLD: i16 = 108;
    /// Warp point.
    pub const WARP: i16 = 116;
    /// Volume.
    pub const VOLUME: i16 = 123;
    /// Level exit.
    pub const EXIT: i16 = 125;
    /// Animation segment.
    pub const SEGMENT: i16 = 131;
    /// State machine.
    pub const STATE_MACHINE: i16 = 136;
}

/// The class's descriptive name, or `None` for IDs with no layout on the USA disc.
fn name_of(class: i16) -> Option<&'static str> {
    Some(match class {
        1 => "base object",
        2 => "placed object",
        3 => "solid object",
        4 => "static world mesh",
        7 => "player character",
        9 => "guard, smart variant",
        10..=13 | 15 | 17 | 18 => "guard variant",
        14 => "companion character",
        19 => "boss",
        20 => "tank",
        21 => "vehicle variant",
        22 => "turret",
        23 => "vault",
        24 => "puffer",
        25 => "vehicle",
        26 => "vehicle variant",
        27 => "jet pack",
        28 => "rhythm game object",
        29 => "minigame vehicle",
        31 => "water",
        33 => "breakable",
        34 => "breakable alarm",
        35 => "breakable gun",
        36 => "breakable variant",
        37 => "fragile breakable",
        38 => "electrified breakable",
        39 => "breakable piece",
        40 => "button",
        41 => "trigger volume button",
        42 => "trigger volume variant",
        43 => "physics bone",
        46 => "clue bottle",
        47 => "alarm",
        49 => "laser sensor",
        50 => "camera sensor",
        51 => "pressure sensor",
        52 => "barrier",
        54 => "hazard zone",
        55 => "hazard volume",
        56 => "checkpoint",
        57 => "conveyor",
        58 => "hiding place",
        59 => "bomb",
        60 => "missile",
        61 => "missile variant",
        62 => "homing missile",
        63 => "mine",
        64 => "fly",
        65 => "rat",
        66..=69 => "minigame actor",
        70 => "dart",
        71 => "jump object",
        72 => "puffer target",
        73 => "solid variant",
        74 => "final boss",
        75 => "final boss part",
        76 => "decoy",
        78 => "camera placement",
        79 => "skeleton bone",
        80 => "particle emitter",
        81 => "light",
        82 => "skeleton helper",
        83 => "proxy",
        84 => "sky",
        86 => "charm",
        87 => "life token",
        88 => "coin",
        89 => "key",
        90 => "sprint hourglass",
        91 => "race prize",
        92 => "lock",
        93 => "lock group",
        94 => "tail segment",
        95 => "placed variant",
        96 => "flash volume",
        97 => "dynamic shadow",
        98 => "waypoint",
        99 => "camera zone",
        100 => "jump location",
        101 => "dialogue",
        102 => "speaker",
        103 => "world map",
        104 => "spawner",
        105 => "hub selector",
        106 => "hub progress",
        107 => "integrity check object",
        110 => "curve shape",
        111 => "curve shape variant",
        112 => "pipe",
        113 => "rail",
        115 => "transform marker",
        116 => "warp point",
        117 => "target marker",
        118 => "handle marker",
        120 => "explosion group",
        121 => "explosion emitter",
        122 => "explosion emitter, simple",
        123 => "volume",
        124 => "rat hole",
        125 => "level exit",
        126 => "point",
        127 | 128 => "point variant",
        129 => "perch point",
        130 => "scan point",
        131 => "animation segment",
        134 => "visibility map",
        135 => "freeze group",
        136 => "state machine",
        137 => "guard group",
        138 => "path zone",
        139 => "reach map",
        140 | 141 | 143 => "object variant",
        142 => "screen mask",
        144 => "rhythm sequence",
        145 => "lightning flash",
        _ => return None,
    })
}

/// The class's information, or `None` for IDs that never occur as records on the USA disc.
pub fn class_info(class: i16) -> Option<ClassInfo> {
    Some(ClassInfo { id: class, name: name_of(class)?, layout: layout_of(class)? })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_disc_class_has_a_name_and_layout() {
        let known: alloc::vec::Vec<i16> = (0..CLASS_COUNT as i16).filter(|&c| class_info(c).is_some()).collect();
        // 126 classes occur on the disc; class 1 (base object) has a layout but no records. Every one of them is
        // named.
        assert_eq!(known.len(), 127, "{known:?}");
        for c in 0..CLASS_COUNT as i16 {
            assert_eq!(name_of(c).is_some(), layout_of(c).is_some(), "class {c}");
        }
        assert!(class_info(id::WORLD).is_none());
        assert!(class_info(id::PROXY).unwrap().has_transform());
        assert!(!class_info(id::PROXY).unwrap().is_placed());
        assert!(class_info(id::BUTTON).unwrap().is_solid());
        assert!(!class_info(id::SEGMENT).unwrap().has_transform());
    }
}
