//! The Sly 3 safe house and shop (specs/sly3/safehouse-and-shop.md), with Sly 2's behaviour.
//!
//! - **Safe house** (class 69, sections 1 and 2): option 1518 gives the selectable states at the start (Bentley 25,
//!   the computer 27, Sly 24 on the disc); left and right step through them, wrapping; scripts may set the list (at
//!   most 6). Option 1517, on the safe house and in its placement's option blocks, gives the actions: in a state,
//!   a button fires a target. States 24, 25 and 26 are Sly, Bentley and Murray (their object IDs); their action
//!   starts the hub's take-out script with that character ([`Pressed::TakeOut`]). State 27 is the computer; its
//!   action opens screen 1065, the shop ([`Pressed::OpenShop`]).
//! - **Shop** (section 3): the item table's records with an episode other than −1, from their episode on, not yet
//!   owned, in table order; buying needs the coins.
//!
//! The take-out script itself (command script 2061: tasks, the door, the walk out) needs the Sly 3 command runner
//! and is not run; the session switches to the character instead.

use alloc::vec::Vec;

use engine::gang::sly3::{Member, Sly3Item, Sly3ItemTable};
use engine::object::Handle;
use engine::world::World;

use super::flow::actions;

/// The safe-house class.
pub const SAFEHOUSE_CLASS: i16 = 69;
/// The shop screen object.
pub const SHOP_SCREEN: i32 = 1065;
/// The computer's state.
pub const COMPUTER: i32 = 27;
/// Cross.
pub const CROSS: i32 = 0x40;
/// Most selectable states.
pub const MAX_STATES: usize = 6;

/// Option IDs.
pub mod opt {
    /// The state machine that seats the characters.
    pub const SEATS: u16 = 1516;
    /// An action: (state, buttons, target).
    pub const ACTION: u16 = 1517;
    /// The selectable states.
    pub const STATES: u16 = 1518;
}

/// The character a state seats.
pub fn member_of_state(state: i32) -> Option<Member> {
    match state {
        24 => Some(Member::Sly),
        25 => Some(Member::Bentley),
        26 => Some(Member::Murray),
        _ => None,
    }
}

/// A safe-house action (option 1517).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    /// The state it belongs to, the buttons and the target object.
    pub state: i32,
    /// See [`Action::state`].
    pub buttons: i32,
    /// See [`Action::state`].
    pub target: i32,
}

/// What pressing a button in the safe house did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressed {
    /// Open the shop.
    OpenShop,
    /// Take this character out into the hub (the target script).
    TakeOut { member: Member, script: i32 },
    /// Another target.
    Target(i32),
}

/// The safe house of a hub.
#[derive(Clone, Debug, PartialEq)]
pub struct SafeHouse {
    /// The object.
    pub handle: Option<Handle>,
    /// The selectable states and the one shown.
    pub states: Vec<i32>,
    /// See [`SafeHouse::states`].
    pub shown: usize,
    /// The actions.
    pub actions: Vec<Action>,
}

impl SafeHouse {
    /// The world's safe house, if it has one.
    pub fn from_world(world: &World) -> Option<SafeHouse> {
        let h = world.handles().find(|&h| world.class(h) == Some(SAFEHOUSE_CLASS))?;
        let states: Vec<i32> = actions(world, h, opt::STATES).into_iter().flatten().take(MAX_STATES).collect();
        let mut list = Vec::new();
        let mut at = Some(h);
        while let Some(o) = at {
            for v in actions(world, o, opt::ACTION) {
                if let [state, buttons, target, ..] = v[..] {
                    let a = Action { state, buttons, target };
                    if !list.contains(&a) {
                        list.push(a);
                    }
                }
            }
            at = world.parent(o);
        }
        Some(SafeHouse { handle: Some(h), states, shown: 0, actions: list })
    }

    /// The state shown.
    pub fn state(&self) -> Option<i32> {
        self.states.get(self.shown).copied()
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

    /// Buttons pressed this frame (`Buttons` bits as the game's: Cross `0x40`): the action of the shown state.
    pub fn press(&self, buttons: i32) -> Option<Pressed> {
        let s = self.state()?;
        let a = self.actions.iter().find(|a| a.state == s && a.buttons & buttons != 0)?;
        Some(if a.target == SHOP_SCREEN {
            Pressed::OpenShop
        } else if let Some(member) = member_of_state(s) {
            Pressed::TakeOut { member, script: a.target }
        } else {
            Pressed::Target(a.target)
        })
    }
}

/// The shop (safehouse-and-shop.md 3).
#[derive(Clone, Debug, PartialEq)]
pub struct Shop {
    /// What is on sale: (item record, the record).
    pub items: Vec<(usize, Sly3Item)>,
}

/// Why a purchase failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuyError {
    /// Not on sale.
    NotListed,
    /// Coins short by this many.
    Short(i32),
}

impl Shop {
    /// The shop in `episode`: sold records from their episode on, not in `owned`, in table order.
    pub fn new(table: &Sly3ItemTable, episode: i32, owned: &[usize]) -> Shop {
        let items = table
            .items
            .iter()
            .copied()
            .enumerate()
            .filter(|(i, it)| it.episode != -1 && it.episode <= episode && !owned.contains(i))
            .collect();
        Shop { items }
    }

    /// Buys record `item` with `coins`: the coins left, and the item leaves the list.
    pub fn buy(&mut self, item: usize, coins: i32) -> Result<i32, BuyError> {
        let k = self.items.iter().position(|(i, _)| *i == item).ok_or(BuyError::NotListed)?;
        let price = self.items[k].1.price;
        if coins < price {
            return Err(BuyError::Short(price - coins));
        }
        self.items.remove(k);
        Ok(coins - price)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(episode: i32, price: i32) -> Sly3Item {
        Sly3Item {
            button: 2,
            owner: 24,
            price,
            cost: 0,
            cost2: 0,
            episode,
            tier: 15,
            name: 0,
            description: 0,
            usage: 0,
            effect: 0,
            pairs: [0; 4],
        }
    }

    #[test]
    fn shop_lists_and_sells() {
        let t = Sly3ItemTable { items: alloc::vec![item(-1, 0), item(3, 300), item(4, 500), item(3, 200)] };
        let mut s = Shop::new(&t, 3, &[3]);
        assert_eq!(s.items.iter().map(|x| x.0).collect::<Vec<_>>(), [1]);
        assert_eq!(s.buy(1, 250), Err(BuyError::Short(50)));
        assert_eq!(s.buy(1, 1000), Ok(700));
        assert_eq!(s.buy(1, 1000), Err(BuyError::NotListed));
        assert_eq!(Shop::new(&t, 4, &[]).items.len(), 3);
    }

    #[test]
    fn states_wrap_and_actions_fire() {
        let mut h = SafeHouse {
            handle: None,
            states: alloc::vec![25, 27, 24],
            shown: 0,
            actions: alloc::vec![
                Action { state: 27, buttons: CROSS, target: SHOP_SCREEN },
                Action { state: 24, buttons: CROSS, target: 44600 },
                Action { state: 25, buttons: CROSS, target: 44602 },
            ],
        };
        assert_eq!(h.press(CROSS), Some(Pressed::TakeOut { member: Member::Bentley, script: 44602 }));
        h.step(-1);
        assert_eq!(h.state(), Some(24));
        h.step(-1);
        assert_eq!(h.press(CROSS), Some(Pressed::OpenShop));
        assert_eq!(h.press(0x10), None);
        h.set_states(&[24, 25, 26, 27, 1, 2, 3]);
        assert_eq!((h.states.len(), h.state()), (6, Some(27)));
    }
}
