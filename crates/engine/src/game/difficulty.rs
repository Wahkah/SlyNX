//! The adaptive difficulty level and breakable rewards (pickups-and-progress.md 3.4).
//!
//! The tables are read from the player's executable after checking its SHA-256: four 96-byte settings records at
//! `0x261D70` + k × `0x60` and four 12-byte reward ranges at `0x261D40`.

use alloc::vec::Vec;

use formats::disc::Executable;

use super::rng::Rng;

/// Address of the reward ranges.
pub const REWARDS_VA: u32 = 0x0026_1D40;
/// Address of the first settings record.
pub const SETTINGS_VA: u32 = 0x0026_1D70;
/// Size of a settings record.
pub const SETTINGS_SIZE: u32 = 0x60;

/// One settings record.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Settings {
    /// Allowed range of the level value.
    pub range: [f32; 2],
    /// Range applied when the world loads.
    pub load: [f32; 2],
    /// Change on death.
    pub death: f32,
    /// Change on a new checkpoint.
    pub new_checkpoint: f32,
    /// Change on a checkpoint reached again.
    pub repeat_checkpoint: f32,
    /// Breaks before a rich drop: min and max at level 0, min and max at level 1.
    pub rich_breaks: [i32; 4],
    /// (Level threshold, charms given on load), low and high.
    pub charms: [(f32, i32); 2],
    /// Raw change on death, on game over.
    pub raw_death: [f32; 2],
    /// Raw change when a life-token reward is given, raw change per break.
    pub raw_life: f32,
    /// See [`Settings::raw_life`].
    pub raw_break: f32,
    /// Low-lives count and the extra raw change on death at or below it.
    pub low_lives: (i32, f32),
}

/// The tables.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DifficultyTables {
    /// The four settings records, in address order.
    pub settings: [Settings; 4],
    /// Reward ranges: (raw-level threshold, min, max), in table order.
    pub rewards: Vec<(f32, i32, i32)>,
}

fn f(b: &[u8], o: usize) -> f32 {
    f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn i(b: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

impl DifficultyTables {
    /// Reads the tables, refusing an executable other than the supported one.
    pub fn from_executable(exe: &Executable) -> Option<DifficultyTables> {
        if !exe.is_supported() {
            return None;
        }
        let mut t = DifficultyTables::default();
        for k in 0..4 {
            let b = exe.bytes_at(SETTINGS_VA + k * SETTINGS_SIZE, SETTINGS_SIZE as usize).ok()?;
            t.settings[k as usize] = Settings {
                range: [f(b, 0), f(b, 4)],
                load: [f(b, 8), f(b, 12)],
                death: f(b, 16),
                new_checkpoint: f(b, 20),
                repeat_checkpoint: f(b, 24),
                rich_breaks: [i(b, 28), i(b, 32), i(b, 36), i(b, 40)],
                charms: [(f(b, 44), i(b, 48)), (f(b, 52), i(b, 56))],
                raw_death: [f(b, 60), f(b, 64)],
                raw_life: f(b, 68),
                raw_break: f(b, 72),
                low_lives: (i(b, 76), f(b, 80)),
            };
        }
        let n = i(exe.bytes_at(SETTINGS_VA + 84, 4).ok()?, 0).clamp(0, 16) as u32;
        let b = exe.bytes_at(REWARDS_VA, (n * 12) as usize).ok()?;
        t.rewards = (0..n as usize).map(|k| (f(b, k * 12), i(b, k * 12 + 4), i(b, k * 12 + 8))).collect();
        Some(t)
    }

    /// Which record applies (pickups-and-progress.md 3.4): the last in the prologue, the hubs and the final
    /// world (`task_mask` 0); the first in a level whose key is not collected; the third after the key.
    pub fn record(&self, task_mask: u32, key_collected: bool) -> &Settings {
        if task_mask == 0 {
            &self.settings[3]
        } else if key_collected {
            &self.settings[2]
        } else {
            &self.settings[0]
        }
    }
}

/// The per-level difficulty state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LevelDifficulty {
    /// Level 0..1.
    pub level: f32,
    /// Raw value 0..1.
    pub raw: f32,
    /// Breaks left before a rich drop.
    pub rich_countdown: i32,
    /// Breaks left before a life-token reward.
    pub life_countdown: i32,
}

/// What a break that lets the difficulty decide drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drop {
    /// Coins.
    pub coins: i32,
    /// A life token instead.
    pub life: bool,
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn draw_int(rng: &mut Rng, lo: i32, hi: i32) -> i32 {
    if hi <= lo {
        return lo;
    }
    lo + (rng.next_u32() % (hi - lo + 1) as u32) as i32
}

impl LevelDifficulty {
    /// One counted break with the coin count left to the difficulty (option 233 negative): a rich drop of 4 to 6
    /// coins when the rich countdown runs out, else 1 to 3, or a life token when the reward countdown runs out
    /// (only with a non-zero raw level). Each counted break lowers the raw level.
    pub fn on_break(&mut self, s: &Settings, rewards: &[(f32, i32, i32)], rng: &mut Rng) -> Drop {
        let mut drop = Drop { coins: 0, life: false };
        if self.rich_countdown <= 0 {
            let lo = lerp(s.rich_breaks[0] as f32, s.rich_breaks[2] as f32, self.level) as i32;
            let hi = lerp(s.rich_breaks[1] as f32, s.rich_breaks[3] as f32, self.level) as i32;
            self.rich_countdown = draw_int(rng, lo, hi.max(lo));
        }
        self.rich_countdown -= 1;
        drop.coins = if self.rich_countdown <= 0 { draw_int(rng, 4, 6) } else { draw_int(rng, 1, 3) };
        if self.raw > 0.0 {
            if self.life_countdown <= 0
                && let Some(&(_, lo, hi)) = rewards.iter().find(|r| self.raw >= r.0)
            {
                self.life_countdown = draw_int(rng, lo, hi);
            }
            self.life_countdown -= 1;
            if self.life_countdown == 0 {
                drop = Drop { coins: 0, life: true };
                self.raw = (self.raw + s.raw_life).clamp(0.0, 1.0);
            }
        }
        self.raw = (self.raw + s.raw_break).clamp(0.0, 1.0);
        drop
    }

    /// A checkpoint reached (new or again).
    pub fn on_checkpoint(&mut self, s: &Settings, again: bool) {
        let d = if again { s.repeat_checkpoint } else { s.new_checkpoint };
        self.level = (self.level + d).clamp(s.range[0], s.range[1]);
    }

    /// A death, with the lives left.
    pub fn on_death(&mut self, s: &Settings, lives: i32, game_over: bool) {
        self.level = (self.level + s.death).clamp(s.range[0], s.range[1]);
        let mut raw = s.raw_death[usize::from(game_over)];
        if lives <= s.low_lives.0 {
            raw += s.low_lives.1;
        }
        self.raw = (self.raw + raw).clamp(0.0, 1.0);
    }

    /// The charms given after a restart: the high count at or above the high threshold, the low one at or above
    /// the low threshold.
    pub fn charms_on_load(&self, s: &Settings) -> i32 {
        if self.level >= s.charms[1].0 {
            s.charms[1].1
        } else if self.level >= s.charms[0].0 {
            s.charms[0].1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> Settings {
        Settings {
            range: [0.0, 1.0],
            load: [0.0, 0.6],
            death: 0.1,
            new_checkpoint: -0.8,
            repeat_checkpoint: 0.0,
            rich_breaks: [4, 6, 0, 2],
            charms: [(0.399, 1), (0.799, 2)],
            raw_death: [0.075, 0.25],
            raw_life: -0.2,
            raw_break: -0.003,
            low_lives: (2, 0.075),
        }
    }

    #[test]
    fn drops_and_levels() {
        let s = record();
        let rewards = [(0.8, 2, 5), (0.4, 6, 10), (0.2, 15, 25), (0.0, 50, 100)];
        let mut d = LevelDifficulty::default();
        let mut rng = Rng::new(3);
        let drops: Vec<Drop> = (0..12).map(|_| d.on_break(&s, &rewards, &mut rng)).collect();
        assert!(drops.iter().all(|x| !x.life), "no life tokens at raw level 0");
        assert!(drops.iter().any(|x| x.coins >= 4), "a rich drop within 6 breaks");
        assert!(drops.iter().all(|x| (1..=6).contains(&x.coins)));
        d.on_death(&s, 2, false);
        assert!((d.level - 0.1).abs() < 1e-6 && (d.raw - 0.15).abs() < 1e-6);
        d.level = 0.85;
        assert_eq!(d.charms_on_load(&s), 2);
        d.on_checkpoint(&s, false);
        assert!((d.level - 0.05).abs() < 1e-5);
        assert_eq!(d.charms_on_load(&s), 0);
        d.raw = 0.9;
        d.life_countdown = 1;
        let x = d.on_break(&s, &rewards, &mut rng);
        assert!(x.life);
    }
}
