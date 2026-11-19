//! Sly 2's pause menu (object 880; user-interface.md 6.1): opened with Start during play when no other menu is open;
//! the game pauses. The entries are the game's (string IDs from the table at `0x2C00E8` [D]): Keep Playing (57),
//! Quit Game (70), Exit to Episode Menu (66), Abandon Job (107), Options (44), Controls (67), Job Help (106), shown
//! only when they apply. SlyNX has no options or controls pages yet, so it offers Save Game (62, from the options
//! table) in their place [working choice]. Up and Down step with repeat (the wheel), Cross selects, Triangle or
//! Start goes back.

use alloc::vec::Vec;

use super::font::FontSheet;
use super::jobs::{Phase, Screen};
use crate::draw::{DrawList, Rect};
use crate::nav::{Dir, MenuInput, Repeat};

/// The pause menu's screen object.
pub const PAUSE_SCREEN: i32 = 880;
/// Text size [working choice].
pub const ENTRY_SIZE: f32 = 24.0;

/// An entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    KeepPlaying,
    SaveGame,
    JobHelp,
    AbandonJob,
    ExitToEpisodeMenu,
    QuitGame,
}

impl Entry {
    /// The entry's string ID.
    pub fn string(self) -> i16 {
        match self {
            Entry::KeepPlaying => 57,
            Entry::SaveGame => 62,
            Entry::JobHelp => 106,
            Entry::AbandonJob => 107,
            Entry::ExitToEpisodeMenu => 66,
            Entry::QuitGame => 70,
        }
    }

    /// English text when the world's strings are not at hand.
    pub fn fallback(self) -> &'static str {
        match self {
            Entry::KeepPlaying => "Keep Playing",
            Entry::SaveGame => "Save Game...",
            Entry::JobHelp => "Job Help",
            Entry::AbandonJob => "Abandon Job",
            Entry::ExitToEpisodeMenu => "Exit to Episode Menu",
            Entry::QuitGame => "Quit Game",
        }
    }
}

/// What applies now (user-interface.md 6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// Inside an episode (not the intro or the menu world).
    pub in_episode: bool,
    /// The world has the job-help screen (1248).
    pub has_job_help: bool,
    /// A job runs.
    pub job_running: bool,
    /// Saving is possible (a slot and a file system).
    pub can_save: bool,
}

/// The entries that apply, in the game's order.
pub fn entries(c: Context) -> Vec<Entry> {
    let mut v = alloc::vec![Entry::KeepPlaying];
    if c.can_save {
        v.push(Entry::SaveGame);
    }
    if c.in_episode && c.has_job_help {
        v.push(Entry::JobHelp);
    }
    if c.job_running {
        v.push(Entry::AbandonJob);
    }
    if c.in_episode {
        v.push(Entry::ExitToEpisodeMenu);
    }
    v.push(Entry::QuitGame);
    v
}

/// What the menu asks of the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    /// An entry was chosen (the menu closes for every entry but Save Game).
    Chosen(Entry),
    /// Closed (Keep Playing, back).
    Closed,
}

/// The pause menu.
#[derive(Clone, Debug, PartialEq)]
pub struct PauseMenu {
    pub entries: Vec<Entry>,
    pub cursor: usize,
    pub screen: Screen,
    repeat: Repeat,
}

impl Default for PauseMenu {
    fn default() -> Self {
        PauseMenu { entries: Vec::new(), cursor: 0, screen: Screen::new(PAUSE_SCREEN), repeat: Repeat::default() }
    }
}

impl PauseMenu {
    /// Opens it with the entries that apply.
    pub fn open(&mut self, c: Context) {
        self.entries = entries(c);
        self.cursor = 0;
        self.screen.show();
    }

    /// Whether it is open (it pauses the game).
    pub fn is_open(&self) -> bool {
        self.screen.visible()
    }

    /// Closes it.
    pub fn close(&mut self) {
        self.screen.hide();
    }

    /// Advances on the real clock with this frame's input.
    pub fn update(&mut self, dt: f32, input: &MenuInput) -> Action {
        self.screen.update(dt);
        if !matches!(self.screen.phase, Phase::Idle(_) | Phase::Showing(_)) {
            return Action::None;
        }
        if input.back || input.start {
            self.close();
            return Action::Closed;
        }
        match self.repeat.update(input, dt) {
            Some(Dir::Up) if self.cursor > 0 => self.cursor -= 1,
            Some(Dir::Down) if self.cursor + 1 < self.entries.len() => self.cursor += 1,
            _ => {}
        }
        if input.accept
            && let Some(&e) = self.entries.get(self.cursor)
        {
            if e != Entry::SaveGame {
                self.close();
            }
            return if e == Entry::KeepPlaying { Action::Closed } else { Action::Chosen(e) };
        }
        Action::None
    }

    /// Draws the menu with `label` giving each entry's text (Latin-1 bytes).
    pub fn draw(&self, list: &mut DrawList, font: Option<&FontSheet>, label: &dyn Fn(Entry) -> Vec<u8>) {
        if !self.screen.visible() {
            return;
        }
        let a = self.screen.coverage();
        let (cw, ch) = (list.canvas.width, list.canvas.height);
        let row = 34.0;
        let h = row * self.entries.len() as f32 + 40.0;
        let panel = Rect::new(cw * 0.3, (ch - h) / 2.0, cw * 0.4, h);
        list.rect(Rect::new(0.0, 0.0, cw, ch), [0.0, 0.0, 0.0, 0.35 * a]);
        list.rect(panel, [0.05, 0.08, 0.2, 0.9 * a]);
        list.frame(panel, 3.0, [0.9, 0.75, 0.25, a]);
        for (i, &e) in self.entries.iter().enumerate() {
            let y = panel.y + 20.0 + row * i as f32;
            if i == self.cursor {
                list.rect(Rect::new(panel.x + 10.0, y - 2.0, panel.w - 20.0, row - 4.0), [0.9, 0.75, 0.25, 0.35 * a]);
            }
            if let Some(f) = font {
                let text = label(e);
                let w = f.measure(&text, ENTRY_SIZE);
                f.draw(list, &text, panel.x + (panel.w - w) / 2.0, y, ENTRY_SIZE, [[1.0, 1.0, 1.0, a]; 2], None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_follow_the_context() {
        assert_eq!(entries(Context::default()), alloc::vec![Entry::KeepPlaying, Entry::QuitGame]);
        let all = Context { in_episode: true, has_job_help: true, job_running: true, can_save: true };
        assert_eq!(entries(all).len(), 6);
    }

    #[test]
    fn choose_and_back() {
        let mut m = PauseMenu::default();
        m.open(Context { in_episode: true, has_job_help: true, job_running: false, can_save: true });
        let down = MenuInput { dpad: [false, true, false, false], ..MenuInput::default() };
        m.update(0.016, &MenuInput::default());
        m.update(0.016, &down);
        m.update(0.5, &MenuInput::default());
        m.update(0.016, &down);
        assert_eq!(m.entries[m.cursor], Entry::JobHelp);
        let a = m.update(0.016, &MenuInput { accept: true, ..MenuInput::default() });
        assert_eq!(a, Action::Chosen(Entry::JobHelp));
        m.open(Context::default());
        m.update(0.016, &MenuInput::default());
        assert_eq!(m.update(0.016, &MenuInput { back: true, ..MenuInput::default() }), Action::Closed);
    }
}
