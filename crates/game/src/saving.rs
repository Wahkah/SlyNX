//! Saving: the engine's [`Progress`] in the `save` crate's slots (docs/save.md).
//!
//! - **Slot files** go through [`SlotFiles`], which [`save::SaveStore`] implements over any
//!   `platform_api::FileSystem`: the PC's user folder or `sdmc:/switch/SlyNX/user/` on the Switch.
//! - **Tracking.** Every frame [`Saves::track`] compares the progress with the slot state: a newly collected key, an
//!   opened vault, a finished sprint or a defeated boss goes through the save session's events, which decide whether
//!   to auto-save (save-format.md 2); bottles, checkpoints, the adaptive difficulty, lives, charms and coins are
//!   copied. Loading a world other than the one last saved also auto-saves.
//! - **Restoring.** [`restore`] turns a loaded slot back into a [`Progress`]; the game resumes in the slot's world.
//!
//! What the slot format does not hold (collected coins and other placed pickups, broken breakables, knocked-out
//! guards, played dialogues by anchor) is kept only while the game runs (save-format.md 3, open question 1).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use engine::game::progress::{Progress, level_flag};
use formats::disc::WorldEntry;
use platform_api::{FileSystem, FsError};
use save::model::{EPISODES, LEVELS_PER_EPISODE, episode_flags};
use save::{CompletionTable, LoadError, Loaded, PortSettings, SaveRequest, SaveStore, Slot, SlotState, SlotSummary};

/// Seconds the auto-save icon stays up after a save.
pub const ICON_SECONDS: f32 = 2.0;

/// The save files of one game, as the session uses them.
pub trait SlotFiles {
    /// Loads a slot (falling back to its backup).
    fn load(&self, slot: Slot) -> Result<Loaded, LoadError>;
    /// Saves a slot (keeping the previous good save as the backup).
    fn save(&self, slot: Slot, state: &SlotState) -> Result<(), FsError>;
    /// Erases a slot and its backup.
    fn erase(&self, slot: Slot) -> Result<(), FsError>;
    /// The port's options.
    fn load_settings(&self) -> PortSettings;
    /// Writes the port's options.
    fn save_settings(&self, settings: &PortSettings) -> Result<(), FsError>;
}

impl<F: FileSystem> SlotFiles for SaveStore<F> {
    fn load(&self, slot: Slot) -> Result<Loaded, LoadError> {
        SaveStore::load(self, slot)
    }
    fn save(&self, slot: Slot, state: &SlotState) -> Result<(), FsError> {
        SaveStore::save(self, slot, state)
    }
    fn erase(&self, slot: Slot) -> Result<(), FsError> {
        SaveStore::erase(self, slot)
    }
    fn load_settings(&self) -> PortSettings {
        SaveStore::load_settings(self)
    }
    fn save_settings(&self, settings: &PortSettings) -> Result<(), FsError> {
        SaveStore::save_settings(self, settings)
    }
}

/// A world id (high byte episode, low byte level) as a slot position, if the slot format has room for it.
pub fn episode_level(world_id: u32) -> Option<(u8, u8)> {
    let (e, l) = ((world_id >> 8) as usize, (world_id & 0xFF) as usize);
    (e < EPISODES && l < LEVELS_PER_EPISODE).then_some((e as u8, l as u8))
}

/// The world id of a slot position.
pub fn world_id(episode: u8, level: u8) -> u32 {
    (u32::from(episode) << 8) | u32::from(level)
}

/// The percent-complete table of a disc: every world's goals, and each episode's last level as its boss level.
pub fn completion_table(worlds: &[WorldEntry]) -> CompletionTable {
    let mut bosses = Vec::new();
    for e in 1..EPISODES as u32 {
        if let Some(l) = worlds.iter().filter(|w| w.episode() == e).map(WorldEntry::level).max() {
            bosses.push((e as u8, l as u8));
        }
    }
    let levels = worlds.iter().filter_map(|w| episode_level(w.id).map(|(e, l)| (e, l, w.task_mask as u8)));
    CompletionTable::new(levels, &bosses)
}

/// The progress a slot holds: counters, rewards, and per level its goal flags, bottles, checkpoints, best sprint
/// and difficulty. The adaptive difficulty tables are not part of a slot; the caller keeps its own.
pub fn restore(state: &SlotState) -> Progress {
    let mut p = Progress::new_game();
    p.lives = i32::from(state.lives);
    p.charms = i32::from(state.charms);
    p.coins = i32::from(state.coins);
    p.rewards = state.reward_mask;
    if state.milestones & 1 != 0 {
        p.flags |= engine::game::progress::game_flag::FIRST_BOTTLE;
    }
    for (e, ep) in state.episodes.iter().enumerate() {
        if ep.keys > 0 {
            p.keys.insert(e as u32, u32::from(ep.keys));
        }
        if ep.vaults > 0 {
            p.vaults.insert(e as u32, u32::from(ep.vaults));
        }
        if ep.flags & episode_flags::FINISHED != 0 {
            p.episodes |= 1 << e;
        }
        for (l, rec) in ep.levels.iter().enumerate() {
            if rec.flags == 0 && rec.bottles == 0 && rec.checkpoints == 0 {
                continue;
            }
            let lr = p.level(world_id(e as u8, l as u8));
            lr.flags = u32::from(rec.flags);
            lr.bottles = rec.bottles;
            lr.checkpoints = rec.checkpoints;
            if rec.flags & level_flag::SPRINT as u8 != 0 {
                lr.best_elapsed = Some(rec.best_sprint);
            }
            lr.difficulty.level = rec.difficulty;
            lr.difficulty.raw = rec.difficulty_raw;
        }
    }
    p
}

/// The game's saving: the slot files, the save session (the slot state and when to save), the port's settings.
pub struct Saves {
    files: Box<dyn SlotFiles>,
    /// The game in progress as the slot format holds it.
    pub session: save::Session,
    /// The port's options (the last slot used).
    pub settings: PortSettings,
    /// Percent complete.
    pub table: CompletionTable,
    /// The last save or load error, for the status line.
    pub last_error: Option<String>,
    /// Seconds the auto-save icon still shows.
    pub icon: f32,
    /// A slot was loaded at the title (Start resumes it).
    pub loaded: bool,
    /// The auto-save notice has not been shown for the game in progress (a new game in a slot, or a loaded slot that
    /// never auto-saved): the session shows it before the first auto-save (`ui::menu::Menus::before_autosave`).
    pub notice_pending: bool,
    /// Auto-saves asked for and not yet written ([`Saves::take_requests`]).
    requests: Vec<SaveRequest>,
}

impl Saves {
    /// Saving through `files` for a disc with `worlds`. No slot is in use until one is loaded or chosen.
    pub fn new(files: Box<dyn SlotFiles>, worlds: &[WorldEntry]) -> Saves {
        let settings = files.load_settings();
        Saves {
            files,
            session: save::Session::new_game(None, save::Settings(0)),
            settings,
            table: completion_table(worlds),
            last_error: None,
            icon: 0.0,
            loaded: false,
            notice_pending: false,
            requests: Vec::new(),
        }
    }

    /// What each slot shows in the slot lists, for the menus.
    pub fn slot_infos(&self, worlds: &[WorldEntry]) -> [Option<ui::tables::SlotInfo>; 3] {
        Slot::ALL.map(|s| match self.files.load(s) {
            Ok(l) => {
                let name = worlds
                    .iter()
                    .find(|w| w.id == world_id(l.state.episode, l.state.level))
                    .map_or_else(String::new, |w| w.name.clone());
                match SlotSummary::of(&l.state, &self.table, &name) {
                    SlotSummary::Game { percent, world, play_seconds } => Some(ui::tables::SlotInfo {
                        percent,
                        world: world.into_bytes(),
                        minutes: (play_seconds / 60) as u32,
                    }),
                    _ => None,
                }
            }
            Err(_) => None,
        })
    }

    /// Loads the slot used last (at the title, so Start resumes it). Returns whether one was loaded.
    pub fn load_last(&mut self) -> bool {
        let Some(slot) = self.settings.last_slot.and_then(Slot::new) else { return false };
        self.load(slot).is_some()
    }

    /// Loads a slot; returns its progress and the world id it resumes in.
    pub fn load(&mut self, slot: Slot) -> Option<(Progress, u32)> {
        match self.files.load(slot) {
            Ok(l) => {
                let at = world_id(l.state.episode, l.state.level);
                let progress = restore(&l.state);
                // A slot whose game never accepted the auto-save notice shows it before its first auto-save.
                self.notice_pending = !l.state.settings.autosave();
                self.session = save::Session::loaded(slot, l.state);
                if self.notice_pending {
                    self.session.acknowledge_autosave_notice();
                }
                self.requests.clear();
                self.loaded = true;
                self.remember_slot(slot);
                if l.from_backup {
                    self.last_error = Some(format!("slot {}: damaged, loaded its backup", slot.index() + 1));
                }
                Some((progress, at))
            }
            Err(e) => {
                self.last_error = Some(format!("slot {}: {e}", slot.index() + 1));
                None
            }
        }
    }

    /// A new game in `slot` (or without one). With a slot, auto-saves are on and the notice is pending: the session
    /// shows it (prompt 20) before the first auto-save and writes that save once it is answered.
    pub fn new_game(&mut self, slot: Option<Slot>) {
        let settings = self.session.state.settings;
        self.session = save::Session::new_game(slot, settings);
        self.loaded = false;
        self.requests.clear();
        self.notice_pending = slot.is_some();
        if let Some(s) = slot {
            self.session.acknowledge_autosave_notice();
            self.remember_slot(s);
        }
    }

    fn remember_slot(&mut self, slot: Slot) {
        if self.settings.last_slot != Some(slot.index()) {
            self.settings.last_slot = Some(slot.index());
            if let Err(e) = self.files.save_settings(&self.settings) {
                self.last_error = Some(format!("settings: {e}"));
            }
        }
    }

    /// Erases a slot.
    pub fn erase(&mut self, slot: Slot) {
        if let Err(e) = self.files.erase(slot) {
            self.last_error = Some(format!("slot {}: {e}", slot.index() + 1));
        }
        self.session.slot_erased(slot);
    }

    /// Writes a save request; shows the icon.
    pub fn commit(&mut self, request: SaveRequest) {
        match self.files.save(request.slot, &self.session.state) {
            Ok(()) => {
                self.session.saved(&request);
                self.icon = ICON_SECONDS;
            }
            Err(e) => self.last_error = Some(format!("save: {e}")),
        }
    }

    /// A world was loaded (`resumable`: a level a save may resume in, not the start-up screen).
    pub fn world_loaded(&mut self, world: u32, resumable: bool) {
        let Some((e, l)) = episode_level(world).filter(|_| resumable) else { return };
        if let Some(r) = self.session.on_world_loaded(e, l) {
            self.requests.push(r);
        }
    }

    /// Brings the slot state up to date with `progress` in world `world` after `dt` seconds of play (0 while
    /// paused), and writes the saves its events ask for. `bottles` is the current world's clue-bottle count.
    pub fn track(&mut self, progress: &Progress, world: u32, dt: f32, bottles: usize) {
        self.icon = (self.icon - dt.max(0.0)).max(0.0);
        let mut requests: Vec<SaveRequest> = Vec::new();
        let s = &mut self.session;
        if let Some((e, l)) = episode_level(world)
            && dt > 0.0
        {
            s.state.add_time(e, l, dt);
        }
        for (&id, rec) in &progress.levels {
            let Some((e, l)) = episode_level(id) else { continue };
            let have = *s.state.level(e, l);
            let new = |f: u32| rec.flags & f != 0 && u32::from(have.flags) & f == 0;
            if new(level_flag::VISITED) {
                s.state.level_mut(e, l).flags |= level_flag::VISITED as u8;
                s.state.episodes[e as usize].flags |= episode_flags::VISITED;
            }
            if new(level_flag::KEY) {
                requests.extend(s.on_key_collected(e, l));
            }
            if new(level_flag::VAULT) {
                requests.extend(s.on_vault_opened(e, l, None).1);
            }
            if let Some(t) = rec.best_elapsed
                && rec.flags & level_flag::SPRINT != 0
                && (have.flags & level_flag::SPRINT as u8 == 0 || t < have.best_sprint)
            {
                requests.extend(s.on_sprint_finished(e, l, t).1);
            }
            if new(level_flag::BOSS) {
                requests.extend(s.on_boss_defeated(e, l));
            }
            let fresh = rec.bottles & !have.bottles;
            if fresh != 0 {
                let total = if id == world { bottles.clamp(1, 64) } else { 64 };
                for i in (0..64).filter(|i| fresh & (1 << i) != 0) {
                    s.state.collect_bottle(e, l, i, total);
                }
            }
            let reached = rec.checkpoints & !have.checkpoints;
            for i in (0..8).filter(|i| reached & (1 << i) != 0) {
                s.state.reach_checkpoint(e, l, i);
            }
            let lm = s.state.level_mut(e, l);
            lm.difficulty = rec.difficulty.level.clamp(0.0, 1.0);
            lm.difficulty_raw = rec.difficulty.raw.clamp(0.0, 1.0);
        }
        s.state.lives = progress.lives.clamp(-1, 99) as i8;
        s.state.charms = progress.charms.clamp(0, 2) as u8;
        s.state.coins = progress.coins.clamp(0, 99) as u8;
        s.state.reward_mask |= progress.rewards;
        self.requests.extend(requests);
    }

    /// The auto-saves asked for since the last call, to write with [`Saves::commit`] (after the notice, when it is
    /// pending).
    pub fn take_requests(&mut self) -> Vec<SaveRequest> {
        core::mem::take(&mut self.requests)
    }

    /// Writes every auto-save asked for (no notice shown).
    pub fn flush(&mut self) {
        for r in self.take_requests() {
            self.commit(r);
        }
    }

    /// "Save Game..." to `slot`.
    pub fn save_to(&mut self, slot: Slot) {
        let r = self.session.save_game(slot);
        self.remember_slot(slot);
        self.commit(r);
    }

    /// "Save and Quit": saves to the slot in use, if any.
    pub fn save_and_quit(&mut self) {
        if let Some(r) = self.session.save_and_quit() {
            self.commit(r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;
    use core::cell::RefCell;

    /// Slot files in memory.
    #[derive(Default)]
    struct Mem {
        slots: RefCell<BTreeMap<u8, SlotState>>,
        settings: RefCell<PortSettings>,
        writes: RefCell<u32>,
    }

    impl SlotFiles for alloc::rc::Rc<Mem> {
        fn load(&self, slot: Slot) -> Result<Loaded, LoadError> {
            let s = self.slots.borrow().get(&slot.index()).cloned().ok_or(LoadError::Empty)?;
            Ok(Loaded { state: s, version: 1, from_backup: false })
        }
        fn save(&self, slot: Slot, state: &SlotState) -> Result<(), FsError> {
            *self.writes.borrow_mut() += 1;
            self.slots.borrow_mut().insert(slot.index(), state.clone());
            Ok(())
        }
        fn erase(&self, slot: Slot) -> Result<(), FsError> {
            self.slots.borrow_mut().remove(&slot.index());
            Ok(())
        }
        fn load_settings(&self) -> PortSettings {
            self.settings.borrow().clone()
        }
        fn save_settings(&self, settings: &PortSettings) -> Result<(), FsError> {
            *self.settings.borrow_mut() = settings.clone();
            Ok(())
        }
    }

    fn saves() -> (Saves, alloc::rc::Rc<Mem>) {
        let mem = alloc::rc::Rc::new(Mem::default());
        (Saves::new(Box::new(mem.clone()), &[]), mem)
    }

    #[test]
    fn progress_round_trips_through_a_slot() {
        let (mut s, mem) = saves();
        s.new_game(Slot::new(1));
        s.world_loaded(0x103, true);
        assert!(s.notice_pending);
        s.flush();
        assert_eq!(*mem.writes.borrow(), 1, "a new world auto-saves once the slot is chosen");
        let mut p = Progress::new_game();
        p.level(0x103).flags |= level_flag::VISITED;
        p.collect_key(0x103);
        p.collect_bottle(0x103, 3, 10);
        p.reach_checkpoint(0x103, 2, Some(7));
        p.coins = 42;
        p.lives = 7;
        s.track(&p, 0x103, 1.0 / 60.0, 10);
        s.flush();
        assert_eq!(*mem.writes.borrow(), 2, "a new key auto-saves");
        s.track(&p, 0x103, 1.0 / 60.0, 10);
        s.flush();
        assert_eq!(*mem.writes.borrow(), 2, "nothing new, no save");
        let saved = mem.slots.borrow()[&1].clone();
        assert_eq!((saved.episode, saved.level, saved.coins, saved.lives), (1, 3, 42, 7));
        let r = restore(&saved);
        assert!(r.has_flag(0x103, level_flag::KEY));
        assert_eq!(r.level_ref(0x103).map(|l| (l.bottles, l.checkpoints)), Some((1 << 3, 1 << 2)));
        assert_eq!(r.keys.get(&1), Some(&1));
        assert_eq!((r.coins, r.lives), (42, 7));
        // The title loads the last slot used.
        let (mut t, _) = (Saves::new(Box::new(mem.clone()), &[]), ());
        assert!(t.load_last());
        assert_eq!(t.session.slot(), Slot::new(1));
    }

    #[test]
    fn no_slot_no_save_and_erase_forgets() {
        let (mut s, mem) = saves();
        s.new_game(None);
        s.world_loaded(0x103, true);
        let mut p = Progress::new_game();
        p.collect_key(0x103);
        s.track(&p, 0x103, 0.0, 0);
        s.flush();
        assert_eq!(*mem.writes.borrow(), 0);
        s.save_to(Slot::new(0).unwrap());
        assert_eq!(*mem.writes.borrow(), 1);
        s.erase(Slot::new(0).unwrap());
        assert!(mem.slots.borrow().is_empty());
        s.save_and_quit();
        assert_eq!(*mem.writes.borrow(), 1, "the erased slot is no longer in use");
    }
}
