//! The menu framework (ui-and-menus.md 3 to 7): a state machine driven by [`MenuInput`] that produces a draw list
//! and [`Action`]s for the game to carry out.
//!
//! Screens: the title, pause (entries depend on the level), options, controls, center screen, the save / load /
//! new-game slot lists, yes/no confirmations and messages (any prompt of the table), the hideout episode select and
//! the map screen.

use alloc::vec;
use alloc::vec::Vec;

use crate::draw::{DrawList, Rect, WHITE, rgba8};
use crate::font::FontSet;
use crate::hud::{Slide, draw_shadowed};
use crate::nav::{Dir, MenuInput, Repeat};
use crate::tables::{SlotInfo, TextTables, prompt, resp};
use crate::text::{Align, TextStyle, layout};

/// The three levels with a "Restart Level" entry.
pub const RESTART_LEVEL_WORLDS: [&str; 3] = ["Piranha Lake", "Down Home Cooking", "Burning Rubber"];
/// Number of save slots.
pub const SLOTS: usize = 3;

/// Facts about where the game is, which decide the pause menu's entries (ui-and-menus.md 4).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PauseContext {
    /// Display name of the current world.
    pub world_name: Vec<u8>,
    /// In a race level, during the race.
    pub in_race: bool,
    /// A Master Thief Sprint is running.
    pub sprint_running: bool,
    /// The level has a map.
    pub has_map: bool,
    /// In the prologue.
    pub prologue: bool,
    /// In the hideout.
    pub hideout: bool,
    /// In the final episode.
    pub final_episode: bool,
    /// In an episode's approach level that has not been completed.
    pub approach_incomplete: bool,
    /// A save slot is in use ("Save and Quit").
    pub slot_in_use: bool,
}

impl PauseContext {
    /// The pause menu's response IDs, in order.
    pub fn entries(&self) -> Vec<i32> {
        let mut e = vec![resp::RETURN_TO_GAME];
        if self.in_race {
            e.push(resp::RESTART_RACE);
        }
        if self.sprint_running {
            e.push(resp::RESTART_SPRINT);
        }
        if RESTART_LEVEL_WORLDS.iter().any(|w| w.as_bytes() == self.world_name.as_slice()) {
            e.push(resp::RESTART_LEVEL);
        }
        if self.has_map {
            e.push(resp::VIEW_MAP);
        }
        e.push(resp::OPTIONS);
        if !self.prologue {
            e.push(if self.final_episode || self.hideout || self.approach_incomplete {
                resp::EXIT_TO_HIDEOUT
            } else {
                resp::EXIT_LEVEL
            });
        }
        e.push(resp::QUIT);
        e
    }
}

/// Player settings the options menu toggles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Vibrations on.
    pub vibration: bool,
    /// Music on.
    pub music: bool,
    /// Stereo (false: mono).
    pub stereo: bool,
    /// Inverted up/down: binoc-u-com, turrets, jet pack (true: "Up = Look Up" etc.).
    pub invert: [bool; 3],
    /// Picture offset set by Center Screen, canvas units.
    pub center: (i8, i8),
}

impl Default for Settings {
    fn default() -> Self {
        Settings { vibration: true, music: true, stereo: true, invert: [false; 3], center: (0, 0) }
    }
}

/// What a slot list is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotMode {
    /// Save Game.
    Save,
    /// Load Game.
    Load,
    /// New Game.
    New,
}

/// A yes/no question.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    /// Overwrite a used slot (saving or starting a new game there).
    Overwrite(SlotMode, usize),
    /// Erase a slot.
    Erase(usize),
    /// "Really Quit?"
    Quit,
}

/// One screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Title screen.
    Title,
    /// Pause menu.
    Pause,
    /// Options menu.
    Options,
    /// Controls (invert options).
    Controls,
    /// Center Screen.
    CenterScreen,
    /// A slot list.
    Slots(SlotMode),
    /// A yes/no question.
    Confirm(Confirm),
    /// Any prompt of the table, answered with its responses ([`Action::Answer`]).
    Prompt(usize),
    /// The hideout episode select.
    Hideout,
    /// The map screen.
    Map,
}

/// What the game should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Close the menus and resume.
    Resume,
    /// Title: Start pressed (new game, or resume the loaded save).
    TitleStart {
        /// A save is loaded.
        resume: bool,
    },
    /// Start a new game in a slot (after any overwrite question).
    NewGame(usize),
    /// Load a slot.
    LoadGame(usize),
    /// Save to a slot.
    Save(usize),
    /// Erase a slot.
    EraseSlot(usize),
    /// View the map.
    ViewMap,
    /// Restart the race.
    RestartRace,
    /// Restart the sprint.
    RestartSprint,
    /// Restart the level.
    RestartLevel,
    /// Exit the level.
    ExitLevel,
    /// Exit to the hideout.
    ExitToHideout,
    /// Quit the game (saving first when `save`).
    Quit {
        /// Save and quit.
        save: bool,
    },
    /// The settings changed.
    Settings(Settings),
    /// Hideout: enter a visited episode (its map screen).
    EnterEpisode(usize),
    /// Hideout: play the newest episode's introduction.
    PlayEpisodeIntro(usize),
    /// Hideout: the selection moved (play the episode's voice hint).
    EpisodeSelected(usize),
    /// Map: enter a level.
    EnterLevel(usize),
    /// Map closed without choosing.
    MapClosed,
    /// A prompt opened with [`Menus::open_prompt`] was answered.
    Answer {
        /// Prompt index.
        prompt: usize,
        /// Response ID.
        response: i32,
    },
    /// A menu sound to play.
    Sound(MenuSound),
}

/// Menu sounds (the sound IDs are an open question in the spec; the game maps these).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuSound {
    /// The cursor moved.
    Move,
    /// Accepted.
    Accept,
    /// Backed out.
    Back,
    /// Not allowed.
    Refuse,
}

/// One episode of the hideout select.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Episode {
    /// Name shown.
    pub name: Vec<u8>,
    /// Unlocked.
    pub unlocked: bool,
    /// Visited (entered at least once).
    pub visited: bool,
    /// Keys (got, total).
    pub keys: (u32, u32),
    /// Vaults (got, total).
    pub vaults: (u32, u32),
    /// Sprints (got, total).
    pub sprints: (u32, u32),
}

/// One level on the map screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapLevel {
    /// Name shown.
    pub name: Vec<u8>,
    /// Position on the map picture, canvas units.
    pub at: (f32, f32),
    /// Can be entered.
    pub unlocked: bool,
    /// Goals done: key, vault, sprint.
    pub done: [bool; 3],
}

/// The menu state machine.
#[derive(Clone, Debug, Default)]
pub struct Menus {
    stack: Vec<(Screen, usize)>,
    repeat: Repeat,
    /// The prompt box's slide.
    pub slide: Slide,
    /// Settings.
    pub settings: Settings,
    /// Pause context.
    pub context: PauseContext,
    /// Save slots (`None` = empty).
    pub slots: [Option<SlotInfo>; SLOTS],
    /// The title screen was the way in (options shows New Game, hides Save Game).
    pub from_title: bool,
    /// A save is loaded (title says "Resume").
    pub save_loaded: bool,
    /// Hideout episodes (index 0 the prologue).
    pub episodes: Vec<Episode>,
    /// Map levels.
    pub map: Vec<MapLevel>,
    closing: bool,
    /// The auto-save notice (prompt 20) is still to be shown for the slot being played: set when a new game starts
    /// in a slot, cleared once the notice opens (hud.md 6: the player is told once what the save icon means).
    pub autosave_notice_pending: bool,
}

impl Menus {
    /// The screen on top, if any.
    pub fn screen(&self) -> Option<Screen> {
        self.stack.last().map(|s| s.0)
    }

    /// The cursor of the top screen.
    pub fn cursor(&self) -> usize {
        self.stack.last().map_or(0, |s| s.1)
    }

    /// Whether a menu is showing (or sliding out).
    pub fn is_open(&self) -> bool {
        !self.stack.is_empty() || self.slide.on_screen()
    }

    fn open(&mut self, s: Screen) {
        self.stack.clear();
        self.stack.push((s, 0));
        self.closing = false;
        self.slide.show();
    }

    /// Shows the title screen.
    pub fn open_title(&mut self) {
        self.from_title = true;
        self.open(Screen::Title);
    }

    /// Opens the pause menu for `context`.
    pub fn open_pause(&mut self, context: PauseContext) {
        self.from_title = false;
        self.context = context;
        self.open(Screen::Pause);
    }

    /// Opens any prompt of the table (game over, sprint results, messages); answering it gives
    /// [`Action::Answer`] and closes it.
    pub fn open_prompt(&mut self, index: usize) {
        self.open(Screen::Prompt(index));
    }

    /// Call before each auto-save. The first one in a new slot opens the auto-save notice (prompt 20, answered with
    /// Continue as `Action::Answer { prompt: 20, response: 2 }`) and returns true: save once it is answered, while
    /// the HUD's save icon shows. Returns false when the notice was already shown (save right away).
    pub fn before_autosave(&mut self) -> bool {
        if !self.autosave_notice_pending {
            return false;
        }
        self.autosave_notice_pending = false;
        self.open_prompt(crate::tables::prompt::AUTOSAVE_NOTICE);
        true
    }

    /// Opens the hideout episode select at `selected`.
    pub fn open_hideout(&mut self, episodes: Vec<Episode>, selected: usize) {
        self.episodes = episodes;
        self.open(Screen::Hideout);
        if let Some(t) = self.stack.last_mut() {
            t.1 = selected.min(self.episodes.len().saturating_sub(1));
        }
    }

    /// Opens the map screen with the cursor on `selected`.
    pub fn open_map(&mut self, levels: Vec<MapLevel>, selected: usize) {
        self.map = levels;
        let from_pause = self.screen() == Some(Screen::Pause);
        if from_pause {
            self.stack.push((Screen::Map, selected));
        } else {
            self.open(Screen::Map);
            if let Some(t) = self.stack.last_mut() {
                t.1 = selected;
            }
        }
    }

    /// Closes everything (slides out).
    pub fn close(&mut self) {
        self.stack.clear();
        self.slide.hide();
    }

    /// The entries (response IDs) of a list screen.
    pub fn entries(&self, s: Screen) -> Vec<i32> {
        match s {
            Screen::Pause => self.context.entries(),
            Screen::Options => {
                let mut e = Vec::new();
                if self.from_title {
                    e.push(resp::NEW_GAME);
                }
                e.push(resp::LOAD_GAME);
                if !self.from_title {
                    e.push(resp::SAVE_GAME);
                }
                e.extend_from_slice(&[
                    resp::VIBRATION,
                    resp::CENTER_SCREEN,
                    resp::MUSIC,
                    resp::SPEAKERS,
                    resp::CONTROLS,
                    resp::BACK,
                ]);
                e
            }
            Screen::Controls => vec![resp::INVERT_BINOC, resp::INVERT_TURRET, resp::INVERT_JETPACK, resp::BACK],
            Screen::Slots(_) => vec![resp::SLOT_1, resp::SLOT_2, resp::SLOT_3, resp::BACK],
            Screen::Confirm(_) => vec![resp::YES, resp::NO],
            Screen::Prompt(_) | Screen::Title | Screen::CenterScreen | Screen::Hideout | Screen::Map => Vec::new(),
        }
    }

    fn pop(&mut self, out: &mut Vec<Action>) {
        out.push(Action::Sound(MenuSound::Back));
        self.stack.pop();
        if self.stack.is_empty() {
            self.slide.hide();
            out.push(Action::Resume);
        }
    }

    fn set_cursor(&mut self, c: usize) {
        if let Some(t) = self.stack.last_mut() {
            t.1 = c;
        }
    }

    /// Advances by `dt` seconds with this frame's input. `prompt_responses` gives the response count of a
    /// [`Screen::Prompt`] (from the prompt table). Returns what the game should do.
    pub fn update(&mut self, input: &MenuInput, dt: f32, tables: &TextTables) -> Vec<Action> {
        let mut out = Vec::new();
        self.slide.update(dt, true);
        let Some((screen, cursor)) = self.stack.last().copied() else {
            self.repeat.update(&MenuInput::default(), dt);
            return out;
        };
        let mv = self.repeat.update(input, dt);
        match screen {
            Screen::Title => {
                if input.start {
                    out.push(Action::Sound(MenuSound::Accept));
                    out.push(Action::TitleStart { resume: self.save_loaded });
                    self.close();
                } else if input.select {
                    out.push(Action::Sound(MenuSound::Accept));
                    self.stack.push((Screen::Options, 0));
                }
            }
            Screen::CenterScreen => {
                if let Some(d) = mv {
                    let (dx, dy) = match d {
                        Dir::Up => (0, -1),
                        Dir::Down => (0, 1),
                        Dir::Left => (-1, 0),
                        Dir::Right => (1, 0),
                    };
                    let c = &mut self.settings.center;
                    *c = ((c.0 as i32 + dx).clamp(-16, 16) as i8, (c.1 as i32 + dy).clamp(-16, 16) as i8);
                    out.push(Action::Settings(self.settings));
                    out.push(Action::Sound(MenuSound::Move));
                }
                if input.accept || input.back {
                    self.pop(&mut out);
                }
            }
            Screen::Hideout => self.update_hideout(input, mv, cursor, &mut out),
            Screen::Map => self.update_map(input, mv, cursor, &mut out),
            Screen::Prompt(p) => {
                let q = tables.prompts.get(p);
                let n = q.map_or(0, |p| p.responses.len());
                if n > 0 {
                    self.move_cursor(mv, cursor, n, q.is_none_or(|q| q.vertical()), &mut out);
                }
                if input.accept && n > 0 {
                    let response = tables.prompts[p].responses[self.cursor().min(n - 1)];
                    out.push(Action::Sound(MenuSound::Accept));
                    out.push(Action::Answer { prompt: p, response });
                    self.close();
                } else if input.back && q.is_some_and(|q| q.responses.contains(&resp::NO)) {
                    // Back on a yes/no question answers No.
                    out.push(Action::Sound(MenuSound::Back));
                    out.push(Action::Answer { prompt: p, response: resp::NO });
                    self.close();
                } else if input.accept || input.back {
                    out.push(Action::Answer { prompt: p, response: -1 });
                    self.close();
                }
            }
            _ => {
                let entries = self.entries(screen);
                let vertical = tables.prompts.get(prompt_of(screen)).is_none_or(|q| q.vertical());
                self.move_cursor(mv, cursor, entries.len(), vertical, &mut out);
                let cursor = self.cursor();
                if (input.start && screen == Screen::Pause) || (input.back && screen == Screen::Pause) {
                    out.push(Action::Sound(MenuSound::Back));
                    out.push(Action::Resume);
                    self.close();
                } else if input.back {
                    self.pop(&mut out);
                } else if input.erase
                    && let Screen::Slots(_) = screen
                {
                    if cursor < SLOTS && self.slots[cursor].is_some() {
                        out.push(Action::Sound(MenuSound::Accept));
                        self.stack.push((Screen::Confirm(Confirm::Erase(cursor)), 1));
                    } else {
                        out.push(Action::Sound(MenuSound::Refuse));
                    }
                } else if input.accept
                    && let Some(&id) = entries.get(cursor)
                {
                    self.accept(screen, id, cursor, &mut out);
                }
            }
        }
        out
    }

    /// Moves the cursor over `n` responses: up / down in a vertical list, left / right side by side (the prompt's
    /// layout, ui-and-menus.md 2).
    fn move_cursor(&mut self, mv: Option<Dir>, cursor: usize, n: usize, vertical: bool, out: &mut Vec<Action>) {
        let (back, fwd) = if vertical { (Dir::Up, Dir::Down) } else { (Dir::Left, Dir::Right) };
        let c = match mv {
            Some(d) if d == back && cursor > 0 => cursor - 1,
            Some(d) if d == fwd && cursor + 1 < n => cursor + 1,
            _ => return,
        };
        self.set_cursor(c);
        out.push(Action::Sound(MenuSound::Move));
    }

    fn accept(&mut self, screen: Screen, id: i32, cursor: usize, out: &mut Vec<Action>) {
        out.push(Action::Sound(MenuSound::Accept));
        match (screen, id) {
            (_, resp::BACK) => {
                out.pop();
                self.pop(out);
            }
            (Screen::Pause, resp::RETURN_TO_GAME) => self.finish(out, Action::Resume),
            (Screen::Pause, resp::RESTART_RACE) => self.finish(out, Action::RestartRace),
            (Screen::Pause, resp::RESTART_SPRINT) => self.finish(out, Action::RestartSprint),
            (Screen::Pause, resp::RESTART_LEVEL) => self.finish(out, Action::RestartLevel),
            (Screen::Pause, resp::EXIT_LEVEL) => self.finish(out, Action::ExitLevel),
            (Screen::Pause, resp::EXIT_TO_HIDEOUT) => self.finish(out, Action::ExitToHideout),
            (Screen::Pause, resp::VIEW_MAP) => out.push(Action::ViewMap),
            (Screen::Pause, resp::QUIT) => self.stack.push((Screen::Confirm(Confirm::Quit), 1)),
            (_, resp::OPTIONS) => self.stack.push((Screen::Options, 0)),
            (_, resp::NEW_GAME) => self.stack.push((Screen::Slots(SlotMode::New), 0)),
            (_, resp::LOAD_GAME) => self.stack.push((Screen::Slots(SlotMode::Load), 0)),
            (_, resp::SAVE_GAME) => self.stack.push((Screen::Slots(SlotMode::Save), 0)),
            (_, resp::CONTROLS) => self.stack.push((Screen::Controls, 0)),
            (_, resp::CENTER_SCREEN) => self.stack.push((Screen::CenterScreen, 0)),
            (_, resp::VIBRATION) => self.toggle(out, |s| s.vibration = !s.vibration),
            (_, resp::MUSIC) => self.toggle(out, |s| s.music = !s.music),
            (_, resp::SPEAKERS) => self.toggle(out, |s| s.stereo = !s.stereo),
            (_, resp::INVERT_BINOC) => self.toggle(out, |s| s.invert[0] = !s.invert[0]),
            (_, resp::INVERT_TURRET) => self.toggle(out, |s| s.invert[1] = !s.invert[1]),
            (_, resp::INVERT_JETPACK) => self.toggle(out, |s| s.invert[2] = !s.invert[2]),
            (Screen::Slots(mode), _) => {
                let used = self.slots[cursor].is_some();
                match (mode, used) {
                    (SlotMode::Load, false) => {
                        out.pop();
                        out.push(Action::Sound(MenuSound::Refuse));
                    }
                    (SlotMode::Load, true) => self.finish(out, Action::LoadGame(cursor)),
                    (SlotMode::Save, false) => self.finish(out, Action::Save(cursor)),
                    (SlotMode::New, false) => {
                        self.autosave_notice_pending = true;
                        self.finish(out, Action::NewGame(cursor))
                    }
                    (_, true) => self.stack.push((Screen::Confirm(Confirm::Overwrite(mode, cursor)), 1)),
                }
            }
            (Screen::Confirm(c), resp::YES) => match c {
                Confirm::Overwrite(SlotMode::Save, s) => self.finish(out, Action::Save(s)),
                Confirm::Overwrite(_, s) => {
                    self.autosave_notice_pending = true;
                    self.finish(out, Action::NewGame(s))
                }
                Confirm::Erase(s) => {
                    out.push(Action::EraseSlot(s));
                    self.slots[s] = None;
                    self.stack.pop();
                }
                Confirm::Quit => self.finish(out, Action::Quit { save: self.context.slot_in_use }),
            },
            (Screen::Confirm(_), resp::NO) => {
                out.pop();
                self.pop(out);
            }
            _ => {}
        }
    }

    fn finish(&mut self, out: &mut Vec<Action>, a: Action) {
        out.push(a);
        self.close();
    }

    fn toggle(&mut self, out: &mut Vec<Action>, f: impl FnOnce(&mut Settings)) {
        f(&mut self.settings);
        out.push(Action::Settings(self.settings));
    }

    /// The highest selectable episode, and whether the list wraps (only once the final episode is unlocked).
    fn episode_bounds(&self) -> (usize, bool) {
        let last = self.episodes.iter().rposition(|e| e.unlocked).unwrap_or(0);
        (last, !self.episodes.is_empty() && last + 1 == self.episodes.len())
    }

    fn update_hideout(&mut self, input: &MenuInput, mv: Option<Dir>, cursor: usize, out: &mut Vec<Action>) {
        let (last, wraps) = self.episode_bounds();
        let next = match mv {
            Some(Dir::Right | Dir::Down) => {
                if cursor < last {
                    Some(cursor + 1)
                } else if wraps {
                    Some(0)
                } else {
                    None
                }
            }
            Some(Dir::Left | Dir::Up) => {
                if cursor > 0 {
                    Some(cursor - 1)
                } else if wraps {
                    Some(last)
                } else {
                    None
                }
            }
            None => None,
        };
        if let Some(n) = next
            && n != cursor
        {
            self.set_cursor(n);
            out.push(Action::Sound(MenuSound::Move));
            out.push(Action::EpisodeSelected(n));
        }
        if input.accept
            && let Some(e) = self.episodes.get(self.cursor())
        {
            let c = self.cursor();
            out.push(Action::Sound(MenuSound::Accept));
            out.push(if e.visited { Action::EnterEpisode(c) } else { Action::PlayEpisodeIntro(c) });
        }
    }

    fn update_map(&mut self, input: &MenuInput, mv: Option<Dir>, cursor: usize, out: &mut Vec<Action>) {
        if let Some(d) = mv
            && let Some(n) = map_neighbour(&self.map, cursor, d)
        {
            self.set_cursor(n);
            out.push(Action::Sound(MenuSound::Move));
        }
        if input.accept {
            match self.map.get(self.cursor()) {
                Some(l) if l.unlocked => {
                    out.push(Action::Sound(MenuSound::Accept));
                    out.push(Action::EnterLevel(self.cursor()));
                    self.close();
                }
                _ => out.push(Action::Sound(MenuSound::Refuse)),
            }
        } else if input.back {
            out.push(Action::Sound(MenuSound::Back));
            self.stack.pop();
            if self.stack.is_empty() {
                self.slide.hide();
                out.push(Action::MapClosed);
            }
        }
    }

    /// Draws the menus.
    pub fn draw(&self, fonts: &FontSet, tables: &TextTables, list: &mut DrawList) {
        let a = self.slide.amount();
        if a <= 0.0 {
            return;
        }
        let start = list.quads.len();
        // Draw the screen under a confirmation too, so the question sits on its menu.
        if let Some(&(top, cursor)) = self.stack.last() {
            match top {
                Screen::Title => self.draw_title(fonts, tables, list),
                Screen::Hideout => self.draw_hideout(fonts, tables, list, cursor),
                Screen::Map => self.draw_map(fonts, list, cursor),
                Screen::CenterScreen => self.draw_center(fonts, tables, list),
                Screen::Prompt(p) => {
                    let p = tables.prompts.get(p);
                    let (title, ts, rs, ids) = p.map_or((&[][..], 1.0, 1.0, &[][..]), |p| {
                        (&p.title[..], p.title_scale, p.response_scale, &p.responses[..])
                    });
                    let texts: Vec<Vec<u8>> = ids.iter().map(|&i| tables.response(i, false).to_vec()).collect();
                    let vertical = p.is_none_or(|p| p.vertical());
                    prompt_box(fonts, list, title, ts, rs, &texts, Some(cursor), vertical, true);
                }
                s => {
                    let ids = self.entries(s);
                    let texts: Vec<Vec<u8>> =
                        ids.iter().enumerate().map(|(k, &i)| self.entry_text(tables, s, k, i)).collect();
                    let pr = tables.prompts.get(prompt_of(s));
                    let title = pr.map_or(&[][..], |p| &p.title[..]);
                    let (ts, rs) = pr.map_or((1.0, 1.0), |p| (p.title_scale, p.response_scale));
                    let vertical = pr.is_none_or(|p| p.vertical());
                    prompt_box(fonts, list, title, ts, rs, &texts, Some(cursor), vertical, true);
                }
            }
        }
        list.fade_from(start, a);
    }

    fn entry_text(&self, tables: &TextTables, s: Screen, k: usize, id: i32) -> Vec<u8> {
        let st = &self.settings;
        match id {
            resp::SLOT_1 | resp::SLOT_2 | resp::SLOT_3 => {
                let slot = (id - resp::SLOT_1) as usize;
                let _ = (s, k);
                tables.slot_description(self.slots[slot].as_ref())
            }
            resp::QUIT => tables.response(id, self.context.slot_in_use).to_vec(),
            resp::VIBRATION => tables.response(id, !st.vibration).to_vec(),
            resp::MUSIC => tables.response(id, !st.music).to_vec(),
            resp::SPEAKERS => tables.response(id, !st.stereo).to_vec(),
            resp::INVERT_BINOC => tables.response(id, st.invert[0]).to_vec(),
            resp::INVERT_TURRET => tables.response(id, st.invert[1]).to_vec(),
            resp::INVERT_JETPACK => tables.response(id, st.invert[2]).to_vec(),
            _ => tables.response(id, false).to_vec(),
        }
    }

    fn draw_title(&self, fonts: &FontSet, tables: &TextTables, list: &mut DrawList) {
        let c = list.canvas;
        let big = TextStyle::default().aligned(Align::Center).scaled(1.6);
        let l = layout(fonts, &tables.title, &big);
        draw_shadowed(&l, fonts, list, (c.width - l.width) * 0.5, c.height * 0.18, 1.0);
        let small = TextStyle::default().aligned(Align::Center).scaled(0.8);
        let start: &[u8] = if self.save_loaded { &tables.press_start_resume } else { &tables.press_start_new };
        for (i, t) in [start, &tables.press_select[..]].iter().enumerate() {
            let l = layout(fonts, t, &small);
            draw_shadowed(&l, fonts, list, (c.width - l.width) * 0.5, c.height * 0.72 + i as f32 * 26.0, 1.0);
        }
    }

    fn draw_center(&self, fonts: &FontSet, tables: &TextTables, list: &mut DrawList) {
        let c = list.canvas;
        let (dx, dy) = (self.settings.center.0 as f32, self.settings.center.1 as f32);
        list.frame(Rect::new(4.0 + dx, 4.0 + dy, c.width - 8.0, c.height - 8.0), 2.0, WHITE);
        let help = tables.prompts.get(prompt::HELP).map_or(&[][..], |p| &p.title[..]);
        prompt_box(fonts, list, help, 0.8, 1.0, &[], None, true, true);
    }

    fn draw_hideout(&self, fonts: &FontSet, tables: &TextTables, list: &mut DrawList, cursor: usize) {
        let c = list.canvas;
        let Some(e) = self.episodes.get(cursor) else { return };
        let title = TextStyle::default().aligned(Align::Center).scaled(1.2);
        let l = layout(fonts, &e.name, &title);
        draw_shadowed(&l, fonts, list, (c.width - l.width) * 0.5, 30.0, 1.0);
        // Arrows where the selection can move.
        let (last, wraps) = self.episode_bounds();
        let arrow = TextStyle::default().scaled(1.2);
        if cursor > 0 || wraps {
            let l = layout(fonts, b"<", &arrow);
            draw_shadowed(&l, fonts, list, 40.0, 30.0, 1.0);
        }
        if cursor < last || wraps {
            let l = layout(fonts, b">", &arrow);
            draw_shadowed(&l, fonts, list, c.width - 40.0 - l.width, 30.0, 1.0);
        }
        // Totals panel: progress, or who briefs the episode.
        let panel = Rect::new(c.width - 200.0, c.height - 130.0, 180.0, 100.0);
        list.rect(panel, rgba8(0, 0, 0, 150));
        list.frame(panel, 1.5, rgba8(230, 200, 120, 255));
        let style = TextStyle::default().scaled(0.7);
        let text: Vec<u8> = if cursor == 0 {
            tables.bentley.clone()
        } else if !e.visited {
            tables.murray.clone()
        } else {
            use crate::tables::{Arg, sprintf};
            sprintf(
                b"&3K&. %d/%d\n&3v&. %d/%d\n&3w&. %d/%d",
                &[
                    Arg::Int(e.keys.0 as i64),
                    Arg::Int(e.keys.1 as i64),
                    Arg::Int(e.vaults.0 as i64),
                    Arg::Int(e.vaults.1 as i64),
                    Arg::Int(e.sprints.0 as i64),
                    Arg::Int(e.sprints.1 as i64),
                ],
            )
        };
        let l = layout(fonts, &text, &style);
        draw_shadowed(&l, fonts, list, panel.x + 12.0, panel.y + 10.0, 1.0);
        if !e.visited {
            let l = layout(fonts, &tables.start_mission, &TextStyle::default().aligned(Align::Center).scaled(0.8));
            draw_shadowed(&l, fonts, list, (c.width - l.width) * 0.5, c.height * 0.8 - 40.0, 1.0);
        }
    }

    fn draw_map(&self, fonts: &FontSet, list: &mut DrawList, cursor: usize) {
        let small = TextStyle::default().scaled(0.55);
        for (i, l) in self.map.iter().enumerate() {
            let sel = i == cursor;
            let col = if !l.unlocked {
                rgba8(90, 90, 90, 255)
            } else if sel {
                rgba8(255, 220, 60, 255)
            } else {
                rgba8(230, 230, 230, 255)
            };
            let s = if sel { 8.0 } else { 6.0 };
            list.rect(Rect::new(l.at.0 - s, l.at.1 - s, 2.0 * s, 2.0 * s), rgba8(0, 0, 0, 160));
            list.frame(Rect::new(l.at.0 - s, l.at.1 - s, 2.0 * s, 2.0 * s), 2.0, col);
            for (k, done) in l.done.iter().enumerate() {
                if *done {
                    list.rect(Rect::new(l.at.0 - 6.0 + k as f32 * 5.0, l.at.1 + s + 2.0, 3.0, 3.0), col);
                }
            }
            if sel {
                let t = layout(fonts, &l.name, &small);
                draw_shadowed(&t, fonts, list, l.at.0 - t.width * 0.5, l.at.1 - s - 4.0 - t.height, 1.0);
            }
        }
    }
}

/// The level reached from `from` in direction `d`: the nearest unlocked one within 60 degrees of the direction.
pub fn map_neighbour(levels: &[MapLevel], from: usize, d: Dir) -> Option<usize> {
    let o = levels.get(from)?.at;
    let (dx, dy) = match d {
        Dir::Up => (0.0, -1.0),
        Dir::Down => (0.0, 1.0),
        Dir::Left => (-1.0, 0.0),
        Dir::Right => (1.0, 0.0),
    };
    levels
        .iter()
        .enumerate()
        .filter(|(i, l)| *i != from && l.unlocked)
        .filter_map(|(i, l)| {
            let (vx, vy) = (l.at.0 - o.0, l.at.1 - o.1);
            let along = vx * dx + vy * dy;
            let dist = crate::sqrt(vx * vx + vy * vy);
            // cos 60 = 0.5
            (along > 0.5 * dist).then_some((i, dist))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The prompt record a menu screen is drawn with (its title, scales and response layout).
fn prompt_of(s: Screen) -> usize {
    match s {
        Screen::Prompt(p) => p,
        Screen::Pause => prompt::PAUSED,
        Screen::Options => prompt::OPTIONS,
        Screen::Controls => prompt::CONTROLS,
        Screen::Slots(SlotMode::Save) => prompt::SAVE_GAME,
        Screen::Slots(SlotMode::Load) => prompt::LOAD_GAME,
        Screen::Slots(SlotMode::New) => prompt::NEW_GAME,
        Screen::Confirm(Confirm::Erase(_)) => prompt::ERASE,
        Screen::Confirm(Confirm::Overwrite(..)) => prompt::OVERWRITE,
        _ => prompt::REALLY_QUIT,
    }
}

/// Gap between responses laid out side by side, in pixels.
pub const SIDE_BY_SIDE_GAP: f32 = 32.0;

/// Draws a centred prompt box: the title, then the responses, the current one highlighted. `vertical`: one
/// response per line; otherwise all on one line, side by side (the prompt's layout, ui-and-menus.md 2).
#[allow(clippy::too_many_arguments)]
pub fn prompt_box(
    fonts: &FontSet,
    list: &mut DrawList,
    title: &[u8],
    title_scale: f32,
    response_scale: f32,
    responses: &[Vec<u8>],
    cursor: Option<usize>,
    vertical: bool,
    frame: bool,
) -> Rect {
    let c = list.canvas;
    let wrap = c.width * 0.7;
    let ts = TextStyle::default().aligned(Align::Center).scaled(title_scale * 0.9).wrapped(wrap);
    let rs = TextStyle::default().aligned(Align::Center).scaled(response_scale * 0.8);
    let tl = layout(fonts, title, &ts);
    let rl: Vec<_> = responses.iter().map(|r| layout(fonts, r, &rs)).collect();
    let gap = 8.0;
    // Side by side: one row as wide as the responses and the gaps between them.
    let row_w = rl.iter().map(|l| l.width).sum::<f32>() + SIDE_BY_SIDE_GAP * rl.len().saturating_sub(1) as f32;
    let row_h = rl.iter().map(|l| l.height).fold(0.0, f32::max) + 2.0;
    let responses_w = if vertical { rl.iter().map(|l| l.width).fold(0.0, f32::max) } else { row_w };
    let responses_h = if vertical { rl.iter().map(|l| l.height + 2.0).sum::<f32>() } else { row_h };
    let w = responses_w.max(tl.width) + 40.0;
    let h = tl.height + responses_h + if rl.is_empty() { 0.0 } else { gap } + 24.0;
    let r = Rect::new((c.width - w) * 0.5, (c.height - h) * 0.5, w, h);
    if frame {
        list.rect(r, rgba8(10, 14, 30, 200));
        list.frame(r, 1.5, rgba8(230, 200, 120, 255));
    }
    let mut y = r.y + 12.0;
    draw_shadowed(&tl, fonts, list, r.x + (w - tl.width) * 0.5, y, 1.0);
    y += tl.height + gap;
    let mut row_x = r.x + (w - row_w) * 0.5;
    for (i, l) in rl.iter().enumerate() {
        let sel = cursor == Some(i);
        let x = if vertical { r.x + (w - l.width) * 0.5 } else { row_x };
        if sel {
            let hl = if vertical {
                Rect::new(r.x + 10.0, y - 1.0, w - 20.0, l.height + 2.0)
            } else {
                Rect::new(x - 6.0, y - 1.0, l.width + 12.0, l.height + 2.0)
            };
            list.rect(hl, rgba8(240, 200, 60, 70));
        }
        let from = list.quads.len();
        draw_shadowed(l, fonts, list, x, y, 1.0);
        if sel {
            // Highlight: tint the (non-shadow) glyphs yellow.
            let n = (list.quads.len() - from) / 2;
            for q in &mut list.quads[from + n..] {
                q.color[2] *= 0.35;
            }
        }
        if vertical {
            y += l.height + 2.0;
        } else {
            row_x += l.width + SIDE_BY_SIDE_GAP;
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::Prompt;

    pub(crate) fn tables() -> TextTables {
        let mut t = TextTables {
            responses: (0..27).map(|i| alloc::format!("R{i}").into_bytes()).collect(),
            two_state: (0..8)
                .map(|i| (alloc::format!("A{i}").into_bytes(), alloc::format!("B{i}").into_bytes()))
                .collect(),
            slot_format: b"%d%% - %s - %d:%02d".to_vec(),
            title: b"TITLE".to_vec(),
            press_start_new: b"START".to_vec(),
            press_select: b"SELECT".to_vec(),
            ..TextTables::default()
        };
        t.prompts = (0..33)
            .map(|i| Prompt {
                title: alloc::format!("P{i}").into_bytes(),
                title_scale: 1.0,
                response_scale: 1.0,
                // As on the disc: the yes/no questions lay their answers side by side.
                layout: if [1, 2, 4, 7, 14, 18, 25, 26, 27, 28, 29, 30].contains(&i) { 0 } else { 1 },
                responses: if i == 1 { vec![0, 1] } else { vec![] },
            })
            .collect();
        t
    }

    fn press(f: impl FnOnce(&mut MenuInput)) -> MenuInput {
        let mut i = MenuInput::default();
        f(&mut i);
        i
    }

    fn down(m: &mut Menus, t: &TextTables) {
        let d = press(|i| i.dpad[1] = true);
        m.update(&d, 0.016, t);
        m.update(&MenuInput::default(), 0.016, t);
    }

    #[test]
    fn pause_entries_follow_the_rules() {
        let mut c = PauseContext::default();
        assert_eq!(c.entries(), [resp::RETURN_TO_GAME, resp::OPTIONS, resp::EXIT_LEVEL, resp::QUIT]);
        c.prologue = true;
        assert_eq!(c.entries(), [resp::RETURN_TO_GAME, resp::OPTIONS, resp::QUIT]);
        c = PauseContext {
            world_name: b"Piranha Lake".to_vec(),
            in_race: true,
            sprint_running: true,
            has_map: true,
            hideout: true,
            ..PauseContext::default()
        };
        assert_eq!(
            c.entries(),
            [
                resp::RETURN_TO_GAME,
                resp::RESTART_RACE,
                resp::RESTART_SPRINT,
                resp::RESTART_LEVEL,
                resp::VIEW_MAP,
                resp::OPTIONS,
                resp::EXIT_TO_HIDEOUT,
                resp::QUIT
            ]
        );
    }

    #[test]
    fn options_from_title_and_toggles() {
        let t = tables();
        let mut m = Menus::default();
        m.open_title();
        let a = m.update(&press(|i| i.select = true), 0.016, &t);
        assert!(a.contains(&Action::Sound(MenuSound::Accept)));
        assert_eq!(m.screen(), Some(Screen::Options));
        assert_eq!(m.entries(Screen::Options)[0], resp::NEW_GAME);
        assert!(!m.entries(Screen::Options).contains(&resp::SAVE_GAME));
        // Down twice to Vibrations, toggle it.
        down(&mut m, &t);
        down(&mut m, &t);
        assert_eq!(m.entries(Screen::Options)[m.cursor()], resp::VIBRATION);
        let a = m.update(&press(|i| i.accept = true), 0.016, &t);
        assert!(a.iter().any(|x| matches!(x, Action::Settings(s) if !s.vibration)));
        assert_eq!(m.entry_text(&t, Screen::Options, 0, resp::VIBRATION), b"B2");
        // Back returns to the title.
        m.update(&press(|i| i.back = true), 0.016, &t);
        assert_eq!(m.screen(), Some(Screen::Title));
        let a = m.update(&press(|i| i.start = true), 0.016, &t);
        assert!(a.contains(&Action::TitleStart { resume: false }));
        assert!(m.screen().is_none());
    }

    #[test]
    fn save_flow_with_overwrite_and_erase() {
        let t = tables();
        let mut m = Menus::default();
        m.slots[1] = Some(SlotInfo { percent: 10, world: b"W".to_vec(), minutes: 61 });
        m.open_pause(PauseContext { slot_in_use: true, ..PauseContext::default() });
        // Pause -> Options -> Save Game...
        down(&mut m, &t);
        m.update(&press(|i| i.accept = true), 0.016, &t);
        assert_eq!(m.screen(), Some(Screen::Options));
        down(&mut m, &t);
        assert_eq!(m.entries(Screen::Options)[m.cursor()], resp::SAVE_GAME);
        m.update(&press(|i| i.accept = true), 0.016, &t);
        assert_eq!(m.screen(), Some(Screen::Slots(SlotMode::Save)));
        assert_eq!(m.entry_text(&t, Screen::Slots(SlotMode::Save), 1, resp::SLOT_2), b"10% - W - 1:01");
        assert_eq!(m.entry_text(&t, Screen::Slots(SlotMode::Save), 0, resp::SLOT_1), b"B0");
        // Slot 2 is used: overwrite question, default No.
        down(&mut m, &t);
        m.update(&press(|i| i.accept = true), 0.016, &t);
        assert_eq!(m.screen(), Some(Screen::Confirm(Confirm::Overwrite(SlotMode::Save, 1))));
        assert_eq!(m.cursor(), 1);
        let a = m.update(&press(|i| i.accept = true), 0.016, &t);
        assert!(!a.iter().any(|x| matches!(x, Action::Save(_))));
        assert_eq!(m.screen(), Some(Screen::Slots(SlotMode::Save)));
        // Erase it instead.
        m.update(&press(|i| i.erase = true), 0.016, &t);
        assert_eq!(m.screen(), Some(Screen::Confirm(Confirm::Erase(1))));
        m.update(&press(|i| i.dpad[2] = true), 0.016, &t); // Yes is to the left of No
        m.update(&MenuInput::default(), 0.016, &t);
        let a = m.update(&press(|i| i.accept = true), 0.016, &t);
        assert!(a.contains(&Action::EraseSlot(1)));
        assert!(m.slots[1].is_none());
        // Now saving there needs no question.
        let a = m.update(&press(|i| i.accept = true), 0.016, &t);
        assert!(a.contains(&Action::Save(1)));
        assert!(m.screen().is_none());
    }

    #[test]
    fn pause_closes_and_quits() {
        let t = tables();
        let mut m = Menus::default();
        m.open_pause(PauseContext { slot_in_use: true, ..PauseContext::default() });
        let a = m.update(&press(|i| i.start = true), 0.016, &t);
        assert!(a.contains(&Action::Resume));
        m.open_pause(PauseContext { slot_in_use: true, ..PauseContext::default() });
        for _ in 0..3 {
            down(&mut m, &t);
        }
        assert_eq!(m.context.entries()[m.cursor()], resp::QUIT);
        assert_eq!(m.entry_text(&t, Screen::Pause, 3, resp::QUIT), b"B1", "Save and Quit");
        m.update(&press(|i| i.accept = true), 0.016, &t);
        m.update(&press(|i| i.dpad[2] = true), 0.016, &t); // Yes is to the left of No
        let a = m.update(&press(|i| i.accept = true), 0.016, &t);
        assert!(a.contains(&Action::Quit { save: true }));
    }

    #[test]
    fn prompt_answers() {
        let t = tables();
        let mut m = Menus::default();
        m.open_prompt(prompt::GAME_OVER);
        // Yes and No sit side by side (layout 0): down does nothing, right moves to No.
        down(&mut m, &t);
        assert_eq!(m.cursor(), 0);
        m.update(&press(|i| i.dpad[3] = true), 0.016, &t);
        m.update(&MenuInput::default(), 0.016, &t);
        assert_eq!(m.cursor(), 1);
        let a = m.update(&press(|i| i.accept = true), 0.016, &t);
        assert!(a.contains(&Action::Answer { prompt: 1, response: resp::NO }));
        // Back on a yes/no question answers No.
        m.open_prompt(prompt::GAME_OVER);
        let a = m.update(&press(|i| i.back = true), 0.016, &t);
        assert!(a.contains(&Action::Answer { prompt: 1, response: resp::NO }));
    }

    #[test]
    fn autosave_notice_once_per_new_slot() {
        let t = tables();
        let mut m = Menus::default();
        assert!(!m.before_autosave(), "no new game: no notice");
        m.autosave_notice_pending = true;
        assert!(m.before_autosave());
        assert_eq!(m.screen(), Some(Screen::Prompt(prompt::AUTOSAVE_NOTICE)));
        let a = m.update(&press(|i| i.accept = true), 0.016, &t);
        assert!(a.iter().any(|x| matches!(x, Action::Answer { prompt: prompt::AUTOSAVE_NOTICE, .. })));
        assert!(!m.before_autosave(), "only once");
    }

    #[test]
    fn prompt_layouts_draw_lists_and_rows() {
        let fonts = crate::font::tests::test_set();
        let responses = [b"Yes".to_vec(), b"No".to_vec()];
        let mut a = DrawList::new(crate::draw::Canvas::default());
        let tall = prompt_box(&fonts, &mut a, b"Q", 1.0, 1.0, &responses, Some(0), true, true);
        let mut b = DrawList::new(crate::draw::Canvas::default());
        let wide = prompt_box(&fonts, &mut b, b"Q", 1.0, 1.0, &responses, Some(0), false, true);
        // Side by side: one line less tall, wider by at least the gap.
        assert!(wide.h < tall.h, "{wide:?} {tall:?}");
        assert!(wide.w > tall.w);
    }

    #[test]
    fn hideout_bounds_and_wrap() {
        let t = tables();
        let mut m = Menus::default();
        let ep = |unlocked, visited| Episode { unlocked, visited, ..Episode::default() };
        m.open_hideout(vec![ep(true, true), ep(true, true), ep(true, false), ep(false, false)], 0);
        let left = press(|i| i.dpad[2] = true);
        let right = press(|i| i.dpad[3] = true);
        let a = m.update(&left, 0.016, &t);
        assert!(a.is_empty(), "stops at the first");
        m.update(&MenuInput::default(), 0.016, &t);
        for _ in 0..5 {
            m.update(&right, 0.016, &t);
            m.update(&MenuInput::default(), 0.016, &t);
        }
        assert_eq!(m.cursor(), 2, "stops at the last unlocked");
        let a = m.update(&press(|i| i.accept = true), 0.016, &t);
        assert!(a.contains(&Action::PlayEpisodeIntro(2)));
        // All unlocked: wraps.
        m.open_hideout(vec![ep(true, true), ep(true, true)], 0);
        let a = m.update(&left, 0.016, &t);
        assert!(a.contains(&Action::EpisodeSelected(1)));
        m.update(&MenuInput::default(), 0.016, &t);
        let a = m.update(&press(|i| i.accept = true), 0.016, &t);
        assert!(a.contains(&Action::EnterEpisode(1)));
    }

    #[test]
    fn map_moves_to_neighbours() {
        let t = tables();
        let lv = |x, y, unlocked| MapLevel { at: (x, y), unlocked, ..MapLevel::default() };
        let levels =
            vec![lv(100.0, 100.0, true), lv(200.0, 110.0, true), lv(100.0, 200.0, false), lv(300.0, 100.0, true)];
        assert_eq!(map_neighbour(&levels, 0, Dir::Right), Some(1));
        assert_eq!(map_neighbour(&levels, 0, Dir::Down), None, "locked levels are skipped");
        assert_eq!(map_neighbour(&levels, 1, Dir::Left), Some(0));
        let mut m = Menus::default();
        m.open_map(levels, 0);
        m.update(&press(|i| i.dpad[3] = true), 0.016, &t);
        assert_eq!(m.cursor(), 1);
        let a = m.update(&press(|i| i.accept = true), 0.016, &t);
        assert!(a.contains(&Action::EnterLevel(1)));
    }

    #[test]
    fn draws_every_screen() {
        let f = crate::font::tests::test_set();
        let t = tables();
        let mut m = Menus::default();
        let screens = [
            Screen::Title,
            Screen::Pause,
            Screen::Options,
            Screen::Controls,
            Screen::CenterScreen,
            Screen::Slots(SlotMode::Load),
            Screen::Confirm(Confirm::Quit),
            Screen::Prompt(1),
        ];
        for s in screens {
            m.open(s);
            m.update(&MenuInput::default(), 0.3, &t);
            let mut d = DrawList::new(crate::draw::Canvas::default());
            m.draw(&f, &t, &mut d);
            assert!(!d.quads.is_empty(), "{s:?}");
        }
        m.open_hideout(vec![Episode { name: b"EP".to_vec(), unlocked: true, ..Episode::default() }], 0);
        m.update(&MenuInput::default(), 0.3, &t);
        let mut d = DrawList::new(crate::draw::Canvas::default());
        m.draw(&f, &t, &mut d);
        assert!(!d.quads.is_empty());
    }
}
