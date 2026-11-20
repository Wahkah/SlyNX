//! The Sly 2 clue bottles' record and the vault (class 78; specs/sly2/pickpocket-and-pickups.md 6 and 6.1, saving.md
//! 3, dialogue-and-voice.md 2.1).
//!
//! - **The records** live in the save progress (`game::sly2_jobs`): a level's 32 clue-bottle flags (`afClue`) and its
//!   vault flag (`fVault`), under the level's save path (the level table's row, disc.md 6). The bottles are all in the
//!   episode's hub, so the episode's count is the hub's `afClue`.
//! - **The vault's options** (6.1) are set by the vault package's own script on load, per episode: 1439 the
//!   combination (a three-digit integer, class default 123), 1440 the step-up line, 1442 the reward item, 1443 the
//!   open line; 1441 (on the body) is the missing-bottles line; 1444 is the "use the vault" method. They are read when
//!   used, after the scripts ran.
//! - **Using it**: Cross near it (or option 1444) with all 30 of the episode's bottles steps up to the dial (state
//!   1), then dials (2); without them the missing-bottles line is asked for. The left stick selects the nearest of
//!   nine keys laid out as a phone keypad ([`KEYS`]), the current key's distance counted at 0.7 times; Cross or Square
//!   enters the selected digit; after each entry the last three digits are compared with the combination: equal is
//!   the right combination (3), then open (5: the reward item owned, the level's vault flag set, script event 2);
//!   Triangle or Circle gives up (4, then idle). Each state from 1 to 5 starts its sequence (1616 to 1620, the
//!   played character as starter) through `World::shared.sequence_requests`, and is reported in
//!   `World::shared.vault_events` (lines and the dial's screen).
//!
//! *Placeholder*: the reach (250); the per-digit sounds of the package's scripts are theirs.

use alloc::vec::Vec;

use super::sly2_commands::SequenceRequest;
use super::{ClassBehavior, Cx};
use crate::game::sly2_jobs;
use crate::object::{Arg, Handle};
use crate::world::World;

/// The vault class.
pub const VAULT_CLASS: i16 = 78;
/// Clue bottles per episode.
pub const BOTTLES: u32 = 30;
/// The save name of a level's clue-bottle flags.
pub const CLUES: &str = "afClue";
/// The save name of a level's vault flag.
pub const VAULT_FLAG: &str = "fVault";
/// How near the played character must be to use the vault. *Placeholder*.
pub const REACH: f32 = 250.0;
/// The dial's keys 1 to 9 by stick direction (6.1; the executable's table at `0x2C0FF0`).
pub const KEYS: [[f32; 2]; 9] =
    [[-0.7, 0.7], [0.0, 0.8], [0.7, 0.7], [-0.8, 0.0], [0.0, 0.0], [0.8, 0.0], [-0.7, -0.7], [0.0, -0.8], [0.7, -0.7]];
/// The current key's distance counts this much (the selection does not flicker).
pub const KEEP: f32 = 0.7;
/// The first state's sequence (states 1 to 5 start 1616 to 1620).
pub const FIRST_SEQUENCE: i32 = 1616;
/// Script event raised when the vault opens.
pub const EVENT_OPENED: u32 = 2;

/// The vault's options (6.1).
pub mod opt {
    /// The combination (a three-digit integer).
    pub const COMBINATION: u16 = 1439;
    /// The step-up line.
    pub const STEP_UP_LINE: u16 = 1440;
    /// The missing-bottles line.
    pub const MISSING_LINE: u16 = 1441;
    /// The reward item record.
    pub const REWARD: u16 = 1442;
    /// The open line.
    pub const OPEN_LINE: u16 = 1443;
    /// "Use the vault" (a method).
    pub const USE: u16 = 1444;
}
/// The class default combination.
pub const DEFAULT_COMBINATION: i32 = 123;

/// The save path of `name` under level row `row`.
fn level_key(world: &mut World, row: usize, name: &str) -> Option<Vec<u16>> {
    let mut path = world.sly2.as_deref()?.header.level_rows.get(row)?.save_path.clone();
    let id = sly2_jobs::with(world, |j, _| j.progress.tree.name_id(name)).flatten()?;
    path.push(id);
    Some(path)
}

/// The world's level row (the job system's, else the world's index).
fn row_of(world: &mut World) -> usize {
    sly2_jobs::with(world, |j, _| j.row).filter(|&r| r >= 0).map_or(world.info.index as usize, |r| r as usize)
}

/// The hub row of the world's episode.
pub fn hub_row(world: &mut World) -> Option<usize> {
    let row = row_of(world);
    let rows = &world.sly2.as_deref()?.header.level_rows;
    let episode = rows.get(row)?.episode;
    rows.iter().position(|r| r.hub && r.episode == episode)
}

/// The episode's clue-bottle flags (bit *n*: bottle *n*), from the hub's record.
pub fn clue_bits(world: &mut World) -> u32 {
    let Some(key) = hub_row(world).and_then(|r| level_key(world, r, CLUES)) else { return 0 };
    sly2_jobs::with(world, |j, _| {
        (0..32).filter(|&n| j.progress.get(&key, Some(n)).is_some_and(|v| v.as_i32() != 0)).fold(0, |a, n| a | 1 << n)
    })
    .unwrap_or(0)
}

/// Records bottle `n` of the world's level as collected (false without a save record).
pub fn set_clue(world: &mut World, n: u32) -> bool {
    let row = row_of(world);
    let Some(key) = level_key(world, row, CLUES) else { return false };
    sly2_jobs::with(world, |j, _| j.progress.set(&key, Some(n as usize), save::sly2::SaveValue::Flag(true)))
        .unwrap_or(false)
}

/// Whether bottle `n` of the world's level was collected.
pub fn clue_collected(world: &mut World, n: u32) -> bool {
    let row = row_of(world);
    let Some(key) = level_key(world, row, CLUES) else { return false };
    sly2_jobs::with(world, |j, _| j.progress.get(&key, Some(n as usize)).is_some_and(|v| v.as_i32() != 0))
        .unwrap_or(false)
}

/// The world's level vault flag.
pub fn vault_open(world: &mut World) -> bool {
    let row = row_of(world);
    let Some(key) = level_key(world, row, VAULT_FLAG) else { return false };
    sly2_jobs::with(world, |j, _| j.progress.get(&key, None).is_some_and(|v| v.as_i32() != 0)).unwrap_or(false)
}

/// Sets the world's level vault flag.
pub fn set_vault_open(world: &mut World) -> bool {
    let row = row_of(world);
    let Some(key) = level_key(world, row, VAULT_FLAG) else { return false };
    sly2_jobs::with(world, |j, _| j.progress.set(&key, None, save::sly2::SaveValue::Flag(true))).unwrap_or(false)
}

/// The key (1 to 9) the stick selects, from the current one: the nearest by [`KEYS`], the current key's distance
/// counted at [`KEEP`] times.
pub fn select_key(stick: [f32; 2], current: u8) -> u8 {
    let mut best = (f32::INFINITY, current);
    for (i, k) in KEYS.iter().enumerate() {
        let key = i as u8 + 1;
        let (dx, dy) = (stick[0] - k[0], stick[1] - k[1]);
        let mut d = crate::math::sqrt(dx * dx + dy * dy);
        if key == current {
            d *= KEEP;
        }
        if d < best.0 {
            best = (d, key);
        }
    }
    best.1
}

/// The three digits of a combination.
pub fn digits(n: i32) -> [u8; 3] {
    let n = n.rem_euclid(1000);
    [(n / 100) as u8, (n / 10 % 10) as u8, (n % 10) as u8]
}

/// The vault's state (6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VaultState {
    /// Idle (0).
    #[default]
    Idle,
    /// Stepping up to the dial (1).
    SteppingUp,
    /// Dialing (2).
    Dialing,
    /// The right combination (3).
    Right,
    /// Gave up (4).
    GaveUp,
    /// Open (5): reward given, vault flag set.
    Open,
}

impl VaultState {
    /// The state's number.
    pub fn number(self) -> i32 {
        match self {
            VaultState::Idle => 0,
            VaultState::SteppingUp => 1,
            VaultState::Dialing => 2,
            VaultState::Right => 3,
            VaultState::GaveUp => 4,
            VaultState::Open => 5,
        }
    }
}

/// A change the session acts on (lines, the dial's screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VaultEvent {
    /// The vault went to this state.
    State(Handle, VaultState),
    /// Used without every bottle: the bottle count and the missing-bottles line (option 1441).
    Bottles(Handle, u32, Option<i32>),
    /// A digit was entered.
    Digit(Handle, u8),
}

/// The vault (class 78).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sly2Vault {
    /// Its state.
    pub state: VaultState,
    /// The key selected (1 to 9).
    pub key: u8,
    /// The digits entered, last three kept (oldest first).
    pub entered: [u8; 3],
    /// How many digits were entered since the dial came up.
    pub count: u32,
    /// Whether the saved flag was read.
    checked: bool,
    /// Option 1444 was called.
    used: bool,
}

impl Sly2Vault {
    fn go(&mut self, cx: &mut Cx<'_>, s: VaultState) {
        self.state = s;
        cx.world.shared.vault_events.push(VaultEvent::State(cx.me, s));
        let n = s.number();
        if n >= 1
            && let Some(seq) = cx.world.find_sly2_id(FIRST_SEQUENCE + n - 1, Some(cx.me))
        {
            let starter = cx.world.player;
            cx.world.shared.sequence_requests.push(SequenceRequest::Start { sequence: seq, starter });
        }
    }

    /// Enters a digit; returns whether the last three equal `combination`.
    pub fn enter(&mut self, digit: u8, combination: [u8; 3]) -> bool {
        self.entered = [self.entered[1], self.entered[2], digit];
        self.count += 1;
        self.count >= 3 && self.entered == combination
    }

    fn start(&mut self, cx: &mut Cx<'_>) {
        let bits = clue_bits(cx.world);
        if bits.count_ones() >= BOTTLES {
            self.go(cx, VaultState::SteppingUp);
        } else {
            let line = cx.world.options(cx.me).and_then(|o| o.i32(opt::MISSING_LINE));
            cx.world.shared.vault_events.push(VaultEvent::Bottles(cx.me, bits.count_ones(), line));
        }
    }
}

impl ClassBehavior for Sly2Vault {
    fn ticks(&self) -> bool {
        self.state != VaultState::Open
    }

    /// Option 1444: use the vault (a script starts it).
    fn action(&mut self, _world: &mut World, _me: Handle, id: u16, _args: &[Arg]) -> bool {
        if id != opt::USE {
            return false;
        }
        self.used = true;
        true
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if !self.checked {
            self.checked = true;
            if vault_open(cx.world) {
                self.state = VaultState::Open;
                return;
            }
        }
        let input = cx.world.shared.input;
        let any = |b: platform_api::Buttons| input.pressed & b.0 != 0;
        // Cross or Square enters; Triangle or Circle gives up (the Switch layout: B, Y; X, A).
        let enter = any(platform_api::Buttons::B) || any(platform_api::Buttons::Y);
        let give_up = any(platform_api::Buttons::X) || any(platform_api::Buttons::A);
        match self.state {
            VaultState::Idle => {
                let used = core::mem::take(&mut self.used);
                let near = cx
                    .world
                    .player
                    .and_then(|p| cx.world.position(p))
                    .is_some_and(|p| super::dist_sq(p, cx.my_position()) <= REACH * REACH);
                if used || (enter && near) {
                    self.start(cx);
                }
            }
            VaultState::SteppingUp => {
                self.key = 5;
                self.entered = [0; 3];
                self.count = 0;
                self.go(cx, VaultState::Dialing);
            }
            VaultState::Dialing => {
                self.key = select_key(input.left, if self.key == 0 { 5 } else { self.key });
                if give_up {
                    self.go(cx, VaultState::GaveUp);
                } else if enter {
                    let combination = digits(
                        cx.world.options(cx.me).and_then(|o| o.i32(opt::COMBINATION)).unwrap_or(DEFAULT_COMBINATION),
                    );
                    cx.world.shared.vault_events.push(VaultEvent::Digit(cx.me, self.key));
                    if self.enter(self.key, combination) {
                        self.go(cx, VaultState::Right);
                    }
                }
            }
            VaultState::Right => {
                // Open: the reward item, the vault flag, event 2.
                if let Some(item) = cx.world.options(cx.me).and_then(|o| o.i32(opt::REWARD)) {
                    sly2_jobs::with(cx.world, |j, _| sly2_jobs::shop::set_owned(&mut j.progress, item, true));
                }
                set_vault_open(cx.world);
                cx.raise(EVENT_OPENED, Vec::new());
                self.go(cx, VaultState::Open);
            }
            VaultState::GaveUp => self.go(cx, VaultState::Idle),
            VaultState::Open => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dial_selects_and_compares_the_last_three() {
        assert_eq!(select_key([0.0, 0.0], 5), 5);
        assert_eq!(select_key([-1.0, 1.0], 5), 1);
        assert_eq!(select_key([0.7, -0.7], 5), 9);
        // Halfway between 5 and 6, the current key holds.
        assert_eq!(select_key([0.42, 0.0], 5), 5);
        assert_eq!(select_key([0.42, 0.0], 6), 6);
        assert_eq!(digits(386), [3, 8, 6]);
        let mut v = Sly2Vault::default();
        let c = digits(231);
        assert!(!v.enter(9, c) && !v.enter(2, c) && !v.enter(3, c));
        assert!(v.enter(1, c), "the last three digits match");
    }
}
