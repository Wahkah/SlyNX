//! The Sly 2 safe house (class 70; specs/sly2/safehouse-and-shop.md 1 and 2, dialogue-and-voice.md 2.1).
//!
//! - **Options**: 1337 the state machine that seats the characters, 1339 the selectable states (7 Sly, 8 Bentley,
//!   9 Murray, 10 the computer; at most 6, scripts may set them), 1338 the actions (state, buttons, target), on the
//!   safe house and in its placements' option blocks (`engine::behavior::sly2_commands::safe_house_actions`).
//! - **Entering** ([`super::Sly2Session::enter_safe_house`], also when a script sets option 884 to 1 on it, as the
//!   hubs' `_ext_rig` modules do on arrival at point 393; 2.1): the safe house becomes the current player (no character
//!   takes damage, `World::shared.safe_house`); every character's health and gadget power are refilled (the gang);
//!   the shown state is the current character's, else the first; script event 5 on the safe house and task event 26.
//! - **Choosing**: left and right step through the selectable states, wrapping.
//! - **Acting**: Cross fires the shown state's action (`sly2_commands::safe_house_action`): the computer's target
//!   (888) opens the shop; a character's state takes that character out (the session leaves the safe house and
//!   switches to it); a sequence target starts with the character.
//! - **Leaving**: script event 6 on the safe house and task event 27.
//!
//! Not done: the job that ends on entering (the job system's, unless marked "keep"), removing the gadgets' objects,
//! the seating state machine's goal (Sly 2 state machines are not run), and the take-out scripts the characters'
//! actions name (their command runs need the Sly 2 command runner); the session switches to the character instead.

use alloc::vec::Vec;

use engine::gang::Who;
use engine::object::Handle;
use engine::world::World;

/// The safe-house class.
pub const SAFE_HOUSE_CLASS: i16 = 70;
/// The shop's screen object (the computer's action target).
pub const SHOP_SCREEN: i32 = 888;
/// Cross, as the actions' button bits.
pub const CROSS: u32 = 0x40;
/// Most selectable states.
pub const MAX_STATES: usize = 6;
/// The computer's state.
pub const COMPUTER: i32 = 10;
/// Script event raised on entering.
pub const EVENT_ENTER: u32 = 5;
/// Script event raised on leaving.
pub const EVENT_LEAVE: u32 = 6;
/// Task event on entering.
pub const TASK_ENTER: u32 = 26;
/// Task event on leaving.
pub const TASK_LEAVE: u32 = 27;

/// Option IDs.
pub mod opt {
    /// The state machine that seats the characters.
    pub const SEATS: u16 = 1337;
    /// An action: (state, buttons, target).
    pub const ACTION: u16 = 1338;
    /// The selectable states.
    pub const STATES: u16 = 1339;
    /// "Enable as current player": scripts set it to 1 on arrival at the hub's point 393 (2.1).
    pub const ENABLE: u16 = 884;
}

/// The character a state seats.
pub fn who_of_state(state: i32) -> Option<Who> {
    match state {
        7 => Some(Who::Sly),
        8 => Some(Who::Bentley),
        9 => Some(Who::Murray),
        _ => None,
    }
}

/// The state that seats a character.
pub fn state_of(who: Who) -> i32 {
    match who {
        Who::Sly => 7,
        Who::Bentley => 8,
        Who::Murray => 9,
    }
}

/// The integer arguments of an entry.
fn ints(args: &[engine::object::Arg]) -> Vec<i32> {
    args.iter()
        .filter_map(|a| match *a {
            engine::object::Arg::Int(n) => Some(n),
            engine::object::Arg::Short(_, n) => Some(i32::from(n)),
            _ => None,
        })
        .collect()
}

/// What Cross did in the safe house.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressed {
    /// Open the shop.
    OpenShop,
    /// Take this character out into the hub.
    TakeOut(Who),
    /// Another target (a sequence was started with the character).
    Target(Handle),
}

/// A hub's safe house.
#[derive(Clone, Debug, PartialEq)]
pub struct SafeHouse {
    /// The object.
    pub handle: Handle,
    /// The selectable states.
    pub states: Vec<i32>,
    /// The index of the one shown.
    pub shown: usize,
}

impl SafeHouse {
    /// The world's safe house, if it has one.
    pub fn from_world(world: &World) -> Option<SafeHouse> {
        let handle = world.handles().find(|&h| world.class(h) == Some(SAFE_HOUSE_CLASS))?;
        let slot = world.slot(handle)?;
        let mut states: Vec<i32> = world
            .sly2
            .as_deref()
            .and_then(|l| l.actions.iter().find(|(s, id, _)| *s == slot && *id == opt::STATES))
            .map(|(_, _, a)| ints(a))
            .unwrap_or_default();
        if states.is_empty()
            && let Some(engine::object::OptionValue::Args(a)) = world.options(handle).and_then(|o| o.get(opt::STATES))
        {
            states = ints(a);
        }
        states.truncate(MAX_STATES);
        Some(SafeHouse { handle, states, shown: 0 })
    }

    /// The state shown.
    pub fn state(&self) -> Option<i32> {
        self.states.get(self.shown).copied()
    }

    /// Shows `state` when it is selectable, else the first.
    pub fn show(&mut self, state: i32) {
        self.shown = self.states.iter().position(|&s| s == state).unwrap_or(0);
    }

    /// Steps the shown state by `by` (left −1, right +1), wrapping.
    pub fn step(&mut self, by: i32) {
        let n = self.states.len() as i32;
        if n > 0 {
            self.shown = (self.shown as i32 + by).rem_euclid(n) as usize;
        }
    }

    /// Sets the selectable states (a script's call; at most 6), keeping the shown state when it stays.
    pub fn set_states(&mut self, states: &[i32]) {
        let shown = self.state();
        self.states = states.iter().copied().take(MAX_STATES).collect();
        self.shown = shown.and_then(|s| self.states.iter().position(|&x| x == s)).unwrap_or(0);
    }

    /// Cross on the shown state, with `character` the object of the character it seats (if any).
    pub fn press(&self, world: &mut World, character: Option<Handle>) -> Option<Pressed> {
        let state = self.state()?;
        let target = engine::behavior::sly2_commands::safe_house_action(world, self.handle, state, CROSS, character)?;
        Some(if world.sly2_id(target) == Some(SHOP_SCREEN) {
            Pressed::OpenShop
        } else if let Some(who) = who_of_state(state) {
            Pressed::TakeOut(who)
        } else {
            Pressed::Target(target)
        })
    }
}
