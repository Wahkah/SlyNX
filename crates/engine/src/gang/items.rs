//! The item table, gadget slots and power (characters.md 6, gadgets.md 1 to 3).

use alloc::vec::Vec;

use formats_sly2::exe::Executable;

use super::Who;

/// Virtual address of the item table.
pub const ITEM_TABLE_VA: u32 = 0x002B_CD00;
/// Records in the item table.
pub const ITEM_COUNT: usize = 43;

/// How an item is used (record field 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonUse {
    /// 2: pressed, fires once.
    Press,
    /// 1: held, runs while held.
    Hold,
    /// −1: passive, no button.
    Passive,
    /// Anything else (the empty records 0 to 3).
    None,
}

/// One item record (32 bytes: eight `s32`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    /// Button use code.
    pub button: i32,
    /// Owner code: 7 Sly, 8 Bentley, 9 Murray.
    pub owner: i32,
    /// Shop price.
    pub price: i32,
    /// Power cost per use.
    pub cost: i32,
    /// Episode it opens in (−1: not sold).
    pub episode: i32,
    /// Name string ID.
    pub name: i32,
    /// Description string ID.
    pub description: i32,
    /// Usage text string ID.
    pub usage: i32,
}

impl Item {
    /// The button use.
    pub fn button_use(&self) -> ButtonUse {
        match self.button {
            2 => ButtonUse::Press,
            1 => ButtonUse::Hold,
            -1 => ButtonUse::Passive,
            _ => ButtonUse::None,
        }
    }

    /// The character that owns it.
    pub fn owner(&self) -> Option<Who> {
        match self.owner {
            7 => Some(Who::Sly),
            8 => Some(Who::Bentley),
            9 => Some(Who::Murray),
            _ => None,
        }
    }
}

/// Item numbers used by the characters' rules.
pub mod id {
    /// Bentley's Hover Pack.
    pub const HOVER_PACK: u8 = 12;
    /// Bentley's Trigger Bomb.
    pub const TRIGGER_BOMB: u8 = 7;
    /// Bentley's Health Extractor.
    pub const HEALTH_EXTRACTOR: u8 = 11;
    /// Murray's Raging Inferno Flop.
    pub const INFERNO_FLOP: u8 = 22;
    /// Sly's Stealth Slide.
    pub const STEALTH_SLIDE: u8 = 25;
    /// Sly's Thief Reflexes.
    pub const THIEF_REFLEXES: u8 = 29;
    /// Sly's Shadow Power.
    pub const SHADOW_POWER: u8 = 40;
    /// Sly's Time Rush.
    pub const TIME_RUSH: u8 = 42;
    /// Long Toss (passive, every character's throw).
    pub const LONG_TOSS: u8 = 36;
    /// Items that play an animation first and pay at its activation event (gadgets.md 3).
    pub const PAY_AT_EVENT: [u8; 7] = [23, 14, 21, 15, 34, 35, 8];
    /// Pocket items, paid when thrown.
    pub const POCKET: [u8; 4] = [26, 37, 38, 41];
}

/// Behaviour codes for the Sly 3 items with no Sly 2 counterpart (Sly 3's other items use the number of the Sly 2
/// item whose rules carry over, specs/sly3/gadgets.md).
pub mod behaviour {
    /// Fishing Pole (Sly 3 item 7): a pocket item taken at range.
    pub const FISHING_POLE: u8 = 100;
    /// Grapple-Cam (13): a remote camera.
    pub const GRAPPLE_CAM: u8 = 101;
    /// Be The Ball (17): Murray rolls.
    pub const BE_THE_BALL: u8 = 102;
    /// Rocket Boots (35): fast movement while held.
    pub const ROCKET_BOOTS: u8 = 103;
    /// Treasure Map (36): shows the hub's treasure trail while held.
    pub const TREASURE_MAP: u8 = 104;
    /// Disguises (38, 39, 40): Venice, China, pirates.
    pub const DISGUISES: [u8; 3] = [105, 106, 107];
    /// Shadow Power level 2 (34): invisible and attacking.
    pub const SHADOW_POWER_2: u8 = 108;
    /// The attack upgrades (41 to 49): combo finishers.
    pub const ATTACK_UPGRADE: u8 = 109;
    /// The unused shield power-up (37).
    pub const SHIELD: u8 = 110;
    /// No behaviour (placeholders).
    pub const NONE: u8 = 0;
}

/// What a game adds to an item record: the behaviour the gang runs it with and a per-second drain for held items.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ItemExtra {
    /// Behaviour code: a Sly 2 item number, or one of [`behaviour`].
    pub behaviour: u8,
    /// Per-second power drain while held (Sly 3's field +16 [I]); 0 when the item has none of its own.
    pub drain: f32,
}

/// The item table.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemTable {
    /// The records, by item number.
    pub items: Vec<Item>,
    /// Per item, the behaviour and drain (empty for Sly 2: an item's behaviour is its own number).
    pub extra: Vec<ItemExtra>,
}

impl ItemTable {
    /// Reads the table from the player's executable, after its SHA-256 check.
    pub fn from_executable(exe: &Executable) -> formats::Result<ItemTable> {
        if !exe.is_supported() {
            return Err(formats::Error::UnknownExecutable);
        }
        let raw = exe.bytes_at(ITEM_TABLE_VA, ITEM_COUNT * 32)?;
        Ok(ItemTable::from_bytes(raw))
    }

    /// From raw records.
    pub fn from_bytes(raw: &[u8]) -> ItemTable {
        let items = raw
            .as_chunks::<32>()
            .0
            .iter()
            .map(|c| {
                let f = |i: usize| i32::from_le_bytes([c[i * 4], c[i * 4 + 1], c[i * 4 + 2], c[i * 4 + 3]]);
                Item {
                    button: f(0),
                    owner: f(1),
                    price: f(2),
                    cost: f(3),
                    episode: f(4),
                    name: f(5),
                    description: f(6),
                    usage: f(7),
                }
            })
            .collect();
        ItemTable { items, extra: Vec::new() }
    }

    /// An item.
    pub fn get(&self, n: u8) -> Option<&Item> {
        self.items.get(n as usize)
    }

    /// The behaviour item `n` runs with (its own number for Sly 2).
    pub fn behaviour(&self, n: u8) -> u8 {
        self.extra.get(n as usize).map_or(n, |e| e.behaviour)
    }

    /// The item number that runs behaviour `b` (`b` itself for Sly 2; `None` when no item does).
    pub fn actual(&self, b: u8) -> Option<u8> {
        if self.extra.is_empty() {
            return Some(b);
        }
        self.extra.iter().position(|e| e.behaviour == b && b != behaviour::NONE).map(|i| i as u8)
    }

    /// The table's own drain for item `n`, if it gives one.
    pub fn drain(&self, n: u8) -> Option<f32> {
        self.extra.get(n as usize).map(|e| e.drain).filter(|&d| d > 0.0)
    }

    /// Whether the table is a Sly 3 one (items carry behaviour codes).
    pub fn is_sly3(&self) -> bool {
        !self.extra.is_empty()
    }
}

/// Which items the gang owns (one flag per item for the whole gang; Sly 3 has 67 items).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Owned(pub u128);

impl Owned {
    /// Whether item `n` is owned.
    pub fn has(&self, n: u8) -> bool {
        n < 128 && self.0 & (1u128 << n) != 0
    }
    /// Marks item `n` owned.
    pub fn add(&mut self, n: u8) {
        if n < 128 {
            self.0 |= 1u128 << n;
        }
    }
    /// Removes item `n` (a Mega Jump request removes it).
    pub fn remove(&mut self, n: u8) {
        if n < 128 {
            self.0 &= !(1u128 << n);
        }
    }
}

/// A character's three gadget slots on L1, L2 and R2 (gadgets.md 2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slots(pub [Option<u8>; 3]);

impl Slots {
    /// Puts `item` in `slot` if it can go on a button (owned, button use press or hold, owned by `who`), clearing
    /// any other slot that held it. Returns whether it was placed.
    pub fn assign(&mut self, slot: usize, item: u8, who: Who, table: &ItemTable, owned: &Owned) -> bool {
        let Some(rec) = table.get(item) else { return false };
        if slot >= 3 || !owned.has(item) || !matches!(rec.button_use(), ButtonUse::Press | ButtonUse::Hold) {
            return false;
        }
        if rec.owner() != Some(who) {
            return false;
        }
        for s in self.0.iter_mut() {
            if *s == Some(item) {
                *s = None;
            }
        }
        self.0[slot] = Some(item);
        true
    }
}

/// A character's gadget power (gadgets.md 3): maximum 100, never below 0, drains accumulated as fractions and
/// taken in whole units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Power {
    /// Current power (whole units).
    pub value: f32,
    /// Maximum (100).
    pub max: f32,
    fraction: f32,
}

impl Power {
    /// A meter at `value` of `max`.
    pub fn new(value: f32, max: f32) -> Power {
        Power { value: value.clamp(0.0, max), max, fraction: 0.0 }
    }

    /// Pays `cost` if there is enough; returns whether it was paid.
    pub fn pay(&mut self, cost: f32) -> bool {
        if self.value < cost {
            return false;
        }
        self.value = (self.value - cost).max(0.0);
        true
    }

    /// Drains `rate` per second for `dt`, in whole units; returns false once power is 0.
    pub fn drain(&mut self, rate: f32, dt: f32) -> bool {
        self.fraction += rate * dt;
        while self.fraction >= 1.0 && self.value > 0.0 {
            self.fraction -= 1.0;
            self.value = (self.value - 1.0).max(0.0);
        }
        self.value > 0.0
    }

    /// Adds power, limited to the maximum.
    pub fn add(&mut self, amount: f32) {
        self.value = (self.value + amount).min(self.max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> ItemTable {
        let mut raw = alloc::vec![0u8; ITEM_COUNT * 32];
        let mut put = |n: usize, rec: [i32; 8]| {
            for (i, v) in rec.iter().enumerate() {
                raw[n * 32 + i * 4..n * 32 + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
        };
        put(7, [2, 8, 200, 0, 1, 129, 130, 209]);
        put(12, [2, 8, 1200, 1, 6, 139, 140, 208]);
        put(29, [1, 7, 1600, 2, 7, 175, 176, 208]);
        put(36, [-1, 7, 0, 0, -1, 189, 190, 206]);
        ItemTable::from_bytes(&raw)
    }

    #[test]
    fn records_slots_and_power() {
        let t = table();
        assert_eq!(t.get(7).unwrap().owner(), Some(Who::Bentley));
        assert_eq!(t.get(29).unwrap().button_use(), ButtonUse::Hold);
        assert_eq!(t.get(36).unwrap().button_use(), ButtonUse::Passive);
        let mut owned = Owned::default();
        let mut s = Slots::default();
        assert!(!s.assign(0, 7, Who::Bentley, &t, &owned), "not owned");
        owned.add(7);
        owned.add(12);
        owned.add(36);
        assert!(s.assign(0, 7, Who::Bentley, &t, &owned));
        assert!(s.assign(2, 7, Who::Bentley, &t, &owned));
        assert_eq!(s.0, [None, None, Some(7)]);
        assert!(!s.assign(1, 36, Who::Sly, &t, &owned), "passive items take no button");
        assert!(!s.assign(1, 12, Who::Sly, &t, &owned), "Bentley's item");
        let mut p = Power::new(30.0, 100.0);
        assert!(p.pay(20.0) && !p.pay(20.0));
        // A drain of 2 per second takes whole units: 1 after 0.5 s.
        let mut p = Power::new(3.0, 100.0);
        assert!(p.drain(2.0, 0.4));
        assert_eq!(p.value, 3.0);
        assert!(p.drain(2.0, 0.1));
        assert_eq!(p.value, 2.0);
        assert!(!p.drain(2.0, 1.0));
        assert_eq!(p.value, 0.0);
        p.add(500.0);
        assert_eq!(p.value, 100.0);
    }
}
