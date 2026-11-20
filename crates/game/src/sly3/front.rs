//! The Sly 3 session's front end (level-flow.md 3, saving.md 3; `ui::sly2::front`, shared with Sly 2):
//!
//! - **Start-up** ([`Sly3Session::start_front`]): the menu world (`dvd_menu`, slot 0) loads as the title's backdrop,
//!   the two logo movies (identities 338 and 339) play, then the title shows. The disc's own boot goes from the logos
//!   to Kaine Island's prologue before the title [O]; SlyNX shows the title first and plays the prologue as the new
//!   game's start (episode 1 with the opening not yet played: `m_ext`, task 1798) [working choice].
//! - **New game, continue, load** with the three Sly 3 slots of the platform's store
//!   (`save::sly3::store`, through [`SlotAccess`]); the chosen slot takes the autosaves.
//! - **Continue and load** start the saved episode (`episodeCurrent`) by the episode start rules
//!   ([`super::flow::episode_start`]), with the progress values they read taken from the save.
//! - **The episode menu**: whenever `dvd_menu` loads (an exit to it, Exit to Game Menu), the episodes show: the Hazard
//!   Room (episode 2, string 171) and episodes 3 to 8 ("Episode 1" to "Episode 6", strings 5558 to 5563, with their
//!   titles 164 to 169). Episodes up to the best one reached (`episodeBest`, at least episode 3) can be chosen
//!   [working choice: the disc's rule is not in the spec]. Sly 3's movie list is not known, so the menu offers none.

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use engine::game::systems::FrameInput;
use formats::iso::ReadAt;
use platform_api::Buttons;
use render::Backend;
use save::sly2::Progress;
use ui::sly2::front::{EpisodeRow, FrontAction, FrontMenu, SlotRow, Texts};

pub use crate::sly2::front::SlotAccess;

use super::Sly3Session;
use super::flow;

/// The start-up logo movies (level-flow.md 3).
pub const LOGO_MOVIES: [u32; 2] = [338, 339];
/// The menu world's slot.
pub const MENU_SLOT: usize = 0;

/// String IDs of the Sly 3 menu world.
pub mod strings {
    pub const NAME: i16 = 25;
    pub const PRESS_NEW: i16 = 13;
    pub const PRESS_RESUME: i16 = 14;
    pub const NEW_GAME: i16 = 42;
    pub const LOAD_GAME: i16 = 43;
    pub const CONTINUE: i16 = 71;
    pub const EMPTY: i16 = 85;
    pub const OVERWRITE: i16 = 40;
    pub const START_EPISODE: i16 = 133;
    pub const MOVIES: i16 = 173;
    pub const HAZARD_ROOM: i16 = 171;
    /// "Episode 1" (episode 3, Venice) to "Episode 6" (episode 8).
    pub const FIRST_EPISODE_LABEL: i16 = 5558;
    /// The title of episode 3; episodes 4 to 8 follow.
    pub const FIRST_TITLE: i16 = 164;
}

/// The front end's state in the session.
#[derive(Default)]
pub struct Front {
    pub menu: Option<FrontMenu>,
    pub slots: Option<Rc<dyn SlotAccess>>,
    /// The slot in play.
    pub slot: Option<usize>,
    pub movies: crate::movie::MovieScreen,
    /// Movies waiting to play after the current one.
    queue: Vec<u32>,
}

/// The progress values the episode start reads (level-flow.md 3), from a progress block.
pub fn episode_progress(p: &Progress) -> flow::EpisodeProgress {
    let done = |task: &str| p.int(&[task, "is_complete"]) != 0;
    flow::EpisodeProgress {
        china_recon_complete: done("t4_recon_complete"),
        treasure_complete: done("t5_treasure_complete"),
        heist_complete: done("t5_heist_complete"),
        // Interpretation: the opening has been played once a later episode was reached [working choice].
        opening_played: p.int(&["episodeBest"]) >= 2,
        kaine_value: p.int(&["oidCheckpointForce"]),
    }
}

impl<B: Backend, R: ReadAt> Sly3Session<B, R> {
    /// The save slots (the platform's `save::sly3::store`). Autosaves go to the slot chosen at the title.
    pub fn set_slots(&mut self, slots: Box<dyn SlotAccess>) {
        self.front.slots = Some(Rc::from(slots));
    }

    /// Movies, read through `opener` (which opens the disc image again): the executable's movie table.
    pub fn set_movie_source(&mut self, opener: crate::movie::Opener) {
        let Some(d) = self.disc.as_ref() else { return };
        let clips = crate::movie::Movies::clips_of_records(&video::movies::sly3_movies(&d.movies));
        self.front.movies.movies = Some(crate::movie::Movies::from_clips(clips, opener));
    }

    /// Starts movie `identity` (its sound on the sound engine when there is one). Returns whether it plays.
    pub fn start_movie(&mut self, identity: u32) -> bool {
        self.front.movies.start(identity, self.sound.as_mut())
    }

    /// Whether a movie plays.
    pub fn movie_playing(&self) -> bool {
        self.front.movies.playing()
    }

    /// Whether the front end is on screen.
    pub fn front_open(&self) -> bool {
        self.front.menu.as_ref().is_some_and(FrontMenu::visible)
    }

    /// The front end's page, if open.
    pub fn front_page(&self) -> Option<ui::sly2::front::Page> {
        self.front.menu.as_ref().filter(|m| m.visible()).map(|m| m.page)
    }

    /// The start-up sequence: the menu world as the backdrop, the logo movies, the title.
    pub fn start_front(&mut self) -> Result<(), String> {
        self.load_at(MENU_SLOT, flow::DEFAULT_POINT)?;
        // The title replaces the episode menu the menu world opened.
        self.front.queue = LOGO_MOVIES[1..].to_vec();
        if !self.start_movie(LOGO_MOVIES[0]) {
            self.front.queue.clear();
        }
        self.open_title();
        Ok(())
    }

    fn front_text(&self, id: i16, fallback: &str) -> Vec<u8> {
        super::layer::bytes(self.ui.strings.get(&id).map_or(fallback, String::as_str))
    }

    fn front_texts(&self) -> Texts {
        use strings::*;
        Texts {
            name: self.front_text(NAME, "Sly 3"),
            press_new: self.front_text(PRESS_NEW, "Press START button for New Game"),
            press_resume: self.front_text(PRESS_RESUME, "Press START button to Resume Game"),
            continue_game: self.front_text(CONTINUE, "Continue"),
            new_game: self.front_text(NEW_GAME, "New Game"),
            load_game: self.front_text(LOAD_GAME, "Load Game"),
            empty: self.front_text(EMPTY, "Empty"),
            overwrite: self.front_text(OVERWRITE, "Do you want to overwrite this game?"),
            episodes: self.front_text(NAME, "Sly 3"),
            start_episode: self.front_text(START_EPISODE, "Start Episode"),
            watch_movie: self.front_text(MOVIES, "Movies"),
        }
    }

    /// An empty progress block of the loaded world's tree.
    fn base_progress(&self) -> Option<Progress> {
        self.game.as_ref().and_then(|g| super::save_tree(&g.world)).map(|t| save::sly3::progress(Arc::new(t)))
    }

    /// The display name of `episode` (2 to 8).
    fn episode_title(&self, episode: u8) -> Vec<u8> {
        if episode == 2 {
            return self.front_text(strings::HAZARD_ROOM, "Hazard Room");
        }
        let k = episode.saturating_sub(3) as i16;
        let mut t = self.front_text(strings::FIRST_EPISODE_LABEL + k, &alloc::format!("Episode {}", k + 1));
        let title = self.front_text(strings::FIRST_TITLE + k, "");
        if !title.is_empty() {
            t.extend_from_slice(b": ");
            t.extend_from_slice(&title);
        }
        t
    }

    fn slot_rows(&self) -> ([SlotRow; 3], Option<usize>) {
        let mut rows: [SlotRow; 3] = Default::default();
        let (Some(slots), Some(base)) = (self.front.slots.as_ref(), self.base_progress()) else { return (rows, None) };
        for (i, row) in rows.iter_mut().enumerate() {
            if let Ok(l) = slots.load(i, &base)
                && !l.unused
            {
                let e = l.progress.int(&["episodeCurrent"]).clamp(0, 8) as u8;
                let text = if e >= 2 { self.episode_title(e) } else { self.front_text(strings::NAME, "Sly 3") };
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

    /// Opens the episode menu.
    pub fn open_episode_menu(&mut self) {
        let best = self.progress().map_or(0, |p| p.int(&["episodeBest"]).max(p.int(&["episodeCurrent"])));
        let highest = best.clamp(3, 8) as u8;
        let episodes = (2..=highest).map(|e| EpisodeRow { episode: e, title: self.episode_title(e) }).collect();
        self.front.menu = Some(FrontMenu::episode_menu(self.front_texts(), episodes, Vec::new()));
    }

    fn use_slot(&mut self, slot: usize) {
        self.front.slot = Some(slot);
    }

    /// Saves the progress to the slot in play (Save Game, autosaves).
    pub fn save_to_slot(&mut self) -> Result<u64, String> {
        let slot = self.front.slot.ok_or("no slot chosen")?;
        let slots = self.front.slots.clone().ok_or("no save slots")?;
        let p = self.progress().ok_or("no progress")?;
        slots.save(slot, p)
    }

    /// Starts an episode with `progress` as the block (none: a fresh one from the next world's tree).
    fn start_with(&mut self, progress: Option<Progress>, episode: usize) -> Result<(), String> {
        let ep = progress.as_ref().map(episode_progress).unwrap_or_default();
        self.jobs = None;
        self.progress = progress;
        self.start_episode(episode, &ep)
    }

    /// A new game in `slot` (its options kept): episode 1, so the prologue on Kaine Island.
    fn front_new_game(&mut self, slot: usize) -> Result<(), String> {
        let options = match (self.front.slots.as_ref(), self.base_progress()) {
            (Some(s), Some(base)) => s.load(slot, &base).ok().map(|l| {
                let mut p = l.progress;
                p.reset_keeping_options();
                p
            }),
            _ => None,
        };
        self.use_slot(slot);
        self.start_with(options, 1)
    }

    /// Loads `slot` and starts its episode.
    fn front_load(&mut self, slot: usize) -> Result<(), String> {
        let base = self.base_progress().ok_or("no Sly 3 world loaded")?;
        let loaded = self.front.slots.as_ref().ok_or("no save slots")?.load(slot, &base)?;
        self.use_slot(slot);
        let episode = loaded.progress.int(&["episodeCurrent"]).clamp(1, 8) as usize;
        self.start_with(Some(loaded.progress), episode)
    }

    /// Starts `episode` from the episode menu with the live progress.
    fn front_episode(&mut self, episode: u8) -> Result<(), String> {
        let p = self.jobs.take().map(|j| j.progress).or_else(|| self.progress.take());
        self.start_with(p, episode as usize)
    }

    /// Called once a world is in: the menu world shows the episode menu.
    pub(super) fn front_world_loaded(&mut self, slot: usize) {
        if slot == MENU_SLOT && !self.front_open() {
            self.open_episode_menu();
        }
    }

    /// The front end's part of a frame: movies first (+ skips), then the open menu. Returns the input the game gets,
    /// or `None` while a movie holds the world.
    pub(super) fn update_front(&mut self, input: &FrameInput, measured: f32) -> Option<FrameInput> {
        if self.front.movies.playing() {
            let skip = input.pressed & Buttons::PLUS.0 != 0;
            if !self.front.movies.update(measured, skip, self.sound.as_mut()) && !self.front.queue.is_empty() {
                let next = self.front.queue.remove(0);
                self.start_movie(next);
            }
            return None;
        }
        let menu = self.front.menu.as_mut()?;
        let (mi, circle) = super::layer::menu_input(input);
        let action = menu.update(engine::game::clock::frame_step(measured), &mi, circle);
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
        }
        Some(if self.front.menu.is_some() { FrameInput::default() } else { *input })
    }
}
