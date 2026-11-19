//! Pickpocketing (specs/sly2/pickpocket-and-pickups.md 2, guards.md 8): the pocket tables, filling a pocket of up to
//! 8 entries, taking entries, the loot kinds, and throwing a taken entry to the character.
//!
//! The guards and other givers own their pockets; their behaviours call [`Pocket::fill`] when the giver resets and
//! [`Pocket::take`] at the pickpocket animation's event, then [`give`] to launch the entry and [`Purse::receive`]
//! when it is collected.

use alloc::vec::Vec;

use formats_sly2::exe::Executable;

use crate::math::{Vec3, add, cos, length, normalize, scale, sin, sub};
use crate::physics::Rng;

/// The guards' pocket table (class 58) [D].
pub const GUARD_TABLE_VA: u32 = 0x002C_3968;
/// The other givers' pocket table [D].
pub const OTHER_TABLE_VA: u32 = 0x002C_3748;
/// One 60-byte row per episode, row 0 unused.
pub const POCKET_ROWS: usize = 9;
/// The loot table: 64 records of 12 bytes [D].
pub const LOOT_TABLE_VA: u32 = 0x002C_3288;
/// Loot records.
pub const LOOT_COUNT: usize = 64;
/// A pocket holds at most this many entries.
pub const POCKET_MAX: usize = 8;
/// The first prize kind: kind 1114 + i is loot record i (i = 0 to 29).
pub const FIRST_PRIZE_KIND: i32 = 1114;

/// One episode's pocket row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PocketRow {
    /// Fewest and most coins.
    pub coins: [i32; 2],
    /// Chance of a prize.
    pub chance: f32,
    /// Six (weight, prize kind) pairs.
    pub prizes: [(i32, i32); 6],
}

impl PocketRow {
    /// From its 60 bytes.
    pub fn from_bytes(b: &[u8; 60]) -> PocketRow {
        let i = |k: usize| i32::from_le_bytes([b[k * 4], b[k * 4 + 1], b[k * 4 + 2], b[k * 4 + 3]]);
        let mut prizes = [(0, 0); 6];
        for (n, p) in prizes.iter_mut().enumerate() {
            *p = (i(3 + 2 * n), i(4 + 2 * n));
        }
        PocketRow { coins: [i(0), i(1)], chance: f32::from_bits(i(2) as u32), prizes }
    }

    /// Whether the row is empty (row 0).
    pub fn is_empty(&self) -> bool {
        self.coins[1] <= 0 && self.prizes.iter().all(|p| p.0 <= 0)
    }
}

/// A pocket table: a row per episode.
#[derive(Clone, Debug, PartialEq)]
pub struct PocketTable {
    /// Rows 0 to 8.
    pub rows: Vec<PocketRow>,
}

impl PocketTable {
    /// Reads a table at `va` from the player's executable after its SHA-256 check.
    pub fn from_executable(exe: &Executable, va: u32) -> formats::Result<PocketTable> {
        if !exe.is_supported() {
            return Err(formats::Error::UnknownExecutable);
        }
        let raw = exe.bytes_at(va, POCKET_ROWS * 60)?;
        Ok(PocketTable::from_bytes(raw))
    }

    /// From raw rows of 60 bytes (Sly 3's tables have a zero row and five episode rows).
    pub fn from_bytes(raw: &[u8]) -> PocketTable {
        PocketTable { rows: raw.as_chunks::<60>().0.iter().map(PocketRow::from_bytes).collect() }
    }

    /// The row of `episode` (none for row 0 or out of range).
    pub fn row(&self, episode: usize) -> Option<&PocketRow> {
        self.rows.get(episode).filter(|r| !r.is_empty())
    }
}

/// One loot record (safehouse-and-shop.md 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loot {
    /// Value in coins.
    pub value: i32,
    /// Name string ID.
    pub name: i32,
    /// The second string ID (the prize kind for records 0 to 29).
    pub kind: i32,
}

/// The loot table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LootTable {
    /// The records.
    pub records: Vec<Loot>,
}

impl LootTable {
    /// Reads it from the player's executable after its SHA-256 check.
    pub fn from_executable(exe: &Executable) -> formats::Result<LootTable> {
        if !exe.is_supported() {
            return Err(formats::Error::UnknownExecutable);
        }
        Ok(LootTable::from_bytes(exe.bytes_at(LOOT_TABLE_VA, LOOT_COUNT * 12)?))
    }

    /// From raw records of 12 bytes.
    pub fn from_bytes(raw: &[u8]) -> LootTable {
        let records = raw
            .as_chunks::<12>()
            .0
            .iter()
            .map(|c| {
                let i = |k: usize| i32::from_le_bytes([c[k * 4], c[k * 4 + 1], c[k * 4 + 2], c[k * 4 + 3]]);
                Loot { value: i(0), name: i(1), kind: i(2) }
            })
            .collect();
        LootTable { records }
    }

    /// The loot record a Sly 2 prize kind names (1114 + i → record i, i = 0 to 29).
    pub fn record_of(kind: i32) -> Option<usize> {
        LootTable::record_from(kind, FIRST_PRIZE_KIND)
    }

    /// The loot record a prize kind names with prize kinds starting at `first` (Sly 2 1114, Sly 3 1255).
    pub fn record_from(kind: i32, first: i32) -> Option<usize> {
        let i = kind - first;
        (0..30).contains(&i).then_some(i as usize)
    }
}

/// A pocket entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A number of coins.
    Coins(u32),
    /// A prize: a loot kind (1114 to 1143), or a task item.
    Prize(i32),
}

/// What a giver shows over its head (pickpocket-and-pickups.md 2.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    /// Nothing (alerted, empty, or the current character is not Sly).
    None,
    /// A plain marker: only coins left.
    Plain,
    /// A glint: the next entry is a prize.
    Glint,
}

/// A giver's pocket.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pocket {
    /// The entries left, taken from the front.
    pub entries: Vec<Entry>,
}

impl Pocket {
    /// Fills the pocket from an episode's row (pickpocket-and-pickups.md 2.2): a prize kept from the previous fill
    /// stays unless `task_done` says its task is done; otherwise one prize is drawn by the weights with the row's
    /// chance; the coins drawn in the row's range are split over 2 or 3 entries as evenly as possible (each at
    /// least 1); the prize, if any, is the last entry.
    pub fn fill(&mut self, row: &PocketRow, rng: &mut impl Rng, task_done: impl Fn(i32) -> bool) {
        let kept = self.entries.iter().find_map(|e| match e {
            Entry::Prize(k) if !task_done(*k) => Some(*k),
            _ => None,
        });
        self.entries.clear();
        let prize = kept.or_else(|| {
            if rng.next_f32() >= row.chance {
                return None;
            }
            let total: i32 = row.prizes.iter().map(|p| p.0.max(0)).sum();
            if total <= 0 {
                return None;
            }
            let mut pick = rng.next_f32() * total as f32;
            for &(w, k) in &row.prizes {
                let w = w.max(0) as f32;
                if pick < w {
                    return Some(k);
                }
                pick -= w;
            }
            row.prizes.iter().rev().find(|p| p.0 > 0).map(|p| p.1)
        });
        let [lo, hi] = row.coins;
        let span = (hi - lo).max(0);
        let coins = (lo + ((rng.next_f32() * (span + 1) as f32) as i32).min(span)).max(0) as u32;
        let parts = if rng.next_f32() < 0.5 { 2 } else { 3 };
        for c in split_coins(coins, parts) {
            self.entries.push(Entry::Coins(c));
        }
        if let Some(k) = prize {
            self.entries.truncate(POCKET_MAX - 1);
            self.entries.push(Entry::Prize(k));
        }
    }

    /// Takes the next entry (one per pickpocket).
    pub fn take(&mut self) -> Option<Entry> {
        (!self.entries.is_empty()).then(|| self.entries.remove(0))
    }

    /// Whether the next entry is a prize.
    pub fn next_is_prize(&self) -> bool {
        matches!(self.entries.first(), Some(Entry::Prize(_)))
    }

    /// The marker over the giver.
    pub fn marker(&self, alerted: bool, current_is_sly: bool) -> Marker {
        if alerted || !current_is_sly || self.entries.is_empty() {
            Marker::None
        } else if self.next_is_prize() {
            Marker::Glint
        } else {
            Marker::Plain
        }
    }
}

/// `coins` split over `parts` entries as evenly as possible, each at least 1 (fewer entries when there are fewer
/// coins).
pub fn split_coins(coins: u32, parts: u32) -> Vec<u32> {
    let parts = parts.min(coins);
    if parts == 0 {
        return Vec::new();
    }
    let (base, extra) = (coins / parts, coins % parts);
    (0..parts).map(|i| base + u32::from(i < extra)).collect()
}

/// The launch constants (pickpocket-and-pickups.md 2.4) [D].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GiveConstants {
    /// Flight time range (0.75, 1.2), the delay between copies (0.05) and the collect delay per order (0.15)
    /// [D 0x2C0CE0].
    pub timing: [f32; 4],
    /// Angle range from the line to the giver (−1.5, 1.5) and the distance range (100, 150) [D 0x2C0CF0].
    pub ring: [f32; 4],
    /// Gravity of launched pickups (−2250): the z of the vector (0, 0, −2250) at 0x2C0D60 [D].
    pub gravity: f32,
}

/// The landing point is this far below the receiver's centre.
pub const GIVE_DROP: f32 = 75.0;

impl GiveConstants {
    /// The spec's values.
    pub const SPEC: GiveConstants =
        GiveConstants { timing: [0.75, 1.2, 0.05, 0.15], ring: [-1.5, 1.5, 100.0, 150.0], gravity: -2250.0 };

    /// Reads them from the player's executable after its SHA-256 check.
    pub fn from_executable(exe: &Executable) -> formats::Result<GiveConstants> {
        if !exe.is_supported() {
            return Err(formats::Error::UnknownExecutable);
        }
        let f = |va: u32| -> formats::Result<f32> {
            let b = exe.bytes_at(va, 4)?;
            Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let four = |va: u32| -> formats::Result<[f32; 4]> { Ok([f(va)?, f(va + 4)?, f(va + 8)?, f(va + 12)?]) };
        Ok(GiveConstants { timing: four(0x002C_0CE0)?, ring: four(0x002C_0CF0)?, gravity: f(0x002C_0D68)? })
    }
}

/// One launched copy of a taken entry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Launch {
    /// What it gives when collected: one coin, or a prize.
    pub gives: Entry,
    /// Launch delay after the take.
    pub delay: f32,
    /// Launch velocity.
    pub velocity: Vec3,
    /// Where it lands.
    pub target: Vec3,
    /// Flight time.
    pub flight: f32,
    /// A coin is collected by itself this long after the throw (0.15 s × its order), wherever it lands; `None` for a
    /// prize.
    pub auto_collect: Option<f32>,
}

/// Throws a taken entry from `from` (the giver) toward the character whose centre is `receiver`: each copy to a
/// point at a random angle of ±1.5 rad from the line to the giver, 100 to 150 out, 75 below the centre, in a flight
/// time of 0.75 to 1.2 s under gravity −2250; copies leave 0.05 s apart.
pub fn give(entry: Entry, from: Vec3, receiver: Vec3, c: &GiveConstants, rng: &mut impl Rng) -> Vec<Launch> {
    let copies = match entry {
        Entry::Coins(n) => n,
        Entry::Prize(_) => 1,
    };
    let toward = sub(from, receiver);
    let base = if length([toward[0], toward[1], 0.0]) > 1e-3 {
        normalize([toward[0], toward[1], 0.0])
    } else {
        [1.0, 0.0, 0.0]
    };
    let mut out = Vec::new();
    for k in 0..copies {
        let lerp = |a: f32, b: f32, u: f32| a + (b - a) * u;
        let a = lerp(c.ring[0], c.ring[1], rng.next_f32());
        let r = lerp(c.ring[2], c.ring[3], rng.next_f32());
        let t = lerp(c.timing[0], c.timing[1], rng.next_f32());
        let (sa, ca) = (sin(a), cos(a));
        let dir = [base[0] * ca - base[1] * sa, base[0] * sa + base[1] * ca, 0.0];
        let target = add(add(receiver, scale(dir, r)), [0.0, 0.0, -GIVE_DROP]);
        let d = sub(target, from);
        let velocity = [d[0] / t, d[1] / t, (d[2] - 0.5 * c.gravity * t * t) / t];
        let delay = c.timing[2] * k as f32;
        let (gives, auto_collect) = match entry {
            Entry::Coins(_) => (Entry::Coins(1), Some(delay + c.timing[3] * (k + 1) as f32)),
            Entry::Prize(p) => (Entry::Prize(p), None),
        };
        out.push(Launch { gives, delay, velocity, target, flight: t, auto_collect });
    }
    out
}

/// Coins and loot counts (the save's `ccoin` and `mplootkc`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Purse {
    /// Coins.
    pub coins: u32,
    /// Count held of each loot record.
    pub loot: [u32; LOOT_COUNT],
}

impl Default for Purse {
    fn default() -> Self {
        Purse { coins: 0, loot: [0; LOOT_COUNT] }
    }
}

impl Purse {
    /// A collected entry: coins add their count; a loot prize adds one to its record. Returns false for a prize
    /// that is not loot (a task item: the mission system completes its task).
    pub fn receive(&mut self, e: Entry) -> bool {
        self.receive_from(e, FIRST_PRIZE_KIND)
    }

    /// As [`Self::receive`] with prize kinds starting at `first` (Sly 3: 1255).
    pub fn receive_from(&mut self, e: Entry, first: i32) -> bool {
        match e {
            Entry::Coins(n) => {
                self.coins = self.coins.saturating_add(n);
                true
            }
            Entry::Prize(k) => match LootTable::record_from(k, first) {
                Some(i) => {
                    self.loot[i] += 1;
                    true
                }
                None => false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::SplitMix64;

    fn row() -> PocketRow {
        PocketRow {
            coins: [6, 12],
            chance: 0.4,
            prizes: [(30, 1116), (30, 1115), (15, 1122), (15, 1121), (5, 1119), (5, 1118)],
        }
    }

    #[test]
    fn rows_and_split() {
        let mut b = [0u8; 60];
        let vals: [u32; 15] = [6, 12, 0.4f32.to_bits(), 30, 1116, 30, 1115, 15, 1122, 15, 1121, 5, 1119, 5, 1118];
        for (i, v) in vals.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        assert_eq!(PocketRow::from_bytes(&b), row());
        assert_eq!(split_coins(7, 3), [3, 2, 2]);
        assert_eq!(split_coins(12, 2), [6, 6]);
        assert_eq!(split_coins(2, 3), [1, 1]);
        assert_eq!(LootTable::record_of(1114), Some(0));
        assert_eq!(LootTable::record_of(1143), Some(29));
        assert_eq!(LootTable::record_of(1144), None);
    }

    #[test]
    fn fill_take_and_markers() {
        let mut rng = SplitMix64(5);
        let (mut prizes, mut total) = (0, 0);
        let mut weights = [0u32; 6];
        for _ in 0..2000 {
            let mut p = Pocket::default();
            p.fill(&row(), &mut rng, |_| false);
            assert!(p.entries.len() <= POCKET_MAX);
            let coins: u32 = p.entries.iter().map(|e| if let Entry::Coins(n) = e { *n } else { 0 }).sum();
            assert!((6..=12).contains(&coins), "{coins}");
            let n = p.entries.iter().filter(|e| matches!(e, Entry::Coins(_))).count();
            assert!((2..=3).contains(&n) && p.entries.iter().all(|e| *e != Entry::Coins(0)));
            if let Some(Entry::Prize(k)) = p.entries.last() {
                prizes += 1;
                weights[row().prizes.iter().position(|q| q.1 == *k).unwrap()] += 1;
            }
            total += 1;
        }
        // The prize chance is 0.4; the six weights are 30, 30, 15, 15, 5, 5.
        let ratio = prizes as f32 / total as f32;
        assert!((ratio - 0.4).abs() < 0.04, "{ratio}");
        assert!(weights[0] > weights[2] * 3 / 2 && weights[2] > weights[4] * 2);
        // A kept prize stays unless its task is done; taking empties from the front, the prize last.
        let mut p = Pocket { entries: alloc::vec![Entry::Coins(3), Entry::Prize(9000)] };
        p.fill(&PocketRow { chance: 0.0, ..row() }, &mut rng, |_| false);
        assert_eq!(p.entries.last(), Some(&Entry::Prize(9000)));
        p.fill(&PocketRow { chance: 0.0, ..row() }, &mut rng, |k| k == 9000);
        assert!(!p.entries.iter().any(|e| matches!(e, Entry::Prize(_))));
        let mut p = Pocket { entries: alloc::vec![Entry::Coins(3), Entry::Prize(1115)] };
        assert_eq!(p.marker(false, true), Marker::Plain);
        assert_eq!(p.take(), Some(Entry::Coins(3)));
        assert_eq!(p.marker(false, true), Marker::Glint);
        assert_eq!(p.marker(true, true), Marker::None);
        assert_eq!(p.marker(false, false), Marker::None);
        let mut purse = Purse::default();
        assert!(purse.receive(p.take().unwrap()));
        assert_eq!(purse.loot[1], 1);
        assert!(purse.receive(Entry::Coins(4)));
        assert_eq!(purse.coins, 4);
        assert!(!purse.receive(Entry::Prize(9000)));
        assert_eq!(p.take(), None);
    }

    #[test]
    fn giving_lands_around_the_receiver() {
        let c = GiveConstants::SPEC;
        let mut rng = SplitMix64(9);
        let from = [300.0, 0.0, 100.0];
        let receiver = [0.0, 0.0, 100.0];
        let l = give(Entry::Coins(3), from, receiver, &c, &mut rng);
        assert_eq!(l.len(), 3);
        for (k, x) in l.iter().enumerate() {
            assert!((x.delay - 0.05 * k as f32).abs() < 1e-6);
            assert!((x.auto_collect.unwrap() - (x.delay + 0.15 * (k + 1) as f32)).abs() < 1e-6);
            assert!((0.75..=1.2).contains(&x.flight));
            let off = sub(x.target, receiver);
            let r = length([off[0], off[1], 0.0]);
            assert!((100.0..=150.0).contains(&r) && (off[2] + 75.0).abs() < 1e-3);
            // Within ±1.5 rad of the line to the giver (+x).
            assert!(off[0] / r >= cos(1.5) - 1e-5);
            // The ballistic flight lands on the target.
            let t = x.flight;
            let p = [
                from[0] + x.velocity[0] * t,
                from[1] + x.velocity[1] * t,
                from[2] + x.velocity[2] * t + 0.5 * c.gravity * t * t,
            ];
            assert!(length(sub(p, x.target)) < 1e-2);
        }
        let l = give(Entry::Prize(1120), from, receiver, &c, &mut rng);
        assert_eq!((l.len(), l[0].auto_collect), (1, None));
    }
}
