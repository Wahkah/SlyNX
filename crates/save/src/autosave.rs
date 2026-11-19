//! The game in progress and when it saves (save-format.md 2).
//!
//! The engine keeps one [`Session`] and reports progress events to it. Each event updates the slot state and returns
//! a [`SaveRequest`] when the game should save now; the engine writes the slot (for example with
//! [`crate::store::SaveStore::commit`]) and shows the auto-save icon meanwhile.
//!
//! Auto-save happens only when a slot is in use **and** the player has acknowledged the auto-save notice (settings
//! bit 8). Manual saves ("Save Game...", "Save and Quit") always happen.

use crate::model::{Settings, SlotState, SprintResult};

/// One of the three save slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(u8);

impl Slot {
    pub const COUNT: usize = 3;
    pub const ALL: [Slot; 3] = [Slot(0), Slot(1), Slot(2)];

    /// Slot `index` (0, 1 or 2).
    pub fn new(index: u8) -> Option<Slot> {
        ((index as usize) < Self::COUNT).then_some(Slot(index))
    }

    pub fn index(self) -> u8 {
        self.0
    }
}

/// Why the game is saving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    KeyCollected,
    VaultOpened,
    /// First completion or a new best time.
    SprintFinished,
    /// An episode boss or the final boss.
    BossDefeated,
    /// In the hideout, after the first progress dialogue.
    HideoutDialogue,
    /// A different world or level than the one last saved was loaded.
    WorldChanged,
    /// "Save Game..." in the options menu.
    Manual,
    /// "Save and Quit" in the pause menu.
    SaveAndQuit,
}

impl Trigger {
    pub fn is_auto(self) -> bool {
        !matches!(self, Trigger::Manual | Trigger::SaveAndQuit)
    }
}

/// "Save the slot state to this slot now."
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveRequest {
    pub slot: Slot,
    pub trigger: Trigger,
}

/// The game in progress: its state, the slot it saves to, and where it was last saved.
#[derive(Clone, Debug)]
pub struct Session {
    pub state: SlotState,
    slot: Option<Slot>,
    /// Episode and level at the last save (or load).
    saved_at: Option<(u8, u8)>,
}

impl Session {
    /// A new game. `slot` is the slot chosen in "New Game..." (or `None` when the player plays without one).
    /// `settings` carries the original's options bits over from the title screen; auto-save stays off until the
    /// notice is acknowledged again.
    pub fn new_game(slot: Option<Slot>, settings: Settings) -> Session {
        let mut state = SlotState::new_game();
        state.settings = settings;
        state.settings.set(Settings::AUTOSAVE, false);
        state.settings.set(Settings::EPISODE_LINE_PLAYED, false);
        Session { state, slot, saved_at: None }
    }

    /// A game loaded from `slot`.
    pub fn loaded(slot: Slot, state: SlotState) -> Session {
        let saved_at = Some((state.episode, state.level));
        Session { state, slot: Some(slot), saved_at }
    }

    pub fn slot(&self) -> Option<Slot> {
        self.slot
    }

    /// Auto-save is on: a slot is in use and the notice has been acknowledged.
    pub fn autosave_enabled(&self) -> bool {
        self.slot.is_some() && self.state.settings.autosave()
    }

    /// The player acknowledged the auto-save notice: auto-save is on from now on (settings bit 8).
    pub fn acknowledge_autosave_notice(&mut self) {
        self.state.settings.set(Settings::AUTOSAVE, true);
    }

    /// Auto-save switched off ("Auto-save has been disabled"), for example after the player declined to create a
    /// save file.
    pub fn disable_autosave(&mut self) {
        self.state.settings.set(Settings::AUTOSAVE, false);
    }

    fn auto(&self, trigger: Trigger) -> Option<SaveRequest> {
        if self.autosave_enabled() { self.slot.map(|slot| SaveRequest { slot, trigger }) } else { None }
    }

    /// The level's key was collected.
    pub fn on_key_collected(&mut self, episode: u8, level: u8) -> Option<SaveRequest> {
        if self.state.collect_key(episode, level) { self.auto(Trigger::KeyCollected) } else { None }
    }

    /// A vault was opened, granting `reward_bit`. Returns the milestones newly reached and the save request.
    pub fn on_vault_opened(&mut self, episode: u8, level: u8, reward_bit: Option<u8>) -> (u8, Option<SaveRequest>) {
        match self.state.open_vault(episode, level, reward_bit) {
            Some(m) => (m, self.auto(Trigger::VaultOpened)),
            None => (0, None),
        }
    }

    /// A Master Thief Sprint was finished in `seconds`. Saves on the first completion or a new best time.
    pub fn on_sprint_finished(&mut self, episode: u8, level: u8, seconds: f32) -> (SprintResult, Option<SaveRequest>) {
        let r = self.state.finish_sprint(episode, level, seconds);
        let save = if r.first_completion || r.new_best { self.auto(Trigger::SprintFinished) } else { None };
        (r, save)
    }

    /// An episode boss or the final boss was defeated in `level`.
    pub fn on_boss_defeated(&mut self, episode: u8, level: u8) -> Option<SaveRequest> {
        self.state.defeat_boss(episode, level);
        self.auto(Trigger::BossDefeated)
    }

    /// The first progress dialogue in the hideout has played.
    pub fn on_hideout_progress_dialogue(&mut self) -> Option<SaveRequest> {
        self.auto(Trigger::HideoutDialogue)
    }

    /// A world or level was loaded. Saves when it is not the one last saved.
    pub fn on_world_loaded(&mut self, episode: u8, level: u8) -> Option<SaveRequest> {
        self.state.visit(episode, level);
        if self.saved_at == Some((episode, level)) { None } else { self.auto(Trigger::WorldChanged) }
    }

    /// "Save Game..." on `slot`: the slot is in use from now on.
    pub fn save_game(&mut self, slot: Slot) -> SaveRequest {
        self.slot = Some(slot);
        SaveRequest { slot, trigger: Trigger::Manual }
    }

    /// "Save and Quit": saves to the slot in use (`None` when no slot is in use: the menu shows "Quit Game").
    pub fn save_and_quit(&self) -> Option<SaveRequest> {
        self.slot.map(|slot| SaveRequest { slot, trigger: Trigger::SaveAndQuit })
    }

    /// Call after a request was written successfully.
    pub fn saved(&mut self, request: &SaveRequest) {
        self.slot = Some(request.slot);
        self.saved_at = Some((self.state.episode, self.state.level));
    }

    /// The slot was erased while in use: nothing to save to any more.
    pub fn slot_erased(&mut self, slot: Slot) {
        if self.slot == Some(slot) {
            self.slot = None;
            self.saved_at = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session::new_game(Slot::new(1), Settings(Settings::MONO | Settings::AUTOSAVE))
    }

    #[test]
    fn slots() {
        assert_eq!(Slot::new(2).map(Slot::index), Some(2));
        assert_eq!(Slot::new(3), None);
    }

    #[test]
    fn nothing_auto_saves_before_the_notice_is_acknowledged() {
        let mut s = session();
        assert!(s.state.settings.mono(), "options carried over");
        assert!(!s.autosave_enabled(), "a new game asks again");
        assert_eq!(s.on_key_collected(1, 0), None);
        assert_eq!(s.on_world_loaded(1, 1), None);
        assert!(s.state.level(1, 0).flags & crate::model::level_flags::KEY != 0, "progress still recorded");
        // Manual saves work regardless.
        assert_eq!(s.save_game(Slot::new(0).unwrap()).trigger, Trigger::Manual);
    }

    #[test]
    fn no_slot_no_auto_save() {
        let mut s = Session::new_game(None, Settings(0));
        s.acknowledge_autosave_notice();
        assert!(!s.autosave_enabled());
        assert_eq!(s.on_key_collected(1, 0), None);
        assert_eq!(s.save_and_quit(), None);
    }

    #[test]
    fn auto_save_triggers() {
        let mut s = session();
        s.acknowledge_autosave_notice();
        let slot = Slot::new(1).unwrap();
        let req = |t| Some(SaveRequest { slot, trigger: t });

        let first = s.on_world_loaded(1, 0);
        assert_eq!(first, req(Trigger::WorldChanged));
        assert_eq!(s.on_world_loaded(1, 0), req(Trigger::WorldChanged), "not saved yet: still differs");
        s.saved(&first.unwrap());
        assert_eq!(s.on_world_loaded(1, 0), None, "same level as the last save");

        assert_eq!(s.on_key_collected(1, 0), req(Trigger::KeyCollected));
        assert_eq!(s.on_key_collected(1, 0), None, "a key saves once");

        let (m, r) = s.on_vault_opened(1, 0, Some(2));
        assert_eq!((m, r), (crate::model::milestones::FIRST_VAULT, req(Trigger::VaultOpened)));
        assert_eq!(s.on_vault_opened(1, 0, Some(2)), (0, None));

        assert_eq!(s.on_sprint_finished(1, 0, 70.0).1, req(Trigger::SprintFinished));
        assert_eq!(s.on_sprint_finished(1, 0, 75.0).1, None, "slower: no save");
        assert_eq!(s.on_sprint_finished(1, 0, 65.0).1, req(Trigger::SprintFinished), "new best");

        assert_eq!(s.on_boss_defeated(1, 8), req(Trigger::BossDefeated));
        assert_eq!(s.on_hideout_progress_dialogue(), req(Trigger::HideoutDialogue));
        assert_eq!(s.save_and_quit(), req(Trigger::SaveAndQuit));

        s.disable_autosave();
        assert_eq!(s.on_world_loaded(2, 0), None);
        s.acknowledge_autosave_notice();
        assert_eq!(s.on_world_loaded(2, 0), req(Trigger::WorldChanged));

        s.slot_erased(slot);
        assert_eq!(s.on_world_loaded(2, 1), None);
    }

    #[test]
    fn loading_resumes_without_an_immediate_save() {
        let mut state = SlotState::new_game();
        state.visit(2, 3);
        state.settings.set(Settings::AUTOSAVE, true);
        let mut s = Session::loaded(Slot::new(0).unwrap(), state);
        assert!(s.autosave_enabled(), "the acknowledged notice is part of the slot");
        assert_eq!(s.on_world_loaded(2, 3), None, "resuming the saved level");
        assert!(s.on_world_loaded(2, 4).is_some());
    }
}
