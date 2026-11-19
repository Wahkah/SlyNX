//! Percent complete (pickups-and-progress.md 6) and the slot description shown in the save, load and new-game
//! menus (save-format.md 4, ui-and-menus.md 2).

use crate::model::{SlotState, level_flags};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Which tasks one level contributes to the percentage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelTasks {
    pub episode: u8,
    pub level: u8,
    /// Visiting the level is a task (every level outside the prologue).
    pub visit: bool,
    /// The level's goals: the world table's task mask (key bit 1, vault bit 2, sprint bit 3).
    pub task_mask: u8,
    /// Beating this level's boss is a task (the five episode bosses).
    pub boss: bool,
}

/// The task list of the game, built once from the world table.
#[derive(Clone, Debug, Default)]
pub struct CompletionTable {
    pub levels: Vec<LevelTasks>,
}

const GOALS: u8 = level_flags::KEY | level_flags::VAULT | level_flags::SPRINT;

impl CompletionTable {
    /// Builds the table from `(episode, level, task_mask)` of every world and the boss levels. Visits count for
    /// every level outside episode 0 (the prologue).
    pub fn new(worlds: impl IntoIterator<Item = (u8, u8, u8)>, boss_levels: &[(u8, u8)]) -> CompletionTable {
        let levels = worlds
            .into_iter()
            .map(|(episode, level, task_mask)| LevelTasks {
                episode,
                level,
                visit: episode != 0,
                task_mask: task_mask & GOALS,
                boss: boss_levels.contains(&(episode, level)),
            })
            .collect();
        CompletionTable { levels }
    }

    /// Number of tasks.
    pub fn total(&self) -> u32 {
        self.levels.iter().map(|l| l.visit as u32 + l.task_mask.count_ones() + l.boss as u32).sum()
    }

    /// Tasks done in `state`.
    pub fn done(&self, state: &SlotState) -> u32 {
        self.levels
            .iter()
            .filter(|l| {
                (l.episode as usize) < crate::model::EPISODES && (l.level as usize) < crate::model::LEVELS_PER_EPISODE
            })
            .map(|l| {
                let flags = state.level(l.episode, l.level).flags;
                let visit = l.visit && flags & level_flags::VISITED != 0;
                let goals = (flags & l.task_mask).count_ones();
                let boss = l.boss && flags & level_flags::BOSS != 0;
                visit as u32 + goals + boss as u32
            })
            .sum()
    }

    /// Percent complete as displayed: 0 when nothing is done, 100 when everything is, otherwise
    /// `floor(100 * done / total)` clamped to 1..=99.
    pub fn percent(&self, state: &SlotState) -> u32 {
        percent(self.done(state), self.total())
    }
}

/// The display rule on its own.
pub fn percent(done: u32, total: u32) -> u32 {
    if done == 0 || total == 0 {
        0
    } else if done >= total {
        100
    } else {
        (100 * done as u64 / total as u64).clamp(1, 99) as u32
    }
}

/// What a slot shows in the slot list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotSummary {
    Empty,
    /// A saved game: percent complete, the name of the world being played, play time in seconds.
    Game {
        percent: u32,
        world: String,
        play_seconds: u64,
    },
    /// The file exists but cannot be read (damaged, or written by a newer SlyNX). A SlyNX addition: the original
    /// had no such case.
    Unreadable,
}

impl SlotSummary {
    pub fn of(state: &SlotState, table: &CompletionTable, world_name: &str) -> SlotSummary {
        SlotSummary::Game {
            percent: table.percent(state),
            world: String::from(world_name),
            play_seconds: if state.play_time.is_finite() && state.play_time > 0.0 { state.play_time as u64 } else { 0 },
        }
    }

    /// The slot text: `"Empty"`, or `"%d%% - %s - %d:%02d"` (percent, world, hours:minutes).
    pub fn text(&self) -> String {
        match self {
            SlotSummary::Empty => String::from("Empty"),
            SlotSummary::Game { percent, world, play_seconds } => {
                let minutes = play_seconds / 60;
                format!("{percent}% - {world} - {}:{:02}", minutes / 60, minutes % 60)
            }
            SlotSummary::Unreadable => String::from("Damaged save"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The task masks of the 46 worlds (pickups-and-progress.md 8), in world order. The test gives them episodes
    /// and level numbers of its own (3 prologue worlds, then 43 across episodes 1 to 5) and picks five boss levels.
    const MASKS: [u8; 46] = [
        0, 0, 0, 0xE, 0, 0xE, 0xE, 0xE, 0xE, 2, 0xE, 0, 0xE, 0, 0xE, 2, 2, 0xE, 0xE, 0xE, 0, 0xE, 0, 0xE, 0xE, 2, 0xE,
        2, 2, 0, 0xE, 0, 0xE, 0xE, 2, 2, 0xE, 2, 0, 0, 0, 0, 0, 0, 0, 0,
    ];

    fn table() -> CompletionTable {
        let worlds = MASKS
            .iter()
            .enumerate()
            .map(|(i, &m)| if i < 3 { (0, i as u8, m) } else { (1 + ((i - 3) / 9) as u8, ((i - 3) % 9) as u8, m) });
        CompletionTable::new(worlds, &[(1, 8), (2, 8), (3, 8), (4, 8), (5, 6)])
    }

    #[test]
    fn total_is_114() {
        // 43 visits + 19 x 3 + 9 goals + 5 bosses.
        assert_eq!(table().total(), 114);
    }

    #[test]
    fn percent_rule() {
        assert_eq!(percent(0, 114), 0);
        assert_eq!(percent(1, 114), 1, "anything done shows at least 1");
        assert_eq!(percent(57, 114), 50);
        assert_eq!(percent(113, 114), 99, "not everything done shows at most 99");
        assert_eq!(percent(114, 114), 100);

        let t = table();
        let mut s = SlotState::new_game();
        assert_eq!(t.percent(&s), 0);
        s.visit(0, 1); // prologue visits are not tasks
        assert_eq!(t.done(&s), 0);
        s.visit(1, 0);
        s.collect_key(1, 0);
        s.open_vault(1, 0, None);
        s.finish_sprint(1, 0, 60.0);
        s.defeat_boss(1, 8);
        assert_eq!(t.done(&s), 5);
        for l in &t.levels {
            let r = s.level_mut(l.episode, l.level);
            r.flags = 0x1F;
        }
        assert_eq!(t.percent(&s), 100);
    }

    #[test]
    fn slot_text() {
        assert_eq!(SlotSummary::Empty.text(), "Empty");
        let g = SlotSummary::Game { percent: 37, world: "Paris, France".into(), play_seconds: 3 * 3600 + 7 * 60 + 59 };
        assert_eq!(g.text(), "37% - Paris, France - 3:07");
        let mut s = SlotState::new_game();
        s.play_time = 125.9;
        assert_eq!(SlotSummary::of(&s, &table(), "The Hideout").text(), "0% - The Hideout - 0:02");
    }
}
