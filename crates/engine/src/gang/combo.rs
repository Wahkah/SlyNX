//! Sly 3's combo definitions and their runner (specs/sly3/characters.md 5).
//!
//! A combo definition is an object of class 148, a child of the character it serves (938 under Sly, class 52; the
//! others under Bentley and the guest characters). Its start-up sets the press window to 0.35 s and a steering speed
//! of 800 [X]. Options [X for the fields; R for the meanings]:
//!
//! | Option | Meaning |
//! |---:|---|
//! | 1659 | the button that continues the combo (pad bits: `0x10` Triangle, `0x20` Circle, `0x40` Cross, `0x80` Square) |
//! | 1667 | a priority (0 to 5; the highest wins) |
//! | 1668 | the press window (no record sets it) |
//! | 1673, 1676, 1679 | the clips: standing, while moving, follow-up after a hit |
//! | 1680, 1681 | two speed factors (2.2 / 1.6, 1.6 / 1.15, 1.3 / 0.85) |
//! | 1683 | the abilities (item numbers) one of which must be owned |
//!
//! The runner's states [R]: off, attack, attack while moving (an overlay on the run), follow-up after a hit, and
//! recovery. While it runs Sly is in Sly 3's state 23 (generic 9, the combo attack), the gang's [`super::sly::COMBO`].
//!
//! *Choices of this port:* a press starts or continues the combo with the allowed definition of the highest priority
//! for that button (the first in stream order on a tie); a definition with neither a standing nor a moving clip is
//! skipped; an attack or follow-up lasts the caller's clip time, then recovery lasts the press window, during which
//! a press continues the combo. Option 1683 occurs on no record of the disc, so every definition is allowed unless it
//! lists abilities. Options 1660, 1662, 1665, 1666, 1670, 1671 and 1682 are kept raw (meanings open).

use alloc::vec::Vec;

use super::input::button;
use crate::behavior::sly3::interactables::{arg_i32, lists};
use crate::object::Handle;
use crate::world::World;

/// The combo definitions' class.
pub const CLASS: i16 = 148;
/// The press window set by every definition (field +0x70) [X].
pub const WINDOW: f32 = 0.35;
/// The steering speed set by every definition (field +0x74) [X].
pub const STEER: f32 = 800.0;
/// Sly 3's state number of the combo attack (generic 9) [X].
pub const SLY3_STATE: i32 = 23;

/// Option IDs.
pub mod opt {
    /// The button that continues the combo (pad bits).
    pub const BUTTON: u16 = 1659;
    /// Priority.
    pub const PRIORITY: u16 = 1667;
    /// The press window.
    pub const WINDOW: u16 = 1668;
    /// Clip while standing.
    pub const CLIP_STANDING: u16 = 1673;
    /// Clip while moving.
    pub const CLIP_MOVING: u16 = 1676;
    /// Clip of the follow-up after a hit.
    pub const CLIP_FOLLOW_UP: u16 = 1679;
    /// First speed factor.
    pub const SPEED_1: u16 = 1680;
    /// Second speed factor.
    pub const SPEED_2: u16 = 1681;
    /// Abilities, one of which must be owned.
    pub const ABILITIES: u16 = 1683;
    /// Options kept raw (meanings open).
    pub const RAW: [u16; 7] = [1660, 1662, 1665, 1666, 1670, 1671, 1682];
}

/// The gang's button for a combo pad bit (`0x10` Triangle, `0x20` Circle, `0x40` Cross, `0x80` Square), or 0.
pub fn button_of_pad(bits: i32) -> u32 {
    let mut b = 0;
    for (pad, g) in [(0x10, button::TRIANGLE), (0x20, button::CIRCLE), (0x40, button::CROSS), (0x80, button::SQUARE)] {
        if bits & pad != 0 {
            b |= g;
        }
    }
    b
}

/// One combo definition.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComboDef {
    /// The definition's object ID.
    pub oid: i16,
    /// The button that continues the combo, as gang buttons (0: none).
    pub button: u32,
    /// Priority (1667; 0 when unset).
    pub priority: i32,
    /// The press window (1668, else [`WINDOW`]).
    pub window: f32,
    /// Clips: standing, moving, follow-up (−1 none).
    pub clips: [i32; 3],
    /// The two speed factors (1680, 1681), if given.
    pub speed: Option<[f32; 2]>,
    /// Abilities, one of which must be owned (1683; empty: none needed).
    pub abilities: Vec<u8>,
    /// The other options, raw: (option, integer value).
    pub raw: Vec<(u16, i32)>,
}

impl ComboDef {
    /// From the options of a class-148 object.
    pub fn new(world: &World, h: Handle) -> ComboDef {
        let Some(o) = world.options(h) else { return ComboDef::default() };
        let i = |id| o.i32(id).unwrap_or(-1);
        let abilities = o
            .all(opt::ABILITIES)
            .filter_map(|v| v.as_i32())
            .chain(lists(world, h, opt::ABILITIES).flat_map(|a| a.iter().filter_map(arg_i32)))
            .map(|n| n as u8)
            .collect();
        ComboDef {
            oid: world.oid(h).unwrap_or(-1),
            button: button_of_pad(o.i32(opt::BUTTON).unwrap_or(0)),
            priority: o.i32(opt::PRIORITY).unwrap_or(0),
            window: o.f32(opt::WINDOW).unwrap_or(WINDOW),
            clips: [i(opt::CLIP_STANDING), i(opt::CLIP_MOVING), i(opt::CLIP_FOLLOW_UP)],
            speed: o.f32(opt::SPEED_1).zip(o.f32(opt::SPEED_2)).map(|(a, b)| [a, b]),
            abilities,
            raw: opt::RAW.iter().filter_map(|&id| o.i32(id).map(|v| (id, v))).collect(),
        }
    }

    /// Whether `owned` allows it (one of its abilities owned, or none listed).
    pub fn allowed(&self, owned: impl Fn(u8) -> bool) -> bool {
        self.abilities.is_empty() || self.abilities.iter().any(|&n| owned(n))
    }

    /// The clip for an attack, standing or moving (falling back to the other).
    pub fn clip(&self, moving: bool) -> Option<i32> {
        let [standing, move_clip, _] = self.clips;
        let (a, b) = if moving { (move_clip, standing) } else { (standing, move_clip) };
        [a, b].into_iter().find(|&c| c >= 0)
    }
}

/// A character's combo definitions.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ComboSet {
    /// In stream order.
    pub defs: Vec<ComboDef>,
}

impl ComboSet {
    /// The class-148 children of the character object `h`.
    pub fn from_world(world: &World, h: Handle) -> ComboSet {
        let defs =
            world.children(h).filter(|&c| world.class(c) == Some(CLASS)).map(|c| ComboDef::new(world, c)).collect();
        ComboSet { defs }
    }

    /// The definitions of the first object of `class` in the world (Sly: 52).
    pub fn for_class(world: &World, class: i16) -> ComboSet {
        world
            .slots_of_class(class)
            .first()
            .and_then(|&s| world.handle(s))
            .map_or_else(ComboSet::default, |h| ComboSet::from_world(world, h))
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// The allowed definition for `b` with the highest priority (the first on a tie) that has an attack clip.
    pub fn choose(&self, b: u32, owned: impl Fn(u8) -> bool) -> Option<usize> {
        let mut best: Option<(i32, usize)> = None;
        for (i, d) in self.defs.iter().enumerate() {
            if d.button & b == 0 || !d.allowed(&owned) || d.clip(false).is_none() {
                continue;
            }
            if best.is_none_or(|(p, _)| d.priority > p) {
                best = Some((d.priority, i));
            }
        }
        best.map(|b| b.1)
    }
}

/// The runner's state [R].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// No combo.
    #[default]
    Off,
    /// An attack, standing.
    Attack,
    /// An attack while moving (an overlay on the run).
    Moving,
    /// The follow-up after a hit.
    FollowUp,
    /// Recovery: a press within the window continues the combo.
    Recovery,
}

/// What the runner asks the character to play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// Play `clip` for definition `def` (an index into the set).
    Clip {
        /// The definition.
        def: usize,
        /// The clip's object ID.
        clip: i32,
    },
    /// The combo ended.
    Ended,
}

/// The combo runner.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Runner {
    /// State.
    pub phase: Phase,
    /// The current definition.
    pub def: Option<usize>,
    /// The clip playing.
    pub clip: i32,
    /// Time in the state.
    pub time: f32,
}

impl Runner {
    /// Whether a combo is running.
    pub fn active(&self) -> bool {
        self.phase != Phase::Off
    }

    /// A press of `b`: starts the combo when off, continues it in recovery. Returns the clip to play.
    pub fn press(&mut self, set: &ComboSet, b: u32, moving: bool, owned: impl Fn(u8) -> bool) -> Option<Cue> {
        if !matches!(self.phase, Phase::Off | Phase::Recovery) {
            return None;
        }
        let i = set.choose(b, owned)?;
        let clip = set.defs[i].clip(moving)?;
        let moving_clip = moving && set.defs[i].clips[1] >= 0;
        *self =
            Runner { phase: if moving_clip { Phase::Moving } else { Phase::Attack }, def: Some(i), clip, time: 0.0 };
        Some(Cue::Clip { def: i, clip })
    }

    /// The attack hit something: the follow-up clip, if the definition has one.
    pub fn hit(&mut self, set: &ComboSet) -> Option<Cue> {
        if !matches!(self.phase, Phase::Attack | Phase::Moving) {
            return None;
        }
        let i = self.def?;
        let clip = set.defs.get(i)?.clips[2];
        if clip < 0 {
            return None;
        }
        *self = Runner { phase: Phase::FollowUp, def: Some(i), clip, time: 0.0 };
        Some(Cue::Clip { def: i, clip })
    }

    /// One step: an attack or follow-up ends after `clip_time`, then recovery lasts the definition's window.
    pub fn update(&mut self, set: &ComboSet, dt: f32, clip_time: f32) -> Option<Cue> {
        self.time += dt;
        match self.phase {
            Phase::Attack | Phase::Moving | Phase::FollowUp if self.time >= clip_time => {
                self.phase = Phase::Recovery;
                self.time = 0.0;
                None
            }
            Phase::Recovery => {
                let window = self.def.and_then(|i| set.defs.get(i)).map_or(WINDOW, |d| d.window);
                if self.time > window {
                    *self = Runner::default();
                    Some(Cue::Ended)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(pad: i32, priority: i32, clips: [i32; 3], abilities: &[u8]) -> ComboDef {
        ComboDef {
            oid: priority as i16,
            button: button_of_pad(pad),
            priority,
            window: WINDOW,
            clips,
            abilities: abilities.to_vec(),
            ..ComboDef::default()
        }
    }

    #[test]
    fn runner_picks_by_priority_and_times_out() {
        // Like Sly's set: Triangle definitions with a follow-up, Square finishers at priorities 4, 3, 2.
        let set = ComboSet {
            defs: alloc::vec![
                def(0x10, 2, [8246, 8246, 8244], &[]),
                def(0x10, 5, [239, -1, 8244], &[]),
                def(0x80, 3, [8240, -1, -1], &[44]),
                def(0x80, 4, [8238, -1, -1], &[45]),
                def(0x20, 4, [-1, -1, -1], &[]),
            ],
        };
        assert_eq!(button_of_pad(0x80), button::SQUARE);
        let mut r = Runner::default();
        // Triangle: the priority-5 definition, standing clip; moving falls back to it as it has no moving clip.
        assert_eq!(r.press(&set, button::TRIANGLE, true, |_| false), Some(Cue::Clip { def: 1, clip: 239 }));
        assert_eq!(r.phase, Phase::Attack);
        // No press while attacking; a hit plays the follow-up.
        assert_eq!(r.press(&set, button::SQUARE, false, |_| true), None);
        assert_eq!(r.hit(&set), Some(Cue::Clip { def: 1, clip: 8244 }));
        // The follow-up ends after the clip time, then a Square within the window continues: only item 44 owned.
        assert_eq!(r.update(&set, 0.5, 0.5), None);
        assert_eq!(r.phase, Phase::Recovery);
        assert_eq!(r.press(&set, button::SQUARE, false, |n| n == 44), Some(Cue::Clip { def: 2, clip: 8240 }));
        // A Circle definition without clips is skipped.
        assert_eq!(set.choose(button::CIRCLE, |_| true), None);
        // After the attack and the 0.35 s window the combo ends.
        r.update(&set, 0.5, 0.5);
        assert_eq!(r.update(&set, 0.3, 0.5), None);
        assert_eq!(r.update(&set, 0.1, 0.5), Some(Cue::Ended));
        assert!(!r.active());
    }
}
