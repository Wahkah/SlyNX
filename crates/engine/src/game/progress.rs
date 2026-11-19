//! The player's progress: counters, per-level records and the timed run (pickups-and-progress.md 3, 5 and 6;
//! level-flow.md 4 and 10). It outlives worlds: the game application keeps one and hands it to each world.

use alloc::collections::{BTreeMap, BTreeSet};

/// Lives at the start of a new game.
pub const START_LIVES: i32 = 5;
/// Most lives.
pub const MAX_LIVES: i32 = 99;
/// Most charms.
pub const MAX_CHARMS: i32 = 2;
/// The coin counter's last value before it rolls over.
pub const MAX_COINS: i32 = 99;
/// Default sprint time when the world sets none (world option 538).
pub const DEFAULT_SPRINT_TIME: f32 = 180.0;
/// Added to the sprint time when the sprint starts.
pub const SPRINT_EXTRA: f32 = 0.99;

/// Level record flag bits (pickups-and-progress.md 6), the same positions as the world table's task mask.
pub mod level_flag {
    /// Visited.
    pub const VISITED: u32 = 1 << 0;
    /// Key collected.
    pub const KEY: u32 = 1 << 1;
    /// Vault opened.
    pub const VAULT: u32 = 1 << 2;
    /// Sprint completed.
    pub const SPRINT: u32 = 1 << 3;
    /// Boss defeated.
    pub const BOSS: u32 = 1 << 4;
}

/// Game-wide flags (pickups-and-progress.md 7.2, "game flags").
pub mod game_flag {
    /// A clue bottle has been collected once.
    pub const FIRST_BOTTLE: u32 = 1 << 0;
}

/// What one level keeps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelRecord {
    /// [`level_flag`] bits.
    pub flags: u32,
    /// One bit per clue bottle, in load order.
    pub bottles: u64,
    /// One bit per checkpoint reached.
    pub checkpoints: u8,
    /// Placed pickups already collected (their persist keys), so they are not placed again.
    pub collected: BTreeSet<u64>,
    /// Remembered buttons and volumes that were pressed (persist keys).
    pub pressed: BTreeSet<u64>,
    /// The sprint's best remaining time.
    pub best_time: Option<f32>,
    /// The sprint's best completion time (seconds from the start to the exit), what a save slot keeps.
    pub best_elapsed: Option<f32>,
    /// All the level's bottles have been collected once (the "all bottles" dialogue played).
    pub all_bottles_seen: bool,
    /// Guards knocked out (persist keys): they stay down after a checkpoint restart.
    pub defeated: BTreeSet<u64>,
    /// Breakables broken (persist keys): broken again silently on load.
    pub broken: BTreeSet<u64>,
    /// Dialogues played (anchor OIDs, dialogue-and-voice.md 6).
    pub dialogues: BTreeSet<i16>,
    /// The adaptive difficulty state.
    pub difficulty: super::difficulty::LevelDifficulty,
}

/// Where the player comes back after dying.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Respawn {
    /// World id.
    pub world: u32,
    /// Warp OID.
    pub warp: Option<i16>,
}

/// A Master Thief Sprint in progress (pickups-and-progress.md 5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprint {
    /// World id.
    pub world: u32,
    /// Seconds left.
    pub remaining: f32,
    /// Seconds the countdown started with.
    pub total: f32,
    /// Alarms triggered so far (the countdown runs at 2^alarms).
    pub alarms: u32,
}

/// What adding a coin did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoinResult {
    /// The counter went up.
    Counted,
    /// The counter rolled over into a charm.
    GaveCharm,
    /// The counter rolled over into a life.
    GaveLife,
    /// Counter, charms and lives were full; the counter stays at 99.
    Full,
}

/// The progress.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    /// Lives (−1 is game over).
    pub lives: i32,
    /// Charms, 0 to 2.
    pub charms: i32,
    /// Coin counter, 0 to 99.
    pub coins: i32,
    /// Per-level records, by world id.
    pub levels: BTreeMap<u32, LevelRecord>,
    /// Keys collected per episode.
    pub keys: BTreeMap<u32, u32>,
    /// Episodes completed (bit n: episode n).
    pub episodes: u32,
    /// [`game_flag`] bits.
    pub flags: u32,
    /// Vault reward bits.
    pub rewards: u32,
    /// Where to respawn.
    pub respawn: Option<Respawn>,
    /// The sprint in progress.
    pub sprint: Option<Sprint>,
    /// Vaults opened per episode.
    pub vaults: BTreeMap<u32, u32>,
    /// World-state bits per episode (lock groups, level-flow.md 7.1).
    pub episode_flags: BTreeMap<u32, u32>,
    /// The adaptive difficulty tables (read from the player's executable), if available.
    pub difficulty_tables: Option<super::difficulty::DifficultyTables>,
}

impl Default for Progress {
    fn default() -> Self {
        Progress::new_game()
    }
}

impl Progress {
    /// A new game: 5 lives, no charms, no coins, nothing done.
    pub fn new_game() -> Progress {
        Progress {
            lives: START_LIVES,
            charms: 0,
            coins: 0,
            levels: BTreeMap::new(),
            keys: BTreeMap::new(),
            episodes: 0,
            flags: 0,
            rewards: 0,
            respawn: None,
            sprint: None,
            vaults: BTreeMap::new(),
            episode_flags: BTreeMap::new(),
            difficulty_tables: None,
        }
    }

    /// The record of a level (made empty on first use).
    pub fn level(&mut self, world: u32) -> &mut LevelRecord {
        self.levels.entry(world).or_default()
    }

    /// The record of a level, if it has one.
    pub fn level_ref(&self, world: u32) -> Option<&LevelRecord> {
        self.levels.get(&world)
    }

    /// Whether a level flag is set.
    pub fn has_flag(&self, world: u32, flag: u32) -> bool {
        self.levels.get(&world).is_some_and(|l| l.flags & flag != 0)
    }

    /// Adds a coin (pickups-and-progress.md 3.1): past 99 the counter resets and gives a charm if fewer than two are
    /// held, else a life if lives are below 99, else it stays at 99.
    pub fn add_coin(&mut self) -> CoinResult {
        self.coins += 1;
        if self.coins <= MAX_COINS {
            return CoinResult::Counted;
        }
        if self.charms < MAX_CHARMS {
            self.coins = 0;
            self.charms += 1;
            CoinResult::GaveCharm
        } else if self.lives < MAX_LIVES {
            self.coins = 0;
            self.lives += 1;
            CoinResult::GaveLife
        } else {
            self.coins = MAX_COINS;
            CoinResult::Full
        }
    }

    /// Adds a charm; false (and nothing changes) when two are held already.
    pub fn add_charm(&mut self) -> bool {
        if self.charms >= MAX_CHARMS {
            return false;
        }
        self.charms += 1;
        true
    }

    /// Adds a life, up to 99.
    pub fn add_life(&mut self) {
        self.lives = (self.lives + 1).min(MAX_LIVES);
    }

    /// Loses a life; returns true when that is game over (lives below 0).
    pub fn lose_life(&mut self) -> bool {
        self.lives -= 1;
        self.lives < 0
    }

    /// Collects a level's key: sets its flag once and counts a key for the episode. Returns false if the level
    /// already had it.
    pub fn collect_key(&mut self, world: u32) -> bool {
        let l = self.level(world);
        if l.flags & level_flag::KEY != 0 {
            return false;
        }
        l.flags |= level_flag::KEY;
        *self.keys.entry(world >> 8).or_insert(0) += 1;
        true
    }

    /// Marks a clue bottle collected. Returns (first bottle ever, first time every bottle of the level is in).
    pub fn collect_bottle(&mut self, world: u32, index: u32, total: u32) -> (bool, bool) {
        let first_ever = self.flags & game_flag::FIRST_BOTTLE == 0;
        self.flags |= game_flag::FIRST_BOTTLE;
        let l = self.level(world);
        if index < 64 {
            l.bottles |= 1 << index;
        }
        let all_mask = if total >= 64 { u64::MAX } else { (1u64 << total) - 1 };
        let all_now = total > 0 && l.bottles & all_mask == all_mask && !l.all_bottles_seen;
        if all_now {
            l.all_bottles_seen = true;
        }
        (first_ever, all_now)
    }

    /// Starts a sprint (pickups-and-progress.md 5): the charms drop to 0 and the countdown starts.
    pub fn start_sprint(&mut self, world: u32, seconds: f32) {
        self.charms = 0;
        let total = seconds + SPRINT_EXTRA;
        self.sprint = Some(Sprint { world, remaining: total, total, alarms: 0 });
    }

    /// Ends a sprint at an exit with time left: the first completion sets the flag; the best time is kept. Returns
    /// whether this was the first completion.
    pub fn finish_sprint(&mut self) -> bool {
        let Some(s) = self.sprint.take() else { return false };
        let l = self.level(s.world);
        let first = l.flags & level_flag::SPRINT == 0;
        l.flags |= level_flag::SPRINT;
        l.best_time = Some(l.best_time.map_or(s.remaining, |b| b.max(s.remaining)));
        let elapsed = (s.total - s.remaining).max(0.0);
        l.best_elapsed = Some(l.best_elapsed.map_or(elapsed, |b| b.min(elapsed)));
        first
    }

    /// Advances the sprint countdown by `dt` game seconds (2^alarms faster). Returns true when it ran out this step
    /// (the sprint fails and ends).
    pub fn tick_sprint(&mut self, dt: f32) -> bool {
        let Some(s) = self.sprint.as_mut() else { return false };
        let rate = (1u32 << s.alarms.min(16)) as f32;
        s.remaining -= dt * rate;
        if s.remaining <= 0.0 {
            self.sprint = None;
            return true;
        }
        false
    }

    /// Records a checkpoint (level-flow.md 4): its bit and the respawn warp. Returns whether it had been reached
    /// before.
    pub fn reach_checkpoint(&mut self, world: u32, index: u32, warp: Option<i16>) -> bool {
        let l = self.level(world);
        let bit = 1u8 << index.min(7);
        let before = l.checkpoints & bit != 0;
        l.checkpoints |= bit;
        self.respawn = Some(Respawn { world, warp });
        before
    }

    /// Completion percentage (pickups-and-progress.md 6): one task per visit of each level outside the intro
    /// episode, one per bit of its task mask, one per finished episode (5). `worlds` lists every world's id and
    /// task mask. 0 when nothing is done, 100 when everything is, else `floor(100 × done / total)` within 1..99.
    pub fn percent_complete(&self, worlds: &[(u32, u32)]) -> u32 {
        let mut total = 5u32;
        let mut done = self.episodes.count_ones().min(5);
        for &(id, mask) in worlds {
            if id >> 8 == 0 {
                continue;
            }
            let flags = self.levels.get(&id).map_or(0, |l| l.flags);
            total += 1 + (mask & 0xE).count_ones();
            done += (flags & level_flag::VISITED != 0) as u32 + (flags & mask & 0xE).count_ones();
        }
        if done == 0 {
            0
        } else if done >= total {
            100
        } else {
            (100 * done / total).clamp(1, 99)
        }
    }

    /// A stable hash of the progress for determinism checks (FNV-1a over the counters and level records).
    pub fn hash(&self) -> u64 {
        let mut h = crate::game::Fnv::new();
        h.i32(self.lives).i32(self.charms).i32(self.coins).u32(self.episodes).u32(self.flags);
        for (id, l) in &self.levels {
            h.u32(*id).u32(l.flags).u64(l.bottles).u32(l.checkpoints as u32).u32(l.collected.len() as u32);
        }
        if let Some(s) = self.sprint {
            h.f32(s.remaining);
        }
        h.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coins_roll_over_into_charms_then_lives() {
        let mut p = Progress::new_game();
        for _ in 0..99 {
            assert_eq!(p.add_coin(), CoinResult::Counted);
        }
        assert_eq!(p.coins, 99);
        assert_eq!(p.add_coin(), CoinResult::GaveCharm);
        assert_eq!((p.coins, p.charms), (0, 1));
        p.charms = 2;
        p.coins = 99;
        assert_eq!(p.add_coin(), CoinResult::GaveLife);
        assert_eq!((p.coins, p.lives), (0, 6));
        p.lives = 99;
        p.coins = 99;
        assert_eq!(p.add_coin(), CoinResult::Full);
        assert_eq!(p.coins, 99);
    }

    #[test]
    fn charms_lives_keys() {
        let mut p = Progress::new_game();
        assert!(p.add_charm() && p.add_charm());
        assert!(!p.add_charm());
        p.lives = 99;
        p.add_life();
        assert_eq!(p.lives, 99);
        p.lives = 0;
        assert!(p.lose_life());
        assert!(p.collect_key(0x103));
        assert!(!p.collect_key(0x103));
        assert_eq!(p.keys[&1], 1);
        assert!(p.has_flag(0x103, level_flag::KEY));
    }

    #[test]
    fn sprint() {
        let mut p = Progress::new_game();
        p.charms = 2;
        p.start_sprint(0x103, 10.0);
        assert_eq!(p.charms, 0);
        assert!((p.sprint.unwrap().remaining - 10.99).abs() < 1e-5);
        p.sprint.as_mut().unwrap().alarms = 1;
        assert!(!p.tick_sprint(5.0)); // 10 seconds of countdown at double speed
        assert!(p.finish_sprint());
        assert!(p.has_flag(0x103, level_flag::SPRINT));
        p.start_sprint(0x103, 1.0);
        assert!(p.tick_sprint(2.0));
        assert!(p.sprint.is_none());
    }

    #[test]
    fn completion_percentage() {
        // 46 worlds: 3 intro, 19 with mask 0xE, 9 with 0x2, 15 with 0: 114 tasks.
        let mut worlds = alloc::vec![(0x002, 0), (0x003, 0), (0x004, 0)];
        worlds.extend((0..19).map(|i| (0x100 + i, 0xE)));
        worlds.extend((0..9).map(|i| (0x200 + i, 0x2)));
        worlds.extend((0..15).map(|i| (0x300 + i, 0)));
        let mut p = Progress::new_game();
        assert_eq!(p.percent_complete(&worlds), 0);
        p.level(0x100).flags = level_flag::VISITED;
        assert_eq!(p.percent_complete(&worlds), 1); // 1 of 114 rounds down to 0, shown as 1
        for &(id, mask) in &worlds {
            if id >> 8 != 0 {
                p.level(id).flags = level_flag::VISITED | mask;
            }
        }
        assert_eq!(p.percent_complete(&worlds), 95); // 109 of 114
        p.episodes = 0b111110;
        assert_eq!(p.percent_complete(&worlds), 100);
    }

    #[test]
    fn bottles_and_checkpoints() {
        let mut p = Progress::new_game();
        assert_eq!(p.collect_bottle(0x103, 0, 2), (true, false));
        assert_eq!(p.collect_bottle(0x103, 1, 2), (false, true));
        assert_eq!(p.collect_bottle(0x103, 1, 2), (false, false));
        assert!(!p.reach_checkpoint(0x103, 0, Some(7)));
        assert!(p.reach_checkpoint(0x103, 0, Some(7)));
        assert_eq!(p.respawn, Some(Respawn { world: 0x103, warp: Some(7) }));
    }
}
