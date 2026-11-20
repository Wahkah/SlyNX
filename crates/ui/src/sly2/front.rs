//! The front end of Sly 2 and Sly 3: the title, the new-game and load pages with the three save slots, the episode
//! menu and its movie list (Sly 2 level-flow.md 3 and 4, saving.md 3; Sly 3 level-flow.md 3).
//!
//! - **Title**: the game's name and "Press START button for New Game" or "... to Resume Game" (Sly 2 strings 5 and
//!   6, Sly 3 13 and 14) when a slot is in use. Start or Cross opens the main page.
//! - **Main page**: Continue (the newest slot, saving.md 3), New Game, Load Game (when a slot is in use).
//! - **Slot pages**: the three slots, "Empty" when unused. A new game over a used slot asks first ("Do you want to
//!   overwrite this game?"); Cross again confirms, Triangle or Circle cancels.
//! - **Episode menu**: the episodes that can be chosen (the session decides which), then "Watch a Movie" when there
//!   are movies; Cross starts the episode or opens the movie list, where Cross plays a movie.
//!
//! Texts come from the session as font codes (the world's strings, with the game's escapes). The menu hides itself
//! when it hands the session an action that leaves it.

use alloc::vec::Vec;

use super::font::FontSheet;
use super::jobs::{Phase, Screen};
use crate::draw::{DrawList, Rect};
use crate::nav::{Dir, MenuInput, Repeat};

/// Text size of the rows [working choice].
pub const SIZE: f32 = 20.0;
/// Text size of the game's name on the title [working choice].
pub const NAME_SIZE: f32 = 56.0;
/// Seconds per blink of the title's "Press START" line [working choice].
pub const BLINK: f32 = 1.2;

/// The pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Title,
    Main,
    /// The slots, for a new game or to load.
    Slots(SlotsFor),
    Episodes,
    Movies,
}

/// What a slot page is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotsFor {
    NewGame,
    Load,
}

/// A save slot as shown.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotRow {
    pub used: bool,
    /// What the slot holds (for example the episode), shown after the slot's number.
    pub text: Vec<u8>,
}

/// An episode of the episode menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EpisodeRow {
    /// The game's episode number.
    pub episode: u8,
    /// Its title.
    pub title: Vec<u8>,
}

/// A movie of the movie list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MovieRow {
    pub identity: u32,
    pub title: Vec<u8>,
}

/// The menu's texts, as font codes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Texts {
    /// The game's name.
    pub name: Vec<u8>,
    pub press_new: Vec<u8>,
    pub press_resume: Vec<u8>,
    pub continue_game: Vec<u8>,
    pub new_game: Vec<u8>,
    pub load_game: Vec<u8>,
    pub empty: Vec<u8>,
    pub overwrite: Vec<u8>,
    /// The episode menu's heading and hint.
    pub episodes: Vec<u8>,
    pub start_episode: Vec<u8>,
    pub watch_movie: Vec<u8>,
}

/// What the menu asks of the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontAction {
    None,
    /// A new game in slot 0 to 2.
    NewGame(usize),
    /// Load a slot.
    LoadGame(usize),
    /// Continue from the newest slot.
    Continue(usize),
    /// Start an episode.
    StartEpisode(u8),
    /// Play a movie (the menu stays).
    PlayMovie(u32),
}

/// The front end.
#[derive(Clone, Debug, PartialEq)]
pub struct FrontMenu {
    pub page: Page,
    pub cursor: usize,
    pub screen: Screen,
    pub texts: Texts,
    pub slots: [SlotRow; 3],
    /// The slot Continue loads.
    pub newest: Option<usize>,
    pub episodes: Vec<EpisodeRow>,
    pub movies: Vec<MovieRow>,
    /// A used slot waiting for the overwrite answer.
    pub confirm: Option<usize>,
    repeat: Repeat,
    time: f32,
}

/// The main page's entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainEntry {
    Continue,
    NewGame,
    LoadGame,
}

impl FrontMenu {
    fn with(page: Page, texts: Texts) -> FrontMenu {
        let mut screen = Screen::new(0);
        screen.show();
        FrontMenu {
            page,
            cursor: 0,
            screen,
            texts,
            slots: Default::default(),
            newest: None,
            episodes: Vec::new(),
            movies: Vec::new(),
            confirm: None,
            repeat: Repeat::default(),
            time: 0.0,
        }
    }

    /// The title, with the slots and the newest one.
    pub fn title(texts: Texts, slots: [SlotRow; 3], newest: Option<usize>) -> FrontMenu {
        FrontMenu { slots, newest, ..FrontMenu::with(Page::Title, texts) }
    }

    /// The episode menu, with the episodes that can be chosen and the movies that can be watched.
    pub fn episode_menu(texts: Texts, episodes: Vec<EpisodeRow>, movies: Vec<MovieRow>) -> FrontMenu {
        FrontMenu { episodes, movies, ..FrontMenu::with(Page::Episodes, texts) }
    }

    /// Whether it is on screen.
    pub fn visible(&self) -> bool {
        self.screen.visible()
    }

    fn main_entries(&self) -> Vec<MainEntry> {
        let mut v = Vec::new();
        if self.newest.is_some() {
            v.push(MainEntry::Continue);
        }
        v.push(MainEntry::NewGame);
        if self.slots.iter().any(|s| s.used) {
            v.push(MainEntry::LoadGame);
        }
        v
    }

    /// Rows on the current page.
    pub fn rows(&self) -> usize {
        match self.page {
            Page::Title => 0,
            Page::Main => self.main_entries().len(),
            Page::Slots(_) => 3,
            Page::Episodes => self.episodes.len() + usize::from(!self.movies.is_empty()),
            Page::Movies => self.movies.len(),
        }
    }

    fn go(&mut self, page: Page) {
        self.page = page;
        self.cursor = 0;
        self.confirm = None;
    }

    fn leave(&mut self, a: FrontAction) -> FrontAction {
        self.screen.hide();
        a
    }

    /// Advances on the real clock. `circle`: Circle went down (a second way back).
    pub fn update(&mut self, dt: f32, input: &MenuInput, circle: bool) -> FrontAction {
        self.screen.update(dt);
        self.time += dt;
        if !matches!(self.screen.phase, Phase::Idle(_)) {
            return FrontAction::None;
        }
        let back = input.back || circle;
        match self.repeat.update(input, dt) {
            Some(Dir::Up) if self.cursor > 0 => {
                self.cursor -= 1;
                self.confirm = None;
            }
            Some(Dir::Down) if self.cursor + 1 < self.rows() => {
                self.cursor += 1;
                self.confirm = None;
            }
            _ => {}
        }
        match self.page {
            Page::Title => {
                if input.start || input.accept {
                    self.go(Page::Main);
                }
            }
            Page::Main => {
                if back {
                    self.go(Page::Title);
                } else if input.accept {
                    match self.main_entries().get(self.cursor) {
                        Some(MainEntry::Continue) => {
                            if let Some(s) = self.newest {
                                return self.leave(FrontAction::Continue(s));
                            }
                        }
                        Some(MainEntry::NewGame) => self.go(Page::Slots(SlotsFor::NewGame)),
                        Some(MainEntry::LoadGame) => self.go(Page::Slots(SlotsFor::Load)),
                        None => {}
                    }
                }
            }
            Page::Slots(what) => {
                if back {
                    if self.confirm.take().is_none() {
                        self.go(Page::Main);
                    }
                } else if input.accept {
                    let i = self.cursor;
                    let used = self.slots[i].used;
                    match what {
                        SlotsFor::NewGame if used && self.confirm != Some(i) => self.confirm = Some(i),
                        SlotsFor::NewGame => return self.leave(FrontAction::NewGame(i)),
                        SlotsFor::Load if used => return self.leave(FrontAction::LoadGame(i)),
                        SlotsFor::Load => {}
                    }
                }
            }
            Page::Episodes => {
                if input.accept {
                    match self.episodes.get(self.cursor) {
                        Some(e) => return self.leave(FrontAction::StartEpisode(e.episode)),
                        None if !self.movies.is_empty() => self.go(Page::Movies),
                        None => {}
                    }
                }
            }
            Page::Movies => {
                if back {
                    self.go(Page::Episodes);
                } else if input.accept
                    && let Some(m) = self.movies.get(self.cursor)
                {
                    return FrontAction::PlayMovie(m.identity);
                }
            }
        }
        FrontAction::None
    }

    /// Draws the page over the canvas (the world behind it shows through the shade).
    pub fn draw(&self, list: &mut DrawList, font: Option<&FontSheet>) {
        if !self.screen.visible() {
            return;
        }
        let a = self.screen.coverage();
        let (cw, ch) = (list.canvas.width, list.canvas.height);
        let white = [[1.0, 1.0, 1.0, a], [1.0, 0.85, 0.3, a]];
        let gold = [[1.0, 0.85, 0.3, a]; 2];
        list.rect(Rect::new(0.0, 0.0, cw, ch), [0.0, 0.0, 0.04, 0.45 * a]);
        let Some(f) = font else { return };
        let centred = |list: &mut DrawList, text: &[u8], y: f32, size: f32, colours: [[f32; 4]; 2]| {
            let w = f.measure(text, size);
            f.draw(list, text, (cw - w) * 0.5, y, size, colours, None);
        };
        if self.page == Page::Title {
            centred(list, &self.texts.name, ch * 0.28, NAME_SIZE, gold);
            // The "Press START" line blinks.
            let t = self.time / BLINK;
            if t - (t as u32 as f32) < 0.7 {
                let line = if self.newest.is_some() { &self.texts.press_resume } else { &self.texts.press_new };
                centred(list, line, ch * 0.68, SIZE, white);
            }
            return;
        }
        let rows: Vec<(Vec<u8>, bool)> = match self.page {
            Page::Title => Vec::new(),
            Page::Main => self
                .main_entries()
                .iter()
                .map(|e| match e {
                    MainEntry::Continue => (self.texts.continue_game.clone(), true),
                    MainEntry::NewGame => (self.texts.new_game.clone(), true),
                    MainEntry::LoadGame => (self.texts.load_game.clone(), true),
                })
                .collect(),
            Page::Slots(what) => self
                .slots
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let mut t = alloc::format!("{}  ", i + 1).into_bytes();
                    t.extend_from_slice(if s.used { &s.text } else { &self.texts.empty });
                    (t, s.used || what == SlotsFor::NewGame)
                })
                .collect(),
            Page::Episodes => {
                let mut v: Vec<(Vec<u8>, bool)> = self.episodes.iter().map(|e| (e.title.clone(), true)).collect();
                if !self.movies.is_empty() {
                    v.push((self.texts.watch_movie.clone(), true));
                }
                v
            }
            Page::Movies => self.movies.iter().map(|m| (m.title.clone(), true)).collect(),
        };
        let heading: &[u8] = match self.page {
            Page::Slots(SlotsFor::NewGame) => &self.texts.new_game,
            Page::Slots(SlotsFor::Load) => &self.texts.load_game,
            Page::Episodes => &self.texts.episodes,
            Page::Movies => &self.texts.watch_movie,
            _ => &self.texts.name,
        };
        // Rows close up when the page has many (the panel stays on the canvas).
        let n = rows.len().max(1) as f32;
        let row = (f.line_height(SIZE) + 10.0).min((ch - 16.0 - 110.0) / n);
        let h = 70.0 + row * n + 40.0;
        let panel = Rect::new(cw * 0.18, ((ch - h) * 0.5).max(8.0), cw * 0.64, h);
        list.rect(panel, [0.04, 0.06, 0.16, 0.9 * a]);
        list.frame(panel, 3.0, [0.9, 0.75, 0.25, a]);
        centred(list, heading, panel.y + 14.0, SIZE + 6.0, gold);
        for (i, (text, enabled)) in rows.iter().enumerate() {
            let y = panel.y + 64.0 + row * i as f32;
            if i == self.cursor {
                list.rect(Rect::new(panel.x + 12.0, y - 5.0, panel.w - 24.0, row - 2.0), [0.9, 0.75, 0.25, 0.3 * a]);
            }
            let c = if *enabled { white } else { [[0.6, 0.6, 0.6, a]; 2] };
            f.draw(list, text, panel.x + 28.0, y, SIZE, c, Some(panel.w - 56.0));
        }
        let hint: &[u8] = match (self.page, self.confirm) {
            (Page::Slots(_), Some(_)) => &self.texts.overwrite,
            (Page::Episodes, _) => &self.texts.start_episode,
            _ => &[],
        };
        if !hint.is_empty() {
            centred(list, hint, panel.bottom() - 34.0, SIZE * 0.85, gold);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn press(m: &mut FrontMenu, i: MenuInput) -> FrontAction {
        let a = m.update(1.0 / 60.0, &i, false);
        // Let the repeat settle between presses.
        m.update(0.5, &MenuInput::default(), false);
        a
    }

    fn settle(m: &mut FrontMenu) {
        for _ in 0..60 {
            m.update(1.0 / 60.0, &MenuInput::default(), false);
        }
    }

    const ACCEPT: MenuInput = MenuInput {
        dpad: [false; 4],
        stick: (0.0, 0.0),
        accept: true,
        back: false,
        erase: false,
        start: false,
        select: false,
    };
    const DOWN: MenuInput = MenuInput { dpad: [false, true, false, false], accept: false, ..ACCEPT };
    const BACK: MenuInput = MenuInput { back: true, accept: false, ..ACCEPT };

    #[test]
    fn title_new_game_asks_before_overwriting() {
        let used = SlotRow { used: true, text: b"Episode 2".to_vec() };
        let mut m = FrontMenu::title(Texts::default(), [used, SlotRow::default(), SlotRow::default()], Some(0));
        settle(&mut m);
        assert_eq!(press(&mut m, MenuInput { start: true, ..MenuInput::default() }), FrontAction::None);
        assert_eq!(m.page, Page::Main);
        // Continue, New Game, Load Game.
        assert_eq!(m.rows(), 3);
        press(&mut m, DOWN);
        press(&mut m, ACCEPT);
        assert_eq!(m.page, Page::Slots(SlotsFor::NewGame));
        // Slot 1 is used: the first Cross asks, Triangle cancels, then Cross twice overwrites.
        assert_eq!(press(&mut m, ACCEPT), FrontAction::None);
        assert_eq!(m.confirm, Some(0));
        press(&mut m, BACK);
        assert_eq!((m.page, m.confirm), (Page::Slots(SlotsFor::NewGame), None));
        press(&mut m, ACCEPT);
        assert_eq!(press(&mut m, ACCEPT), FrontAction::NewGame(0));
        settle(&mut m);
        assert!(!m.visible(), "the menu leaves with its action");
    }

    #[test]
    fn continue_and_load() {
        let used = SlotRow { used: true, text: Vec::new() };
        let mut m = FrontMenu::title(Texts::default(), [SlotRow::default(), used.clone(), used], Some(2));
        settle(&mut m);
        press(&mut m, ACCEPT);
        assert_eq!(press(&mut m, ACCEPT), FrontAction::Continue(2));
        // Without saves the main page is New Game alone.
        let mut m = FrontMenu::title(Texts::default(), Default::default(), None);
        settle(&mut m);
        press(&mut m, ACCEPT);
        assert_eq!(m.rows(), 1);
        // Loading skips an empty slot.
        let used = SlotRow { used: true, text: Vec::new() };
        let mut m = FrontMenu::title(Texts::default(), [SlotRow::default(), used, SlotRow::default()], None);
        settle(&mut m);
        press(&mut m, ACCEPT);
        press(&mut m, DOWN);
        press(&mut m, ACCEPT);
        assert_eq!(m.page, Page::Slots(SlotsFor::Load));
        assert_eq!(press(&mut m, ACCEPT), FrontAction::None);
        press(&mut m, DOWN);
        assert_eq!(press(&mut m, ACCEPT), FrontAction::LoadGame(1));
    }

    #[test]
    fn episodes_and_movies() {
        let e = |n: u8| EpisodeRow { episode: n, title: vec![b'A' + n] };
        let movies = vec![MovieRow { identity: 0x7D9, title: b"M".to_vec() }];
        let mut m = FrontMenu::episode_menu(Texts::default(), vec![e(1), e(2)], movies);
        settle(&mut m);
        assert_eq!(m.rows(), 3);
        press(&mut m, DOWN);
        press(&mut m, DOWN);
        press(&mut m, ACCEPT);
        assert_eq!(m.page, Page::Movies);
        assert_eq!(press(&mut m, ACCEPT), FrontAction::PlayMovie(0x7D9));
        press(&mut m, BACK);
        assert_eq!(press(&mut m, ACCEPT), FrontAction::StartEpisode(1));
    }

    #[test]
    fn draws_rows() {
        let mut m = FrontMenu::title(Texts::default(), Default::default(), None);
        settle(&mut m);
        let mut list = DrawList::default();
        m.draw(&mut list, None);
        assert_eq!(list.quads.len(), 1, "the shade (no font: no text)");
    }
}
