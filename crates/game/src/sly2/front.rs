//! The Sly 2 session's front end (level-flow.md 3 and 4, saving.md 3, movies.md; `ui::sly2::front`):
//!
//! - **Start-up** ([`Sly2Session::start_front`]): the boot world (row 0) loads as the title's backdrop, the logo movie
//!   (`0x808`, executable file record 23) plays, then the title shows over the world.
//! - **New game, continue, load** with the three slots of the platform's store ([`SlotAccess`]); the chosen slot
//!   takes the autosaves and Save Game from then on.
//! - **The episode menu**: whenever the menu world (row 1) loads (the end of the intro, Exit to Episode Menu, a
//!   continue into a finished episode), the episode menu shows. The episodes that can be chosen and the movie list
//!   follow level-flow.md 4. Starting an episode never played (its `dt` is 0) plays its opening movie first; then
//!   its hub loads at point 393.
//! - **Movies** play through [`crate::movie::MovieScreen`] while the world waits; + skips.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

use engine::game::sly2_jobs::{self, flow};
use engine::game::systems::FrameInput;
use formats::iso::ReadAt;
use platform_api::{Buttons, FileSystem};
use render::Backend;
use save::Slot;
use save::sly2::slot::Loaded;
use save::sly2::{Progress, Sly2Store};
use ui::sly2::front::{EpisodeRow, FrontAction, FrontMenu, MovieRow, SlotRow, Texts};

use super::Sly2Session;
use super::progress::Pending;

/// The start-up logo movie (level-flow.md 3: executable file record 23, identity `0x808`).
pub const LOGO_MOVIE: u32 = 0x808;
/// Each episode's opening movie, episodes 1 to 8 (the menu world's exits, level-flow.md 4).
pub const OPENINGS: [u32; 8] = [0x7D9, 0x7DE, 0x7E3, 0x7E7, 0x7EC, 0x7F1, 0x7F6, 0x7FA];
/// Each episode's own movies (movies.md "Where the movies play"): episodes 1 to 8; episode 8 has two.
pub const EPISODE_MOVIES: [&[u32]; 8] =
    [&[0x7DA], &[0x7DF], &[0x7E4], &[0x7E8], &[0x7ED], &[0x7F2], &[0x7F7], &[0x7FB, 0x7D5]];
/// String IDs: the title lines (5, 6), Continue (56), New Game (30), Load Game (31), Empty (69), the overwrite
/// question (28), the game's name (16, also the episode menu's heading), "Start Episode" (108), "Watch a Movie"
/// (118) and the first episode title (109, episode 0).
pub mod strings {
    pub const PRESS_NEW: i16 = 5;
    pub const PRESS_RESUME: i16 = 6;
    pub const NAME: i16 = 16;
    pub const CONTINUE: i16 = 56;
    pub const NEW_GAME: i16 = 30;
    pub const LOAD_GAME: i16 = 31;
    pub const EMPTY: i16 = 69;
    pub const OVERWRITE: i16 = 28;
    pub const START_EPISODE: i16 = 108;
    pub const FIRST_TITLE: i16 = 109;
    pub const WATCH_MOVIE: i16 = 118;
}

/// The platform's save slots, for the front end and the saver.
pub trait SlotAccess {
    /// Loads slot 0 to 2 onto `base` (this disc's tree).
    fn load(&self, slot: usize, base: &Progress) -> Result<Loaded, String>;
    /// Saves to a slot.
    fn save(&self, slot: usize, p: &Progress) -> Result<u64, String>;
    /// The slot saved last.
    fn newest(&self) -> Option<usize>;
}

impl<F: FileSystem> SlotAccess for Sly2Store<F> {
    fn load(&self, slot: usize, base: &Progress) -> Result<Loaded, String> {
        let s = u8::try_from(slot).ok().and_then(Slot::new).ok_or("no such slot")?;
        Sly2Store::load(self, s, base).map_err(|e| alloc::format!("{e}"))
    }
    fn save(&self, slot: usize, p: &Progress) -> Result<u64, String> {
        let s = u8::try_from(slot).ok().and_then(Slot::new).ok_or("no such slot")?;
        Sly2Store::save(self, s, p).map_err(|e| alloc::format!("{e}"))
    }
    fn newest(&self) -> Option<usize> {
        Sly2Store::newest(self).map(|(s, _)| usize::from(s.index()))
    }
}

/// What happens when the playing movie ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AfterMovie {
    Nothing,
    /// Load an episode's hub.
    Episode(u8),
}

/// The front end's state in the session.
pub struct Front {
    pub menu: Option<FrontMenu>,
    pub slots: Option<Rc<dyn SlotAccess>>,
    /// The slot in play.
    pub slot: Option<usize>,
    pub movies: crate::movie::MovieScreen,
    after: AfterMovie,
}

impl Default for Front {
    fn default() -> Self {
        Front {
            menu: None,
            slots: None,
            slot: None,
            movies: crate::movie::MovieScreen::default(),
            after: AfterMovie::Nothing,
        }
    }
}

impl<B: Backend, R: ReadAt> Sly2Session<B, R> {
    /// The save slots (the platform's store). Autosaves go to the slot chosen at the title.
    pub fn set_slots(&mut self, slots: Box<dyn SlotAccess>) {
        self.front.slots = Some(Rc::from(slots));
    }

    /// Movies, read through `opener` (which opens the disc image again): the executable file table's movies.
    pub fn set_movie_source(&mut self, opener: crate::movie::Opener) {
        let Some(d) = self.disc.as_ref() else { return };
        let clips = crate::movie::Movies::clips_of_records(&video::movies::sly2_movies(&d.files));
        self.front.movies.movies = Some(crate::movie::Movies::from_clips(clips, opener));
    }

    /// Starts movie `identity` (its sound on the sound engine when there is one). Returns whether it plays.
    pub fn start_movie(&mut self, identity: u32) -> bool {
        let sound = self.sound.as_mut().map(|s| &mut s.engine);
        self.front.movies.start(identity, sound)
    }

    /// Whether a movie plays.
    pub fn movie_playing(&self) -> bool {
        self.front.movies.playing()
    }

    /// Whether the front end (title, slots, episode menu) is on screen.
    pub fn front_open(&self) -> bool {
        self.front.menu.as_ref().is_some_and(FrontMenu::visible)
    }

    /// The front end's page, if open.
    pub fn front_page(&self) -> Option<ui::sly2::front::Page> {
        self.front.menu.as_ref().filter(|m| m.visible()).map(|m| m.page)
    }

    /// The start-up sequence (level-flow.md 3): the boot world as the backdrop, the logo movie, the title.
    pub fn start_front(&mut self) -> Result<(), String> {
        self.load_row(sly2_jobs::BOOT_ROW as usize)?;
        self.start_movie(LOGO_MOVIE);
        self.open_title();
        Ok(())
    }

    /// A world string as font codes, or `fallback`.
    fn front_text(&self, id: i16, fallback: &str) -> Vec<u8> {
        let s = self.sound.as_ref().and_then(|s| s.string(id));
        super::layer::latin1(s.as_deref().unwrap_or(fallback))
    }

    fn front_texts(&self) -> Texts {
        use strings::*;
        Texts {
            name: self.front_text(NAME, "Sly 2"),
            press_new: self.front_text(PRESS_NEW, "Press START button for New Game"),
            press_resume: self.front_text(PRESS_RESUME, "Press START button to Resume Game"),
            continue_game: self.front_text(CONTINUE, "Continue"),
            new_game: self.front_text(NEW_GAME, "New Game"),
            load_game: self.front_text(LOAD_GAME, "Load Game"),
            empty: self.front_text(EMPTY, "Empty"),
            overwrite: self.front_text(OVERWRITE, "Do you want to overwrite this game?"),
            episodes: self.front_text(NAME, "Sly 2"),
            start_episode: self.front_text(START_EPISODE, "Start Episode"),
            watch_movie: self.front_text(WATCH_MOVIE, "Watch a Movie"),
        }
    }

    /// The tree of the loaded world, as an empty progress block.
    fn base_progress(&self) -> Option<Progress> {
        self.game.as_ref().and_then(|g| sly2_jobs::tree_of(&g.world)).map(Progress::new)
    }

    /// The three slots as the title shows them, and the newest.
    fn slot_rows(&self) -> ([SlotRow; 3], Option<usize>) {
        let mut rows: [SlotRow; 3] = Default::default();
        let (Some(slots), Some(base)) = (self.front.slots.as_ref(), self.base_progress()) else { return (rows, None) };
        for (i, row) in rows.iter_mut().enumerate() {
            if let Ok(l) = slots.load(i, &base)
                && !l.unused
            {
                let episode = l.progress.int(&["episodeCurrent"]).clamp(0, 8);
                let title = self.front_text(strings::FIRST_TITLE + episode as i16, "");
                let mut text = alloc::format!("Episode {episode}").into_bytes();
                if !title.is_empty() {
                    text.extend_from_slice(b": ");
                    text.extend_from_slice(&title);
                }
                *row = SlotRow { used: true, text };
            }
        }
        let newest = slots.newest().filter(|&n| rows.get(n).is_some_and(|r| r.used));
        (rows, newest)
    }

    /// Opens the title over the current world.
    pub fn open_title(&mut self) {
        let (slots, newest) = self.slot_rows();
        self.front.menu = Some(FrontMenu::title(self.front_texts(), slots, newest));
    }

    /// The progress values of `episode` (its hub row's save path): (`dt`, `percent_complete`).
    fn episode_values(&self, p: &Progress, episode: u8) -> (i32, i32) {
        let Some(w) = self.game.as_ref().and_then(|g| g.world.sly2.as_deref()) else { return (0, 0) };
        let Some(row) = w.header.level_rows.iter().find(|r| r.hub && r.episode == episode) else { return (0, 0) };
        let base: Vec<u16> = row.save_path.iter().copied().take(2).collect();
        let value = |name: &str| {
            let Some(id) = p.tree.name_id(name) else { return 0 };
            let mut path = base.clone();
            path.push(id);
            p.get(&path, None).map_or(0, |v| v.as_i32())
        };
        (value("dt"), value("percent_complete"))
    }

    /// The highest episode that can be chosen (level-flow.md 4): counting down from 8, the first played or whose
    /// previous episode is complete; episode 1 always.
    pub fn highest_episode(&self, p: &Progress) -> u8 {
        (2..=8u8)
            .rev()
            .find(|&e| self.episode_values(p, e).0 > 0 || self.episode_values(p, e - 1).1 >= 100)
            .unwrap_or(1)
    }

    /// Opens the episode menu (the menu world is loaded).
    pub fn open_episode_menu(&mut self) {
        let Some(p) = self.progress().cloned() else { return };
        let highest = self.highest_episode(&p);
        let episodes = (1..=highest)
            .map(|e| EpisodeRow {
                episode: e,
                title: self.front_text(strings::FIRST_TITLE + e as i16, &alloc::format!("Episode {e}")),
            })
            .collect();
        // Two movies per open episode, less 2 if the newest was never played, less 1 if it is not complete.
        let (dt, percent) = self.episode_values(&p, highest);
        let mut count = 2 * highest as usize;
        if dt == 0 {
            count -= 2;
        } else if percent < 100 {
            count -= 1;
        }
        let mut movies = Vec::new();
        for e in 1..=highest {
            let title = self.front_text(strings::FIRST_TITLE + e as i16, &alloc::format!("Episode {e}"));
            let ids = core::iter::once(OPENINGS[e as usize - 1]).chain(EPISODE_MOVIES[e as usize - 1].iter().copied());
            for (k, id) in ids.enumerate() {
                let mut t = title.clone();
                t.extend_from_slice(alloc::format!(" ({})", k + 1).as_bytes());
                movies.push(MovieRow { identity: id, title: t });
            }
        }
        movies.truncate(count);
        let texts = self.front_texts();
        self.front.menu = Some(FrontMenu::episode_menu(texts, episodes, movies));
    }

    /// The saver for the slot in play.
    fn use_slot(&mut self, slot: usize) {
        self.front.slot = Some(slot);
        if let Some(slots) = self.front.slots.clone() {
            self.set_saver(Some(Box::new(move |p| slots.save(slot, p))));
        }
    }

    /// A new game in `slot` (its options kept).
    fn front_new_game(&mut self, slot: usize) -> Result<(), String> {
        let options = match (self.front.slots.as_ref(), self.base_progress()) {
            (Some(s), Some(base)) => s.load(slot, &base).ok().map(|l| l.progress),
            _ => None,
        };
        self.use_slot(slot);
        self.new_game(options)
    }

    /// Loads `slot` and continues it (level-flow.md 3).
    fn front_load(&mut self, slot: usize) -> Result<(), String> {
        let base = self.base_progress().ok_or("no Sly 2 world loaded")?;
        let loaded = self.front.slots.as_ref().ok_or("no save slots")?.load(slot, &base)?;
        self.use_slot(slot);
        self.continue_from(loaded.progress)
    }

    /// Starts `episode` from the episode menu: its opening movie first when it was never played, then its hub.
    fn front_episode(&mut self, episode: u8) -> Result<(), String> {
        let never = self.progress().is_some_and(|p| self.episode_values(p, episode).0 == 0);
        if never && self.start_movie(OPENINGS[episode as usize - 1]) {
            self.front.after = AfterMovie::Episode(episode);
            return Ok(());
        }
        self.load_episode_hub(episode)
    }

    /// Loads `episode`'s hub at point 393 with the live progress.
    fn load_episode_hub(&mut self, episode: u8) -> Result<(), String> {
        let row = self.hub_row(episode).ok_or("no hub for the episode")?;
        let g = self.game.as_mut().ok_or("no world")?;
        let (progress, carry) = sly2_jobs::detach(&mut g.world).unzip();
        self.pending = Pending { progress, carry, point: Some(flow::DEFAULT_POINT), task: None };
        self.load_row(row)
    }

    /// Called by `load_row` once a world is in: the menu world shows the episode menu.
    pub(super) fn front_world_loaded(&mut self, row: usize) {
        if row == flow::MENU_ROW as usize && !self.front_open() {
            self.open_episode_menu();
        }
    }

    /// The front end's part of a frame: movies first (they hold everything; + skips), then the open menu. Returns the
    /// input the game gets this frame, or `None` while a movie holds the world.
    pub(super) fn update_front(&mut self, input: &FrameInput, measured: f32) -> Option<FrameInput> {
        if self.front.movies.playing() {
            let skip = input.pressed & Buttons::PLUS.0 != 0;
            let sound = self.sound.as_mut().map(|s| &mut s.engine);
            if !self.front.movies.update(measured, skip, sound) {
                let after = core::mem::replace(&mut self.front.after, AfterMovie::Nothing);
                if let AfterMovie::Episode(e) = after
                    && let Err(e) = self.load_episode_hub(e)
                {
                    self.message = Some(e);
                }
            }
            return None;
        }
        let menu = self.front.menu.as_mut()?;
        let (mi, circle) = super::screens::menu_input(input);
        let action = menu.update(engine::game::clock::frame_step(measured), &mi, circle);
        // An action that leaves the menu drops it at once (the next world may open the episode menu).
        if !menu.visible() || !matches!(action, FrontAction::None | FrontAction::PlayMovie(_)) {
            self.front.menu = None;
        }
        let r = match action {
            FrontAction::None => Ok(()),
            FrontAction::NewGame(s) => self.front_new_game(s),
            FrontAction::LoadGame(s) | FrontAction::Continue(s) => self.front_load(s),
            FrontAction::StartEpisode(e) => self.front_episode(e),
            FrontAction::PlayMovie(id) => {
                self.start_movie(id);
                Ok(())
            }
        };
        if let Err(e) = r {
            self.message = Some(e);
            // A load that failed leaves no blank screen: the page the action came from opens again.
            if self.front.menu.is_none() && !self.front.movies.playing() {
                match action {
                    FrontAction::StartEpisode(_) => self.open_episode_menu(),
                    FrontAction::None | FrontAction::PlayMovie(_) => {}
                    _ => self.open_title(),
                }
            }
        }
        // The world behind the menu runs without the player's input.
        Some(if self.front.menu.is_some() { FrameInput::default() } else { *input })
    }
}
