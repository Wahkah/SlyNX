//! Sly 2 pickups (specs/sly2/pickpocket-and-pickups.md 3 to 6): coins (class 91), task items (92), medicine (93),
//! loot items (94) and clue bottles (95).
//!
//! A pickup is collected when the played character's centre comes within its collect radius (3.3; after the
//! 300-unit rejection): a coin gives one coin (the gang's purse, `World::shared.picked`) and raises script event 5;
//! a task item raises event 5 and is listed in `World::shared.task_items` for the job system; medicine is offered to
//! the played character (`World::shared.medicine`), which takes it unless its health and power are both full (the
//! gang system decides and removes it); a loot item goes to the purse as its prize; a clue bottle sets bit *n* (its
//! number among the world's bottles, in load order) of the level's bottle record and is gone for good.
//!
//! Placed pickups do not move or expire here: launching, bouncing and the rest time apply to dropped pickups, which
//! the gang system gives out (`gang::pocket::give`).

use alloc::vec::Vec;

use super::{ClassBehavior, Cx};
use crate::gang::Entry;
use crate::object::Handle;
use crate::world::World;

/// Classes.
pub mod class {
    /// Coin.
    pub const COIN: i16 = 91;
    /// Task item (keys, key cards, schedules, radio transmitters).
    pub const TASK_ITEM: i16 = 92;
    /// Medicine.
    pub const MEDICINE: i16 = 93;
    /// Shown pocket prize (loot item).
    pub const LOOT: i16 = 94;
    /// Clue bottle.
    pub const CLUE: i16 = 95;
}

/// Script event raised when a pickup is collected.
pub const COLLECTED: u32 = 5;
/// Pickups farther than this from the character are not tested (`0x2C0D7C`).
pub const REJECT: f32 = 300.0;
/// The coin's collect radius when its options give none. *Placeholder*: 3.3 says "from its options".
pub const COIN_RADIUS: f32 = 50.0;

/// What a pickup is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A coin.
    Coin,
    /// A task item.
    TaskItem,
    /// Medicine.
    Medicine,
    /// A loot item (its prize kind).
    Loot(i32),
    /// A clue bottle and its number in the world.
    Clue(u32),
}

impl Kind {
    /// Collect radius (3.3).
    pub fn radius(self) -> f32 {
        match self {
            Kind::Coin => COIN_RADIUS,
            Kind::TaskItem => 35.0,
            Kind::Medicine => 75.0,
            Kind::Loot(_) => 40.0,
            Kind::Clue(_) => 50.0,
        }
    }
}

/// A Sly 2 pickup.
#[derive(Clone, Debug, PartialEq)]
pub struct Sly2Pickup {
    /// What it is.
    pub kind: Kind,
    /// Collected.
    pub collected: bool,
}

impl Sly2Pickup {
    /// The pickup for object `h` of class `class`.
    pub fn new(world: &World, h: Handle, class: i16) -> Option<Sly2Pickup> {
        let kind = match class {
            class::COIN => Kind::Coin,
            class::TASK_ITEM => Kind::TaskItem,
            class::MEDICINE => Kind::Medicine,
            class::LOOT => Kind::Loot(world.options(h).and_then(|o| o.i32(907)).unwrap_or(0)),
            class::CLUE => {
                let n = world.slots_of_class(class::CLUE).iter().position(|&s| s == h.index()).unwrap_or(0);
                Kind::Clue(n as u32)
            }
            _ => return None,
        };
        Some(Sly2Pickup { kind, collected: false })
    }
}

impl ClassBehavior for Sly2Pickup {
    fn ticks(&self) -> bool {
        !self.collected
    }

    fn post_load(&mut self, cx: &mut Cx<'_>) {
        // A bottle collected before is gone for good.
        if let Kind::Clue(n) = self.kind
            && n < 64
            && cx.progress.level_ref(cx.world.info.id).is_some_and(|l| l.bottles & (1 << n) != 0)
        {
            self.collected = true;
            cx.world.set_in_world(cx.me, false);
        }
    }

    fn update(&mut self, cx: &mut Cx<'_>) {
        if self.collected || !cx.world.in_world(cx.me) {
            return;
        }
        let Some(player) = cx.world.player else { return };
        let Some(p) = cx.world.position(player) else { return };
        let c = [p[0], p[1], p[2] + super::sly2_sensors::CHARACTER_CENTRE];
        let me = cx.my_position();
        let d = super::dist_sq(c, me);
        let r = self.kind.radius() + super::sly2_sensors::CHARACTER_REACH;
        if d > REJECT * REJECT || d > r * r {
            return;
        }
        match self.kind {
            Kind::Medicine => {
                // The played character decides (it refuses when health and power are full).
                if !cx.world.shared.medicine.contains(&cx.me) {
                    cx.world.shared.medicine.push(cx.me);
                }
                return;
            }
            Kind::Coin => cx.world.shared.picked.push(Entry::Coins(1)),
            Kind::Loot(k) => cx.world.shared.picked.push(Entry::Prize(k)),
            Kind::TaskItem => cx.world.shared.task_items.push(cx.me),
            Kind::Clue(n) => {
                if n < 64 {
                    cx.progress.level(cx.world.info.id).bottles |= 1 << n;
                }
                let at = me;
                super::sly2::make_noise(cx.world, 706, at, Some(player), None, cx.time);
            }
        }
        self.collected = true;
        cx.raise(COLLECTED, Vec::new());
        cx.world.set_in_world(cx.me, false);
    }
}
