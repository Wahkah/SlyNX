//! The vault (class 23, pickups-and-progress.md 4.2) and lock groups (classes 92 and 93, level-flow.md 7.1).
//!
//! Walking up to a vault before every clue bottle is collected plays a hint dialogue chosen by progress (option
//! 1047: up to 4 dialogues per hint kind 0 to 4). With all bottles collected the player turns its three dials (right
//! or Circle: next dial; left or Square: back; up or Triangle and down or Cross: change the digit); when they match
//! the combination (option 1046, hundreds, tens, units) the vault opens: the level's vault flag, a vault counted for
//! the episode, event 2 and an autosave.

use alloc::vec::Vec;

use platform_api::Buttons;

use super::{ClassBehavior, Cx, dist_sq};
use crate::game::progress::level_flag;
use crate::object::class::id;
use crate::object::{Arg, OptionValue};
use crate::world::{Notice, Request};

/// Vault options.
pub mod opt {
    /// The combination (hundreds, tens, units). The placement's own value overrides the template's (the loader
    /// stores template options first and lookups take the last value); every world's is tabled (Paris: 937).
    pub const COMBINATION: u16 = 1046;
    /// Hint dialogues: (kind, dialogue OID) typed arguments.
    pub const HINTS: u16 = 1047;
    /// Lock group: key slots.
    pub const SLOTS: u16 = 1000;
    /// Lock group: index of its world-state bit (`2 << index`).
    pub const INDEX: u16 = 1002;
}

/// The vault is usable within this distance of its marker object (pickups-and-progress.md 4.2).
pub const REACH: f32 = 250.0;
/// The vault's marker object (OID; searched below the vault, else the vault's own position counts).
pub const MARKER_OID: i16 = 892;
/// Where the vault's marker (OID 892 below it) is, else the vault itself.
fn marker_position(cx: &Cx<'_>) -> [f32; 3] {
    cx.world
        .find_nearest(MARKER_OID, Some(cx.me))
        .and_then(|h| cx.world.position(h))
        .unwrap_or_else(|| cx.my_position())
}

/// Dial state OIDs for digits 0 to 9.
pub const DIAL_STATE_ZERO: i16 = 912;
/// Lock states (level-flow.md 7.1).
pub const LOCK_CLOSED: i16 = 558;
/// See [`LOCK_CLOSED`].
pub const LOCK_OPEN: i16 = 559;
/// See [`LOCK_CLOSED`].
pub const LOCK_PARTIAL: i16 = 560;

/// A vault's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultState {
    /// Closed, player away.
    Idle,
    /// Hint given for this approach.
    Hinted,
    /// The player is turning the dials.
    Dialing,
    /// Open.
    Open,
}

/// A vault.
#[derive(Clone, Debug, PartialEq)]
pub struct Vault {
    /// State.
    pub state: VaultState,
    /// The three dials.
    pub digits: [u8; 3],
    /// The dial being turned.
    pub cursor: usize,
}

impl Vault {
    /// A closed vault.
    pub fn new() -> Vault {
        Vault { state: VaultState::Idle, digits: [0; 3], cursor: 0 }
    }

    /// The combination as three digits.
    pub fn combination(n: i32) -> [u8; 3] {
        let n = n.rem_euclid(1000);
        [(n / 100) as u8, (n / 10 % 10) as u8, (n % 10) as u8]
    }

    /// The hint kind for collected/total bottles: under half 0, under 80 % 1, all but one 2, all 3.
    pub fn hint_kind(collected: u32, total: u32) -> i32 {
        if total > 0 && collected >= total {
            3
        } else if total > 0 && collected + 1 == total {
            2
        } else if collected * 5 < total * 4 && collected * 2 >= total {
            1
        } else if collected * 2 < total {
            0
        } else {
            1
        }
    }
}

impl Default for Vault {
    fn default() -> Self {
        Vault::new()
    }
}

impl ClassBehavior for Vault {
    fn ticks(&self) -> bool {
        self.state != VaultState::Open
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        if cx.progress.has_flag(cx.world.info.id, level_flag::VAULT) {
            self.state = VaultState::Open;
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let near = cx.player_position().is_some_and(|p| dist_sq(p, marker_position(cx)) <= REACH * REACH);
        cx.world.take_inbox(cx.me.index());
        if !near {
            if self.state != VaultState::Open {
                self.state = VaultState::Idle;
            }
            return;
        }
        let world_id = cx.world.info.id;
        let total = cx.world.slots_of_class(id::CLUE).len() as u32;
        let collected = cx.progress.level_ref(world_id).map_or(0, |l| l.bottles.count_ones());
        match self.state {
            VaultState::Idle if collected < total => {
                let kind = Vault::hint_kind(collected, total);
                let hints: Vec<i16> = cx
                    .world
                    .options(cx.me)
                    .map(|o| {
                        o.all(opt::HINTS)
                            .filter_map(|v| {
                                let OptionValue::Args(a) = v else { return None };
                                let ints: Vec<i32> = a
                                    .iter()
                                    .filter_map(|x| match x {
                                        Arg::Int(n) => Some(*n),
                                        Arg::Short(_, n) => Some(*n as i32),
                                        _ => None,
                                    })
                                    .collect();
                                if ints.first() == Some(&kind) { ints.get(1).map(|&d| d as i16) } else { None }
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(&d) = hints.first() {
                    cx.world.request(Request::Dialogue(d));
                }
                self.state = VaultState::Hinted;
            }
            VaultState::Idle => self.state = VaultState::Dialing,
            VaultState::Hinted if collected >= total => self.state = VaultState::Dialing,
            VaultState::Dialing => {
                let i = cx.world.shared.input;
                let any = |b: Buttons| i.pressed & b.0 != 0;
                if any(Buttons::RIGHT) || any(Buttons::A) {
                    self.cursor = (self.cursor + 1).min(2);
                }
                if any(Buttons::LEFT) || any(Buttons::Y) {
                    self.cursor = self.cursor.saturating_sub(1);
                }
                if any(Buttons::UP) || any(Buttons::X) {
                    self.digits[self.cursor] = (self.digits[self.cursor] + 1) % 10;
                }
                if any(Buttons::DOWN) || any(Buttons::B) {
                    self.digits[self.cursor] = (self.digits[self.cursor] + 9) % 10;
                }
                let combo =
                    Vault::combination(cx.world.options(cx.me).and_then(|o| o.i32(opt::COMBINATION)).unwrap_or(0));
                if self.digits == combo {
                    self.state = VaultState::Open;
                    let l = cx.progress.level(world_id);
                    if l.flags & level_flag::VAULT == 0 {
                        l.flags |= level_flag::VAULT;
                        *cx.progress.vaults.entry(world_id >> 8).or_insert(0) += 1;
                    }
                    cx.raise(script::event::TRIGGERED, Vec::new());
                    cx.world.request(Request::Autosave);
                }
            }
            _ => {}
        }
    }
}

/// A lock group (class 93): opens when the episode has as many keys as it has slots.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LockGroup {
    /// Open.
    pub open: bool,
}

impl LockGroup {
    fn bit(cx: &Cx<'_>) -> u32 {
        2 << cx.world.options(cx.me).and_then(|o| o.i32(opt::INDEX)).unwrap_or(0).clamp(0, 30)
    }

    fn goal(cx: &mut Cx<'_>, g: i16) {
        for m in cx.world.children_of_class(cx.me, id::STATE_MACHINE) {
            cx.world.set_option(m, 516, OptionValue::Short(g));
        }
    }
}

impl ClassBehavior for LockGroup {
    fn ticks(&self) -> bool {
        !self.open
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        let episode = cx.world.info.id >> 8;
        if cx.progress.episode_flags.get(&episode).is_some_and(|f| f & Self::bit(cx) != 0) {
            self.open = true;
            Self::goal(cx, LOCK_OPEN);
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        let used = cx.world.take_inbox(cx.me.index()).iter().any(|n| matches!(n, Notice::Touched(_) | Notice::Hit(_)));
        let near = cx.player_position().is_some_and(|p| dist_sq(p, marker_position(cx)) <= REACH * REACH);
        if !(used || near) {
            return;
        }
        let episode = cx.world.info.id >> 8;
        let slots = cx.world.options(cx.me).and_then(|o| o.i32(opt::SLOTS)).unwrap_or(1).max(1) as u32;
        let keys = cx.progress.keys.get(&episode).copied().unwrap_or(0);
        if keys >= slots {
            self.open = true;
            let bit = Self::bit(cx);
            *cx.progress.episode_flags.entry(episode).or_insert(0) |= bit;
            Self::goal(cx, LOCK_OPEN);
            cx.raise(script::event::TRIGGERED, Vec::new());
        } else if keys > 0 {
            Self::goal(cx, LOCK_PARTIAL);
        }
    }
}
