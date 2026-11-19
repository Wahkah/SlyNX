//! The persistent state of one save slot (save-format.md 1, pickups-and-progress.md 6 and 7.2), its value ranges,
//! and the progress and restore rules the engine applies to it (save-format.md 3).

/// Episodes, including the prologue (episode 0).
pub const EPISODES: usize = 6;
/// Level records per episode.
pub const LEVELS_PER_EPISODE: usize = 9;
/// Lives at the start of a new game, and after "Try again?" on game over.
pub const STARTING_LIVES: i8 = 5;
/// Ranges of the slot counters.
pub const MAX_LIVES: i8 = 99;
pub const MIN_LIVES: i8 = -1;
pub const MAX_CHARMS: u8 = 2;
pub const MAX_COINS: u8 = 99;
/// Dialogue-played flags, script key/value pairs, clue bottles and checkpoints per level.
pub const DIALOGUE_FLAGS: usize = 12;
pub const SCRIPT_VALUES: usize = 4;
pub const BOTTLES: usize = 64;
pub const CHECKPOINTS: usize = 8;
/// Power-up rewards fit in a 32-bit mask, so a selected power-up index is below 32.
pub const MAX_POWER_UPS: u8 = 32;

/// Per-level goal flags (pickups-and-progress.md 6). Bits 1 to 3 share their positions with the world table's task
/// mask.
pub mod level_flags {
    pub const VISITED: u8 = 1 << 0;
    pub const KEY: u8 = 1 << 1;
    pub const VAULT: u8 = 1 << 2;
    pub const SPRINT: u8 = 1 << 3;
    pub const BOSS: u8 = 1 << 4;
    pub const ALL: u8 = 0x1F;
}

/// Per-episode flags.
pub mod episode_flags {
    pub const VISITED: u8 = 1 << 0;
    pub const FINISHED: u8 = 1 << 1;
    pub const ALL: u8 = 0x03;
}

/// Game milestone flags (one-time events).
pub mod milestones {
    pub const FIRST_BOTTLE: u8 = 1 << 0;
    pub const HALF_BOTTLES: u8 = 1 << 1;
    pub const ALL_BOTTLES: u8 = 1 << 2;
    pub const FIRST_VAULT: u8 = 1 << 3;
    pub const SECOND_VAULT: u8 = 1 << 4;
    pub const ALL: u8 = 0x1F;
}

/// The slot's settings bits (save-format.md 1). Bits not listed are kept as they are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings(pub u32);

impl Settings {
    /// The "new episode unlocked" voice line has played.
    pub const EPISODE_LINE_PLAYED: u32 = 1 << 1;
    pub const VIBRATION_OFF: u32 = 1 << 5;
    pub const MONO: u32 = 1 << 6;
    pub const MUSIC_OFF: u32 = 1 << 7;
    /// The player acknowledged the auto-save notice: auto-save is on.
    pub const AUTOSAVE: u32 = 1 << 8;
    /// Binoc-u-com: up = look up.
    pub const BINOCUCOM_UP_IS_UP: u32 = 1 << 9;
    pub const TURRETS_INVERTED: u32 = 1 << 10;
    pub const JETPACK_INVERTED: u32 = 1 << 11;

    pub fn get(self, bit: u32) -> bool {
        self.0 & bit != 0
    }

    pub fn set(&mut self, bit: u32, on: bool) {
        if on {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }

    pub fn vibration(self) -> bool {
        !self.get(Self::VIBRATION_OFF)
    }
    pub fn mono(self) -> bool {
        self.get(Self::MONO)
    }
    pub fn music(self) -> bool {
        !self.get(Self::MUSIC_OFF)
    }
    pub fn autosave(self) -> bool {
        self.get(Self::AUTOSAVE)
    }
}

/// One level's record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelRecord {
    /// [`level_flags`].
    pub flags: u8,
    /// Seconds spent in the level.
    pub time: f32,
    /// Best Master Thief Sprint time in seconds; 0 when never completed.
    pub best_sprint: f32,
    /// Adaptive difficulty, 0..=1, and its raw value, 0..=1.
    pub difficulty: f32,
    pub difficulty_raw: f32,
    /// Bit i: dialogue i (of [`DIALOGUE_FLAGS`]) already played.
    pub dialogue_played: u16,
    /// Key/value pairs the level's scripts keep.
    pub script_values: [(i32, i32); SCRIPT_VALUES],
    /// Bit i: clue bottle i collected.
    pub bottles: u64,
    /// Bit i: checkpoint i reached.
    pub checkpoints: u8,
}

impl Default for LevelRecord {
    fn default() -> Self {
        LevelRecord {
            flags: 0,
            time: 0.0,
            best_sprint: 0.0,
            difficulty: 0.0,
            difficulty_raw: 0.0,
            dialogue_played: 0,
            script_values: [(0, 0); SCRIPT_VALUES],
            bottles: 0,
            checkpoints: 0,
        }
    }
}

/// One episode's record.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EpisodeRecord {
    pub levels: [LevelRecord; LEVELS_PER_EPISODE],
    pub keys: u8,
    pub vaults: u8,
    pub sprints: u8,
    /// Seconds spent in the episode.
    pub time: f32,
    /// [`episode_flags`].
    pub flags: u8,
}

/// Everything one save slot holds.
#[derive(Clone, Debug, PartialEq)]
pub struct SlotState {
    /// Total play time in seconds.
    pub play_time: f64,
    /// Where Load resumes.
    pub episode: u8,
    pub level: u8,
    /// -1 means game over.
    pub lives: i8,
    pub charms: u8,
    pub coins: u8,
    pub settings: Settings,
    /// One bit per power-up won from vaults.
    pub reward_mask: u32,
    pub unlocked_movies: u32,
    /// [`milestones`].
    pub milestones: u8,
    pub last_power_up: Option<u8>,
    pub episodes: [EpisodeRecord; EPISODES],
}

impl Default for SlotState {
    fn default() -> Self {
        SlotState::new_game()
    }
}

fn clamp_f32(v: f32, lo: f32, hi: f32) -> f32 {
    if v.is_nan() { lo } else { v.clamp(lo, hi) }
}

fn time_f32(v: f32) -> f32 {
    if v.is_finite() && v > 0.0 { v } else { 0.0 }
}

/// What a newly collected clue bottle reached.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BottleResult {
    /// The bottle was not collected before.
    pub new: bool,
    /// Milestones reached by this bottle ([`milestones`] bits), for their one-time dialogues.
    pub milestones: u8,
    /// Every bottle of the level is now collected (the vault combination is complete).
    pub level_complete: bool,
}

/// What collecting coins gave.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoinResult {
    pub charms: u32,
    pub lives: u32,
}

/// What finishing a Master Thief Sprint did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SprintResult {
    pub first_completion: bool,
    pub new_best: bool,
}

/// What a level contains on entry, from the saved goal flags (save-format.md 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelEntry {
    /// Place the level's key (not yet collected).
    pub key_present: bool,
    /// The vault is already open.
    pub vault_open: bool,
    /// Bit i set: clue bottle i is placed (not yet collected).
    pub bottles_to_place: u64,
}

impl SlotState {
    /// A new game: the prologue, five lives, no rewards, no power-up selected (save-format.md 3). Settings start
    /// with everything off except what the caller carries over.
    pub fn new_game() -> SlotState {
        SlotState {
            play_time: 0.0,
            episode: 0,
            level: 0,
            lives: STARTING_LIVES,
            charms: 0,
            coins: 0,
            settings: Settings(0),
            reward_mask: 0,
            unlocked_movies: 0,
            milestones: 0,
            last_power_up: None,
            episodes: [EpisodeRecord::default(); EPISODES],
        }
    }

    /// Brings every value into its range (save-format.md 5: values outside the ranges are clamped on load).
    pub fn clamp(&mut self) {
        self.play_time = if self.play_time.is_finite() && self.play_time > 0.0 { self.play_time } else { 0.0 };
        if self.episode as usize >= EPISODES || self.level as usize >= LEVELS_PER_EPISODE {
            self.episode = 0;
            self.level = 0;
        }
        self.lives = self.lives.clamp(MIN_LIVES, MAX_LIVES);
        self.charms = self.charms.min(MAX_CHARMS);
        self.coins = self.coins.min(MAX_COINS);
        self.milestones &= milestones::ALL;
        if self.last_power_up.is_some_and(|p| p >= MAX_POWER_UPS) {
            self.last_power_up = None;
        }
        for e in &mut self.episodes {
            e.flags &= episode_flags::ALL;
            e.time = time_f32(e.time);
            for l in &mut e.levels {
                l.flags &= level_flags::ALL;
                l.time = time_f32(l.time);
                l.best_sprint = time_f32(l.best_sprint);
                l.difficulty = clamp_f32(l.difficulty, 0.0, 1.0);
                l.difficulty_raw = clamp_f32(l.difficulty_raw, 0.0, 1.0);
                l.dialogue_played &= (1 << DIALOGUE_FLAGS) - 1;
            }
            // The counters cannot exceed the levels that have the matching goal flag.
            let count = |bit: u8| e.levels.iter().filter(|l| l.flags & bit != 0).count() as u8;
            e.keys = e.keys.min(count(level_flags::KEY));
            e.vaults = e.vaults.min(count(level_flags::VAULT));
            e.sprints = e.sprints.min(count(level_flags::SPRINT));
        }
    }

    fn check(episode: u8, level: u8) -> (usize, usize) {
        assert!(
            (episode as usize) < EPISODES && (level as usize) < LEVELS_PER_EPISODE,
            "episode {episode} level {level} out of range"
        );
        (episode as usize, level as usize)
    }

    pub fn level(&self, episode: u8, level: u8) -> &LevelRecord {
        let (e, l) = Self::check(episode, level);
        &self.episodes[e].levels[l]
    }

    pub fn level_mut(&mut self, episode: u8, level: u8) -> &mut LevelRecord {
        let (e, l) = Self::check(episode, level);
        &mut self.episodes[e].levels[l]
    }

    /// Entering a level: marks it and its episode visited and makes it the resume point.
    pub fn visit(&mut self, episode: u8, level: u8) {
        let (e, _) = Self::check(episode, level);
        self.level_mut(episode, level).flags |= level_flags::VISITED;
        self.episodes[e].flags |= episode_flags::VISITED;
        self.episode = episode;
        self.level = level;
    }

    /// Adds play time to the slot, the episode and the level.
    pub fn add_time(&mut self, episode: u8, level: u8, seconds: f32) {
        let s = time_f32(seconds);
        self.play_time += s as f64;
        let (e, _) = Self::check(episode, level);
        self.episodes[e].time += s;
        self.level_mut(episode, level).time += s;
    }

    /// Collects the level's key. Returns `true` the first time (the caller plays the celebration; auto-save follows).
    pub fn collect_key(&mut self, episode: u8, level: u8) -> bool {
        let (e, _) = Self::check(episode, level);
        let rec = self.level_mut(episode, level);
        if rec.flags & level_flags::KEY != 0 {
            return false;
        }
        rec.flags |= level_flags::KEY;
        self.episodes[e].keys = self.episodes[e].keys.saturating_add(1);
        true
    }

    /// Total vaults opened in all episodes.
    pub fn vaults_opened(&self) -> u32 {
        self.episodes.iter().flat_map(|e| &e.levels).filter(|l| l.flags & level_flags::VAULT != 0).count() as u32
    }

    /// Opens the level's vault and grants `reward_bit` (a power-up). Returns the milestones newly reached (first
    /// and second vault), or `None` when the vault was already open.
    pub fn open_vault(&mut self, episode: u8, level: u8, reward_bit: Option<u8>) -> Option<u8> {
        let (e, _) = Self::check(episode, level);
        let rec = self.level_mut(episode, level);
        if rec.flags & level_flags::VAULT != 0 {
            return None;
        }
        rec.flags |= level_flags::VAULT;
        self.episodes[e].vaults = self.episodes[e].vaults.saturating_add(1);
        if let Some(b) = reward_bit.filter(|&b| b < MAX_POWER_UPS) {
            self.reward_mask |= 1 << b;
        }
        let before = self.milestones;
        match self.vaults_opened() {
            1 => self.milestones |= milestones::FIRST_VAULT,
            2 => self.milestones |= milestones::SECOND_VAULT,
            _ => {}
        }
        Some(self.milestones & !before)
    }

    /// Collects clue bottle `index` of a level holding `total` bottles.
    pub fn collect_bottle(&mut self, episode: u8, level: u8, index: usize, total: usize) -> BottleResult {
        assert!(index < BOTTLES && total <= BOTTLES);
        let rec = self.level_mut(episode, level);
        if rec.bottles & (1 << index) != 0 {
            return BottleResult::default();
        }
        rec.bottles |= 1 << index;
        let count = rec.bottles.count_ones() as usize;
        let before = self.milestones;
        self.milestones |= milestones::FIRST_BOTTLE;
        if total > 0 && count * 2 >= total {
            self.milestones |= milestones::HALF_BOTTLES;
        }
        let level_complete = total > 0 && count >= total;
        if level_complete {
            self.milestones |= milestones::ALL_BOTTLES;
        }
        BottleResult { new: true, milestones: self.milestones & !before, level_complete }
    }

    /// Records a finished Master Thief Sprint of `seconds`.
    pub fn finish_sprint(&mut self, episode: u8, level: u8, seconds: f32) -> SprintResult {
        let (e, _) = Self::check(episode, level);
        let seconds = time_f32(seconds);
        let rec = self.level_mut(episode, level);
        let first = rec.flags & level_flags::SPRINT == 0;
        let new_best = first || (seconds > 0.0 && seconds < rec.best_sprint);
        rec.flags |= level_flags::SPRINT;
        if new_best {
            rec.best_sprint = seconds;
        }
        if first {
            self.episodes[e].sprints = self.episodes[e].sprints.saturating_add(1);
        }
        SprintResult { first_completion: first, new_best }
    }

    /// Records a defeated boss in `level`; the episode counts as finished. Returns `true` the first time.
    pub fn defeat_boss(&mut self, episode: u8, level: u8) -> bool {
        let (e, _) = Self::check(episode, level);
        let rec = self.level_mut(episode, level);
        let first = rec.flags & level_flags::BOSS == 0;
        rec.flags |= level_flags::BOSS;
        self.episodes[e].flags |= episode_flags::FINISHED;
        first
    }

    pub fn reach_checkpoint(&mut self, episode: u8, level: u8, index: usize) {
        assert!(index < CHECKPOINTS);
        self.level_mut(episode, level).checkpoints |= 1 << index;
    }

    /// Where a restart after losing a life puts the player: the highest-numbered checkpoint reached, or `None` for
    /// the level's entry. (Checkpoints are numbered along the level; the original's choice among several reached
    /// ones is inferred.)
    pub fn restart_checkpoint(&self, episode: u8, level: u8) -> Option<u8> {
        let c = self.level(episode, level).checkpoints;
        if c == 0 { None } else { Some(7 - c.leading_zeros() as u8) }
    }

    pub fn dialogue_played(&self, episode: u8, level: u8, index: usize) -> bool {
        index < DIALOGUE_FLAGS && self.level(episode, level).dialogue_played & (1 << index) != 0
    }

    pub fn set_dialogue_played(&mut self, episode: u8, level: u8, index: usize) {
        assert!(index < DIALOGUE_FLAGS);
        self.level_mut(episode, level).dialogue_played |= 1 << index;
    }

    /// What a level contains on entry (save-format.md 3): no key once collected, an open vault once opened, only
    /// the bottles not yet collected (`placed` = the bottles the level places).
    pub fn level_entry(&self, episode: u8, level: u8, placed: u64) -> LevelEntry {
        let rec = self.level(episode, level);
        LevelEntry {
            key_present: rec.flags & level_flags::KEY == 0,
            vault_open: rec.flags & level_flags::VAULT != 0,
            bottles_to_place: placed & !rec.bottles,
        }
    }

    /// A life lost. Returns `true` when that means game over (lives below 0).
    pub fn lose_life(&mut self) -> bool {
        self.lives = (self.lives - 1).max(MIN_LIVES);
        self.lives < 0
    }

    /// "Try again?" after game over: back to the starting lives; saved progress is kept (save-format.md 3).
    pub fn continue_after_game_over(&mut self) {
        self.lives = STARTING_LIVES;
    }

    /// Collects `n` coins one at a time (pickups-and-progress.md 3.1): when the counter passes 99 it resets to 0
    /// and gives a charm if fewer than 2 are held, otherwise a life if lives are below 99; otherwise it stays at 99.
    pub fn add_coins(&mut self, n: u32) -> CoinResult {
        let mut r = CoinResult::default();
        for _ in 0..n {
            if self.coins < MAX_COINS {
                self.coins += 1;
            } else if self.charms < MAX_CHARMS {
                self.coins = 0;
                self.charms += 1;
                r.charms += 1;
            } else if self.lives < MAX_LIVES {
                self.coins = 0;
                self.lives += 1;
                r.lives += 1;
            }
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_values() {
        let s = SlotState::new_game();
        assert_eq!((s.episode, s.level, s.lives, s.charms, s.coins), (0, 0, 5, 0, 0));
        assert_eq!((s.reward_mask, s.last_power_up), (0, None));
        assert!(!s.settings.autosave());
        assert!(s.settings.vibration() && s.settings.music() && !s.settings.mono());
    }

    #[test]
    fn clamping() {
        let mut s = SlotState::new_game();
        s.play_time = f64::NAN;
        s.episode = 9;
        s.lives = -50;
        s.charms = 7;
        s.coins = 250;
        s.milestones = 0xFF;
        s.last_power_up = Some(40);
        let l = &mut s.episodes[2].levels[3];
        l.flags = 0xFF;
        l.difficulty = 3.0;
        l.difficulty_raw = f32::NAN;
        l.time = -4.0;
        l.dialogue_played = 0xFFFF;
        s.episodes[2].keys = 9; // only one level has the key flag
        s.clamp();
        assert_eq!((s.play_time, s.episode, s.level, s.lives, s.charms, s.coins), (0.0, 0, 0, -1, 2, 99));
        assert_eq!((s.milestones, s.last_power_up), (0x1F, None));
        let l = &s.episodes[2].levels[3];
        assert_eq!((l.flags, l.difficulty, l.difficulty_raw, l.time, l.dialogue_played), (0x1F, 1.0, 0.0, 0.0, 0xFFF));
        assert_eq!(s.episodes[2].keys, 1);
    }

    #[test]
    fn goals_and_entry_rules() {
        let mut s = SlotState::new_game();
        s.visit(1, 2);
        assert_eq!((s.episode, s.level), (1, 2));
        assert!(s.episodes[1].flags & episode_flags::VISITED != 0);
        assert!(s.collect_key(1, 2));
        assert!(!s.collect_key(1, 2), "a key counts once");
        assert_eq!(s.episodes[1].keys, 1);
        assert_eq!(s.open_vault(1, 2, Some(3)), Some(milestones::FIRST_VAULT));
        assert_eq!(s.open_vault(1, 2, Some(3)), None);
        assert_eq!(s.open_vault(1, 4, None), Some(milestones::SECOND_VAULT));
        assert_eq!(s.open_vault(2, 0, None), Some(0));
        assert_eq!(s.reward_mask, 1 << 3);
        let e = s.level_entry(1, 2, 0b1111);
        assert!(!e.key_present && e.vault_open);
        s.collect_bottle(1, 2, 1, 4);
        assert_eq!(s.level_entry(1, 2, 0b1111).bottles_to_place, 0b1101);
        assert!(s.level_entry(1, 3, 0).key_present);
    }

    #[test]
    fn bottle_milestones() {
        let mut s = SlotState::new_game();
        assert_eq!(s.collect_bottle(1, 0, 0, 4).milestones, milestones::FIRST_BOTTLE);
        assert_eq!(s.collect_bottle(1, 0, 0, 4), BottleResult::default(), "same bottle again");
        assert_eq!(s.collect_bottle(1, 0, 1, 4).milestones, milestones::HALF_BOTTLES);
        s.collect_bottle(1, 0, 2, 4);
        let last = s.collect_bottle(1, 0, 3, 4);
        assert!(last.level_complete && last.milestones == milestones::ALL_BOTTLES);
    }

    #[test]
    fn sprints_bosses_checkpoints_lives() {
        let mut s = SlotState::new_game();
        assert_eq!(s.finish_sprint(3, 1, 90.0), SprintResult { first_completion: true, new_best: true });
        assert_eq!(s.finish_sprint(3, 1, 95.0), SprintResult { first_completion: false, new_best: false });
        assert_eq!(s.finish_sprint(3, 1, 80.0), SprintResult { first_completion: false, new_best: true });
        assert_eq!((s.level(3, 1).best_sprint, s.episodes[3].sprints), (80.0, 1));
        assert!(s.defeat_boss(3, 8));
        assert!(!s.defeat_boss(3, 8));
        assert!(s.episodes[3].flags & episode_flags::FINISHED != 0);
        assert_eq!(s.restart_checkpoint(3, 1), None);
        s.reach_checkpoint(3, 1, 2);
        s.reach_checkpoint(3, 1, 5);
        assert_eq!(s.restart_checkpoint(3, 1), Some(5));
        for _ in 0..5 {
            assert!(!s.lose_life());
        }
        assert!(s.lose_life(), "the sixth life lost is game over");
        assert_eq!(s.lives, -1);
        s.continue_after_game_over();
        assert_eq!(s.lives, 5);
        // 100 coins: a charm while fewer than 2 are held, then lives.
        assert_eq!(s.add_coins(99), CoinResult::default());
        assert_eq!(s.add_coins(1), CoinResult { charms: 1, lives: 0 });
        assert_eq!((s.coins, s.charms), (0, 1));
        assert_eq!(s.add_coins(300), CoinResult { charms: 1, lives: 2 });
        assert_eq!((s.coins, s.charms, s.lives), (0, 2, 7));
        s.lives = MAX_LIVES;
        s.coins = 95;
        s.add_coins(10);
        assert_eq!((s.coins, s.lives), (99, MAX_LIVES), "at 99 lives the counter stays at 99");
        s.add_time(3, 1, 2.5);
        s.add_time(3, 1, f32::NAN);
        assert_eq!((s.play_time, s.episodes[3].time, s.level(3, 1).time), (2.5, 2.5, 2.5));
        s.set_dialogue_played(3, 1, 11);
        assert!(s.dialogue_played(3, 1, 11) && !s.dialogue_played(3, 1, 12));
    }
}
